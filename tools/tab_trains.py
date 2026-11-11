"""Turns TAB's trains into ours: assets\\trains\\<name>.json + .png (explained in assets\\README.md).

TAB's pictures are copyrighted - only for us, on this PC (assets\\trains\\ is in .gitignore). This script makes them
anew from your own copy of the game:

    python tools\\tab_trains.py [assets folder] [game folder]

`assets folder`: the `_assets` folder of the extracted game (its `Sprites\\TrainsA\\`).
`game folder`: where TAB is installed - its `ZXGame.dxprj` says how big each carriage is drawn.

How TAB draws a train (`ZX.Entities\\TrainCarriage.cs`, `Locomotive.cs`):
1. The locomotive and every carriage is one sprite with 91 pictures - all the way round (about 4 degrees each), none
   mirrored, unlike the figures. TAB picks the picture for an angle with a formula of its own (`num2` in
   `OnUpdateSceneObjectParallel`): n = round(angle * 91 / 360), picture = (91 - n + 13 + 1 in two quarters) mod 91. So
   two angles next to each other can show the same picture - we keep that: `angles` lists the picture of each of the
   91 angles, the game only looks it up. What's left between two angles TAB turns the picture by (`SceneObject.Angle`).
2. Its size: the 700-pixel picture times `ImageScale` (scene units; one tile is 1.414 of them wide). Its middle stands
   on the point `ImageOffset` (map tiles, not turned with the train) next to the carriage's middle, `Height` scene
   units higher up.
3. A shadow: the same picture in black, 35 % (`CShadow.Opacity`), 0.6 scene units to the left, 1.1 times as big.
4. Wheels: 6 per carriage (`GetWheels`: +-0.8 and 0 along it, +-0.4 across - turned with it), each its own picture
   `Wheel` - the same 91 angles (picked with the carriage's picture number) times 20 frames of turning, frame =
   (scene units travelled * 4) mod 20. 0.7 scene units big, 0.2 higher up, drawn behind the carriage.
5. How long each one is (map tiles, `Size`) and the gap between two carriages (`CarriageSeparation` 0.4).
"""

import os
import sys

from PIL import Image

from tab_buildings import Project, Sheet, num, pair, premultiplied_resize
from tab_terrain import DEFAULT_GAME, DEFAULT_TAB, tinted

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "trains")
PIXELS_PER_TILE = 80
SCENE_PER_TILE = 2 ** 0.5  # one tile is 1.414 scene units wide (`DXWorldGrid.ScenePointFromWorldPoint`)

# Our name -> TAB's template (with its `TrainCarriage+CarriageConfiguration`) and sprite.
CARRIAGES = {
    "locomotive": ("Locomotive", "Locomotive_Normal"),
    "armored_carriage": ("ArmoredCarriage", "ArmoredCarriage_Normal"),
    "colonist_carriage": ("ColonistsCarriage", "ColonistCarriage_Normal"),
    "goods_carriage": ("GoodsCarriage", "GoodsCarriage_Normal"),
}
ANGLES = 91
# TrainCarriage.cs: the wheels, the shadow, the gap.
WHEELS = [[-0.8, 0.4], [-0.8, -0.4], [0.8, 0.4], [0.8, -0.4], [0.0, 0.4], [0.0, -0.4]]
WHEEL_SIZE, WHEEL_UP, WHEEL_FRAMES, WHEEL_STEPS_PER_UNIT = 0.7, 0.2, 20, 4
WHEEL_LOOK = (1, 0.85, 1.05, 0)  # saturation, contrast, brightness, hue
SHADOW = {"opacity": 0.35, "left": round(0.6 / SCENE_PER_TILE, 3), "scale": 1.1}
GAP = 0.4


def tab_picture(n):
    """The picture TAB shows for angle number n (0..90) - `OnUpdateSceneObjectParallel`, with the angle the number
    stands for."""
    angle = n * 360 / ANGLES
    extra = 14 if angle < 45 or 135 <= angle < 225 else 13
    return (ANGLES - n + extra) % ANGLES


def pictures(folder, stem):
    """{angle number: [paths of its frames in order]} of `<stem>_<angle>_<frame>.png`."""
    out = {}
    for file in sorted(os.listdir(folder)):
        if file.startswith(stem + "_") and file.endswith(".png"):
            angle = file[len(stem) + 1:-4].split("_")[0]
            out.setdefault(int(angle), []).append(os.path.join(folder, file))
    return out


def scaled(path, scale, look=(1, 1, 1, 0), tint=None):
    """The picture smaller by `scale`, coloured - and where its middle is now."""
    image = Image.open(path).convert("RGBA")
    w, h = max(1, round(image.width * scale)), max(1, round(image.height * scale))
    image = tinted(premultiplied_resize(image, (w, h)), look, tint)
    return image, (w / 2, h / 2)


def carriage(tab, project, key, template, sprite):
    folder = os.path.join(tab, "Sprites", "TrainsA", sprite)
    if template not in project.templates or not os.path.isdir(folder):
        print(f"  {key}: no {template} / {sprite} - left out")
        return
    cfg = project.config(template, "TrainCarriage+CarriageConfiguration")
    image_scale = num(cfg.get("ImageScale"), 1)
    tab_pixels_per_tile = SCENE_PER_TILE / image_scale
    scale = min(1.0, PIXELS_PER_TILE / tab_pixels_per_tile)
    look, tint = project.look(template)
    by_angle = pictures(folder, sprite)
    sheet = Sheet()
    for a in range(ANGLES):
        image, middle = scaled(by_angle[a][0], scale, look, tint)
        sheet.add(image, middle)
    length = pair(project.templates[template].find("Properties").find("Simple[@name='Size']").get("value"))[0]
    meta = {
        "pixels_per_tile": round(tab_pixels_per_tile * scale, 3),
        "length": length,
        "gap": GAP,
        "offset": list(pair(cfg.get("ImageOffset", "0;0"))),
        "height": round(num(cfg.get("Height")) / SCENE_PER_TILE, 3),
        "angles": [tab_picture(n) for n in range(ANGLES)],
        "shadow": SHADOW,
        "wheels": WHEELS,
    }
    sheet.save(key, meta, OUT)


def wheel(tab):
    folder = os.path.join(tab, "Sprites", "TrainsA", "Wheel")
    if not os.path.isdir(folder):
        print("  wheel: no Wheel - left out")
        return
    by_angle = pictures(folder, "Wheel")
    full = Image.open(by_angle[0][0]).width  # 120 pixels = WHEEL_SIZE scene units
    tab_pixels_per_tile = full / (WHEEL_SIZE / SCENE_PER_TILE)
    scale = min(1.0, PIXELS_PER_TILE / tab_pixels_per_tile)
    sheet = Sheet()
    for a in range(ANGLES):
        for path in by_angle[a][:WHEEL_FRAMES]:
            image, middle = scaled(path, scale, WHEEL_LOOK)
            sheet.add(image, middle)
    meta = {
        "pixels_per_tile": round(tab_pixels_per_tile * scale, 3),
        "frames": WHEEL_FRAMES,
        "per_tile_travelled": round(WHEEL_STEPS_PER_UNIT * SCENE_PER_TILE, 3),
        "up": round(WHEEL_UP / SCENE_PER_TILE, 3),
    }
    sheet.save("wheel", meta, OUT)


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    game = sys.argv[2] if len(sys.argv) > 2 else DEFAULT_GAME
    if not os.path.isdir(os.path.join(tab, "Sprites", "TrainsA")):
        sys.exit(f"No Sprites\\TrainsA folder in {tab} - give the _assets folder of the extracted game.")
    print("Reading ZXGame.dxprj ...")
    project = Project(game)
    os.makedirs(OUT, exist_ok=True)
    print(f"From {tab} to {os.path.abspath(OUT)}:")
    for key, (template, sprite) in CARRIAGES.items():
        carriage(tab, project, key, template, sprite)
    wheel(tab)


if __name__ == "__main__":
    main()
