"""Turns TAB's sprites into our skins: assets\\tab\\<skin>.json + .png (explained in assets\\README.md).

TAB's pictures are copyrighted - only for us, on this PC. That's why assets\\tab\\ is in .gitignore and this script
makes it anew from your own extracted copy:

    python tools\\tab_skins.py [folder]

`folder`: the `_assets` folder of the extracted game (default below). It needs `Sprites\\<atlas>\\<animation>\\*.png`,
one picture per frame, named `<name>_<animation>_<angle>_<frame>.png` (tools\\extract_assets.py of the decompiling
project writes them like that).

What it does per skin:
1. Collects TAB's animations (Normal, Walk, Run, Attack, Special, Die) - 10 directions each, 18 degrees apart; the other
   half TAB draws mirrored, and so do we (`mirror` in the JSON).
2. Cuts away the empty border all frames share, makes them smaller (80 pixels per tile - less if the picture would
   get too big) and keeps at most `MAX_FRAMES` frames per animation - so all of them together fit the graphics card.
3. Colours them like TAB does in the game (its `DefaultVisualState`: saturation, contrast, brightness, hue).
4. Lays them out like assets\\README.md says (one row per direction, the frames side by side, the animations as blocks
   next to and below each other) and writes the JSON: where the feet are, how fast each animation plays (TAB's
   `TimeAni...` in Entities.tsv).
"""

import json
import os
import re
import sys

from PIL import Image, ImageEnhance

DEFAULT_TAB = r"E:\Decoded_Software\TheyAreBillions_Decompiled\_assets"
ASSETS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
# Our units' skins go to assets\units\, all others (the zombies) to assets\zombies\.
UNITS = ("ranger", "soldier", "sniper")

# Pixels per tile width we aim for (128 = sharp at the biggest zoom, 64 = at the normal one) ...
PIXELS_PER_TILE = 80
# ... and no picture wider or higher than this (graphics cards all manage 4096).
MAX_SIDE = 4096
# At most this many frames per animation - more are thinned out evenly (the giant: fewer, it's huge). Standing and
# growling need fewer: they move little.
MAX_FRAMES = 16
CALM_FRAMES = 12
# Transparent pixels around every frame, so the neighbours don't bleed in when it's drawn smaller.
PAD = 2

# TAB's stored angles, as our rows: half a step clockwise from straight up (234 = -126 degrees) to half a step before
# straight down (36) - see `Sprite::direction` in src\sprites.rs.
ROWS = [234, 252, 270, 288, 306, 324, 342, 0, 18, 36]


class Skin:
    """One TAB figure: where its pictures are, how TAB draws them, how fast it moves."""

    def __init__(self, atlas, name, width, scaling, offset, look, times, speeds, attack_ms, frames=None, side=None, die_ms=1000):
        self.atlas = atlas  # folder in Sprites\
        self.name = name  # how its frames are called
        self.width = width  # TAB's Width: its body, in tiles
        self.scaling = scaling  # TAB's SceneScalingFactor: the frame is width * scaling tile widths wide
        self.offset = offset  # TAB's SceneOffsetFactor (x, y): the frame moved by that share of its size
        self.look = look  # saturation, contrast, brightness, hue rotation (degrees)
        self.times = times  # TimeAniNormal, TimeAniSpecial, TimeAniWalk, TimeAniRun (milliseconds)
        self.speeds = speeds  # WalkSpeed, RunSpeed (tiles per second)
        self.attack_ms = attack_ms  # one attack (the command's TimeAction)
        self.frames = frames or MAX_FRAMES  # at most this many frames per animation
        self.side = side or MAX_SIDE  # and its picture no wider or higher than this
        self.die_ms = die_ms  # TimeAniDie: how long its death plays


# Our skin -> TAB's figure. Numbers from TAB's ZXGame.dxprj (templates: Width, SceneScalingFactor, SceneOffsetFactor,
# DefaultVisualState) and ZXRules (Entities: TimeAni..., WalkSpeed, RunSpeed; Commands: TimeAction). `die_ms`: its
# TimeAniDie where it isn't 1000.
SKINS = {
    "walker_aged": Skin("InfectedA", "ZombieWeakA", 0.4, 1.8, (0, 0.05), (0.3, 0.8, 1, 0), (3000, 4000, 2000, 1000), (0.13, 0.4), 1000),
    "walker_decrepit": Skin("InfectedA", "ZombieWeakB", 0.4, 2.5, (-0.04, 0.26), (0.3, 0.7, 1, 0), (2000, 1000, 2000, 1000), (0.13, 0.4), 1000),
    "walker_young": Skin("InfectedA", "ZombieWeakC", 0.4, 2.2, (0, 0.22), (0.3, 0.9, 1, 0), (2000, 1000, 2000, 800), (0.334, 0.4), 1000),
    "runner_colonist_a": Skin("InfectedA", "WorkerInfected_A", 0.4, 8.4, (-0.009, 0.41), (0.3, 1.1, 1, 0), (2000, 2000, 2000, 600), (0.3, 1.75), 900),
    "runner_colonist_b": Skin("InfectedA", "WorkerInfected_B", 0.4, 8.4, (-0.009, 0.41), (0.3, 1.2, 1, 0), (2000, 2000, 2000, 600), (0.3, 1.75), 900),
    "runner_fresh_a": Skin("InfectedB", "InfectedMedium_A", 0.4, 3.8, (-0.009, 0.3), (0.3, 0.67, 1.1, 0), (2000, 2000, 2000, 600), (0.3, 1.75), 1000),
    "runner_fresh_b": Skin("InfectedB", "InfectedMedium_B", 0.4, 3.8, (-0.009, 0.3), (0.5, 0.7, 1.1, 0), (2000, 3000, 2000, 600), (0.3, 1.75), 1000),
    "riot": Skin("InfectedA", "ZombieDressedA", 0.4, 2.2, (0, 0.13), (1, 0.7, 1, 0), (2000, 2000, 2000, 1300), (0.08, 1.75), 800),
    "fat": Skin("InfectedC", "InfectedStrong_A", 0.7, 3.2, (-0.009, 0.3), (1, 0.55, 1.02, 0), (2000, 2000, 2000, 1000), (0.3, 1.75), 1000),
    "spitter": Skin("InfectedC", "InfectedVenom", 0.5, 3.3, (-0.03, 0.28), (0.6, 0.5, 1, 30), (1000, 3000, 1600, 500), (0.4, 1.75), 1400),
    "harpy": Skin("InfectedB", "InfectedHarpy", 0.6, 2.8, (-0.02, 0.29), (1.2, 0.6, 1, 0), (2000, 6000, 2000, 500), (0.3, 5), 300),
    "giant": Skin("InfectedGiant", "InfectedGiant", 1.6, 3.6, (-0.009, 0.25), (2, 0.6, 1, 0), (2000, 2000, 1800, 800), (1, 4), 800, frames=10, side=3072),
    "ranger": Skin("HumansA", "Ranger", 0.45, 3.8, (0, 0.3), (1, 0.8, 1, 0), (3000, 4000, 550, 550), (0.5, 4), 1000),
    "soldier": Skin("HumansA", "Soldier_A", 0.5, 1.8, (0, 0.06), (0.8, 1, 1, 0), (3000, 2000, 700, 700), (0.6, 2.4), 500, die_ms=1500),
    "sniper": Skin("HumansA", "Sniper", 0.6, 3, (0, 0.28), (1, 0.8, 1.07, 0), (3000, 2500, 800, 800), (0.5, 1.8), 600, die_ms=1200),
}

# Our animation -> TAB's.
ANIMATIONS = [("idle", "Normal"), ("walk", "Walk"), ("run", "Run"), ("attack", "Attack"), ("special", "Special"), ("die", "Die")]


def frames_of(folder, name, animation):
    """{angle: [paths of its frames in order]} - empty if TAB has no such animation for this figure."""
    path = os.path.join(folder, f"{name}_{animation}")
    if not os.path.isdir(path):
        return {}
    pattern = re.compile(re.escape(f"{name}_{animation}_") + r"(\d+)_(\d+)\.png$")
    angles = {}
    for file in os.listdir(path):
        m = pattern.match(file)
        if m:
            angles.setdefault(int(m.group(1)), []).append((int(m.group(2)), os.path.join(path, file)))
    return {a: [p for _, p in sorted(frames)] for a, frames in angles.items()}


def nearest(angles, wanted):
    """The stored angle closest to `wanted` (round the circle) - TAB does the same when one is missing."""
    return min(angles, key=lambda a: min(abs(a - wanted), 360 - abs(a - wanted)))


def thin(paths, most):
    """At most `most` of them, evenly spread."""
    if len(paths) <= most:
        return paths
    return [paths[i * len(paths) // most] for i in range(most)]


def layout(blocks):
    """Where each block (width, height in pixels) goes: on shelves, filled left to right - as many blocks wide as makes
    the picture closest to a square. Returns their top left corners and the size of the whole picture."""
    best = None
    for shelf_width in range(max(w for w, _ in blocks), sum(w for w, _ in blocks) + 1):
        places, x, y, shelf, width = [], 0, 0, 0, 0
        for w, h in blocks:
            if x > 0 and x + w > shelf_width:
                x, y, shelf = 0, y + shelf, 0
            places.append((x, y))
            x += w
            shelf = max(shelf, h)
            width = max(width, x)
        height = y + shelf
        if best is None or max(width, height) < max(best[1], best[2]):
            best = (places, width, height)
        if width >= height:
            break  # wider still only makes it worse
    return best


def colour(image, look):
    """TAB's DefaultVisualState: hue turned, then saturation, contrast (around the middle grey), brightness."""
    saturation, contrast, brightness, hue = look
    alpha = image.getchannel("A")
    rgb = image.convert("RGB")
    if hue:
        h, s, v = rgb.convert("HSV").split()
        shift = round(hue / 360 * 255)
        h = h.point(lambda x: (x + shift) % 256)
        rgb = Image.merge("HSV", (h, s, v)).convert("RGB")
    rgb = ImageEnhance.Color(rgb).enhance(saturation)
    rgb = rgb.point(lambda x: max(0, min(255, round(((x - 128) * contrast + 128) * brightness))))
    rgb.putalpha(alpha)
    return rgb


def folder_of(key):
    """Where a skin goes: assets\\units\\ or assets\\zombies\\ (made if it isn't there)."""
    folder = os.path.join(ASSETS, "units" if key in UNITS else "zombies")
    os.makedirs(folder, exist_ok=True)
    return folder


def convert(tab, key, skin):
    folder = os.path.join(tab, "Sprites", skin.atlas)
    # Per animation: per row the frame paths (missing angles: the nearest one).
    animations = []
    for ours, theirs in ANIMATIONS:
        angles = frames_of(folder, skin.name, theirs)
        if angles:
            most = min(skin.frames, CALM_FRAMES) if ours in ("idle", "special", "die") else skin.frames
            animations.append((ours, [thin(angles[nearest(angles, row)], most) for row in ROWS]))
    if not animations:
        print(f"  {key}: no pictures of {skin.name} in {folder} - left out")
        return
    names = [a for a, _ in animations]
    if "idle" not in names:
        print(f"  {key}: no Normal animation - left out")
        return

    # The empty border all frames share, in TAB's frame size.
    first = Image.open(animations[0][1][0][0])
    full_w, full_h = first.size
    box = None
    for _, rows in animations:
        for paths in rows:
            for path in paths:
                b = Image.open(path).getchannel("A").getbbox()
                if b:
                    box = b if box is None else (min(box[0], b[0]), min(box[1], b[1]), max(box[2], b[2]), max(box[3], b[3]))
    x0, y0, x1, y1 = box

    # Where the feet are (TAB's CSprite): the frame's bottom middle stands on the lowest corner of the body's square,
    # moved by the offset. The frame is width * scaling tile widths wide; a tile is sqrt(2) of TAB's scene units wide.
    tab_pixels_per_tile = full_w / (skin.width * skin.scaling)
    pixels_per_scene_unit = tab_pixels_per_tile / 2**0.5
    feet_x = full_w / 2 - skin.offset[0] * full_w
    feet_y = full_h * (1 - skin.offset[1]) - skin.width / 2 * pixels_per_scene_unit

    # How much smaller: towards PIXELS_PER_TILE (never bigger than TAB's), less if the picture would be too big.
    scale = min(1.0, PIXELS_PER_TILE / tab_pixels_per_tile)
    while True:
        fw = round((x1 - x0) * scale) + 2 * PAD
        fh = round((y1 - y0) * scale) + 2 * PAD
        places, width, height = layout([(len(rows[0]) * fw, len(ROWS) * fh) for _, rows in animations])
        if (width <= skin.side and height <= skin.side) or scale < 0.05:
            break
        scale *= 0.95

    sheet = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    meta = {
        "image": f"{key}.png",
        "frame": [fw, fh],
        "feet": [round((feet_x - x0) * scale + PAD, 1), round((feet_y - y0) * scale + PAD, 1)],
        "pixels_per_tile": round(tab_pixels_per_tile * scale, 2),
        "directions": len(ROWS),
        "mirror": True,
    }
    normal_ms, special_ms, walk_ms, run_ms = skin.times
    walk_speed, run_speed = skin.speeds
    for (ours, rows), (left, top) in zip(animations, places):
        for r, paths in enumerate(rows):
            for c, path in enumerate(paths):
                frame = Image.open(path).convert("RGBA").crop((x0, y0, x1, y1))
                # Smaller with premultiplied alpha ("RGBa"), so the edges don't get dark fringes.
                frame = frame.convert("RGBa").resize((fw - 2 * PAD, fh - 2 * PAD), Image.LANCZOS).convert("RGBA")
                sheet.paste(colour(frame, skin.look), (left + c * fw + PAD, top + r * fh + PAD))
        n = len(rows[0])
        # `at` counts in frames, not pixels.
        animation = {"at": [left // fw, top // fh], "frames": n}
        if ours == "idle":
            animation["fps"] = round(n / (normal_ms / 1000), 2)
        elif ours == "walk":
            animation["tiles"] = round(walk_speed * walk_ms / 1000, 3)
        elif ours == "run":
            animation["tiles"] = round(run_speed * run_ms / 1000, 3)
        elif ours == "attack":
            animation["fps"] = round(n / (skin.attack_ms / 1000), 2)
        elif ours == "special":
            animation["fps"] = round(n / (special_ms / 1000), 2)
        elif ours == "die":
            animation["fps"] = round(n / (skin.die_ms / 1000), 2)
        meta[ours] = animation
    # No running animation (TAB's weak walkers): the walking one, played at running pace - like TAB does.
    if "run" not in meta and "walk" in meta:
        meta["run"] = dict(meta["walk"], tiles=round(run_speed * run_ms / 1000, 3))
    # No walking animation (TAB's units only run): the running one.
    if "walk" in meta:
        meta["run_from"] = round((walk_speed + run_speed) / 2, 3)

    out = folder_of(key)
    sheet.save(os.path.join(out, f"{key}.png"), optimize=True)
    with open(os.path.join(out, f"{key}.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2)
    print(f"  {key}: {', '.join(names)} - {width} x {height} pixels, {meta['pixels_per_tile']} per tile")


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    if not os.path.isdir(os.path.join(tab, "Sprites")):
        sys.exit(f"No Sprites folder in {tab} - give the _assets folder of the extracted game.")
    print(f"From {tab} to {os.path.abspath(ASSETS)}:")
    for key, skin in SKINS.items():
        convert(tab, key, skin)


if __name__ == "__main__":
    main()
