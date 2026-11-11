"""Turns TAB's effects into ours: assets\\effects\\<name>.json + .png, and its arrows into assets\\projectiles\\
(explained in assets\\README.md).

TAB's pictures are copyrighted - only for us, on this PC (both folders are in .gitignore). This script makes them anew
from your own copy of the game:

    python tools\\tab_effects.py [assets folder] [game folder]

`assets folder`: the `_assets` folder of the extracted game (its `Sprites\\Effects_A\\` and `Images\\`).
`game folder`: where TAB is installed - its `ZXGame.dxprj` says how each effect is drawn.

What TAB does and what we take from it (`ZX\\VisualEffects.cs`, `DXComponentAtlasClip`, `CShotEffect`):
1. An effect is a template with a sprite (`DXComponentAtlasClip`): its frames played once in `Duration` milliseconds
   (or over and over), fading out after `VanishFactorTime` of it. It's drawn `size x SceneScalingFactor` tile widths
   wide - `size` is what the game asks for (a hit on a zombie 0.4, a barrel's blast 5) - its bottom middle on the
   lowest corner of that square, moved by `SceneOffsetFactor`. Coloured by its `DefaultVisualState`; `Add` ones are
   added to what's below (they glow).
2. Stains (`Blood_Stain_A`, `PoolGreenA`) are single pictures lying flat on the ground - the game grows and fades them.
3. The flashes at a gun's muzzle (`CShotEffect`) aren't templates: the sprite with its frame time and size from the
   code (`SHOTS` below).
4. Arrows (`Images\\Proyectiles`) are single pictures, pointing up - the game turns them the way they fly. So is
   Thanatos' rocket (`Missile_A`, `Proyectile.cs`).
5. What TAB's code draws itself, without a template (`SPRITES`): the tesla's and shocking tower's spheres, the falling
   leaves of the forests (`ZXMapThemeFA.AddLeave`) ... - their frames at TAB's 30 ms each.
6. Weather and animated water (`Animations\\`, TAB's GIFs: `Rain_A`, `RainA`, `Snow_A`, `SnowB`, `Water_*`): square
   pictures on black, added to the screen (`BlendMode.Add`) - as lights, with TAB's `FrameTime`.
7. More single pictures (`PICTURES`): the range circle (`AttackRange`), lights, smoke puffs (`SmokeParticle1..9`,
   `VisualEffects.cs`), the lava pool, the blast wave ...
"""

import json
import math
import os
import sys

from PIL import Image, ImageChops

from tab_buildings import Frames, Project, argb, kind, num, premultiplied_resize, props
from tab_skins import layout
from tab_terrain import DEFAULT_GAME, DEFAULT_TAB, single, tinted

ASSETS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")

# Our effect -> (TAB's template, how wide it's mostly drawn - tile widths, for how sharp its picture is made).
# Where TAB shows them: a unit hit (`BleedUnit`), a death (`DieUnit`), a building falling (`ExplosionOnDestroy` in
# Entities.tsv, `TemplateFromExplosion`), a building infected (`CInfected`), a machine gun's hits (`ZXCommand`), a
# barrel (`ExplosiveBarrel`: FireMedium).
EFFECTS = {
    "blood_splash": ("BloodSplash_A", 0.5),
    "blood_zombie": ("ExplosionBloodZombie_A", 1.2),
    "blood_human": ("ExplosionBlood_A", 1.2),
    "venom": ("ExplosionVenom", 0.8),
    "gun_impact_a": ("Explosion_MachineGunA", 0.6),
    "gun_impact_b": ("Explosion_MachineGunB", 0.4),
    "fire_blast": ("ExplosionC", 5.0),
    "small_fire": ("Explosion_B", 1.3),
    "wood_medium": ("Explosion_Wood_A", 2.0),
    "wood_big": ("Explosion_Wood_B", 5.2),
    "stone_medium": ("Explosion_Stone_A", 2.4),
    "stone_big": ("Explosion_Stone_B", 5.2),
    "crystal": ("Explosion_Crystal_A", 3.0),
    # Used by the lasting effects and TAB's late buildings and units (src\effects\lasting.rs, docs\tab-assets.md
    # steps 3-7): fire and smoke on damaged buildings, the shocking tower's ray, water, the infection's green smoke.
    # Not used yet: the transmutator's cosmic blast, snow flakes.
    "fire": ("Fire", 1.5),
    "fire_ornament": ("FireOrnamentA", 1.0),
    "fire_burst": ("FireBurst_A", 1.5),
    "fire_smoke_small": ("ExplosionFireSmokeSmall_A", 1.3),
    "smoke_a": ("Smoke_A", 2.0),
    "smoke_b": ("Smoke_B", 1.0),
    "smoke_c": ("Smoke_C", 1.5),
    "smoke_fast": ("SmokeFast_B", 1.0),
    "infection": ("Infection_A", 1.5),
    "light_flare": ("LightFlare_A", 1.5),
    "shock_ray": ("ShockRay_A", 2.0),
    "cosmic": ("Explosion_Cosmic_A", 3.0),
    "capsule_blast": ("Explosion_InfectedCapsule", 2.4),
    "water_fall": ("Water_Fall_A", 1.5),
    "water_splash": ("WaterSplash_A", 1.0),
    "green_splash": ("GreenLiquidSplash_A", 1.0),
    "snow_flake": ("Snow_A", 0.5),
}
# Sprites TAB's code draws without a template: sprite in Effects_A, how wide it's mostly drawn (tile widths), loops.
# Added (glowing) if the sprite has no see-through pixels (TAB draws those with `BlendMode.Add`).
SPRITES = {
    "tesla_sphere": ("TeslaSphere_A", 1.0, True),
    "electric_sphere": ("ElectricSphere", 2.0, True),
    "electric_halo": ("ElectricHalo_A", 1.5, True),
    "bubbles": ("BubblesA", 0.5, True),
    "flow_blood": ("FlowBlood_A", 1.0, False),
    "leaf": ("Leaf", 0.8, True),
    "flash_thanatos": ("ShotSingle_B", 1.0, False),
}
# TAB's GIFs (`Animations\`): ours -> its folder, how many tiles one picture covers (TAB's `Size` of the entity that
# shows it: rain 12, snow 10, water 5 / 10), FrameTime in ms. All on black, added.
ANIMATIONS = {
    "rain_a": ("Rain_A", 12, 33),
    "rain_b": ("RainA", 12, 25),
    "snow_a": ("Snow_A", 10, 33),
    "snow_b": ("SnowB", 10, 33),
    "water_a": ("Water_A", 5, 33),
    "water_b": ("Water_B", 10, 33),
    "water_shine": ("Water_Animated_A", 10, 33),
    "explosion_a": ("Explosion_A", 3, 33),
}
# More single pictures in Images\Effects: ours -> (TAB's name, tile widths, meta). `ground`: lies flat.
PICTURES = {
    # TAB's `CVisualAttackRange`: added, contrast 0.5, its hue turned 120 degrees (red -> green).
    "range_circle": ("AttackRange", 2.0, {"ground": True, "add": True, "look": (1, 0.5, 1, 120)}),
    "blast_wave": ("BlastWaveA", 4.0, {"ground": True, "add": True}),
    "bright_flare": ("BrightFlareA", 2.0, {"add": True}),
    "flame": ("Flame_A", 1.0, {"add": True}),
    "light": ("Light_A", 3.0, {"add": True}),
    "light_white_a": ("LightWhite_A", 3.0, {"add": True}),
    "light_white_b": ("LightWhiteB", 3.0, {"add": True}),
    "light_white_c": ("LightWhiteC", 3.0, {"add": True}),
    "black_circle": ("BlackGradientCircle", 3.0, {}),
    "splat": ("SplatA", 1.0, {"ground": True}),
    "trail": ("TrailWhiteA", 1.0, {"add": True}),
    "venom_pool_b": ("PoolGreenB", 1.0, {"ground": True}),
    "venom_pool_c": ("PoolGreenC", 1.0, {"ground": True}),
    "venom_pool_d": ("PoolGreenD", 1.0, {"ground": True}),
    **{f"smoke_puff_{n}": (f"SmokeParticle{n}", 1.0, {}) for n in range(1, 10)},
    **{f"lava_pool_{n}": (f"Lavapool/Lavapool_0{n}", 2.0, {"ground": True}) for n in range(1, 5)},
}
# The muzzle flashes (`CShotEffect`): sprite, milliseconds per frame, how long it shows (None: all its frames once),
# size (tile widths: length, thickness), contrast, how far in front of the shooter and how high (TAB's `_Distance`,
# `_DeltaH`).
SHOTS = {
    "flash_soldier": ("Shot_A", 20, 100, (0.8, 0.64), 1.5, 0.1, 0.65),
    "flash_sniper": ("ShotSingle_A", 10, None, (0.8, 0.64), 1.5, 0.7, 0.75),
    "flash_gun": ("Shot_B", 20, 500, (1.2, 0.6), 1.5, 1.0, 0.475),
}
# Stains on the ground: picture in Images\Effects, its look (saturation, contrast, brightness, hue).
STAINS = {
    "blood_stain": ("Blood_Stain_A", (1, 0.2, 1, 0)),
    "venom_pool": ("PoolGreenA", (1, 1, 1, 0)),
}
# Arrows: picture in Images\Proyectiles, how long it's drawn (tile widths).
ARROWS = {"arrow": ("Arrow_A", 0.55), "big_arrow": ("BigArrow", 1.2), "missile": ("Missile_A", 0.8)}

# Pixels per tile width at most (like the figures) - big explosions get fewer, so their frames fit.
PIXELS_PER_TILE = 80
# No frame wider or higher than this, no sheet bigger than MAX_SIDE, and at most MAX_FRAMES frames (more are thinned out
# evenly - the explosions have up to 72).
MAX_FRAME = 256
MAX_SIDE = 4096
MAX_FRAMES = 48
PAD = 2


def merged(project, template, name):
    """A template's configuration of this kind with everything it inherits: the later ones fill in what the earlier
    ones leave empty (TAB keeps the base template's empty one first)."""
    out = {}
    for cfg in project.templates[template].iter("Complex"):
        if kind(cfg) == name:
            out.update({k: v for k, v in props(cfg).items() if v not in (None, "")})
    return out


def as_light(image):
    """An added picture (`BlendMode.Add`) as the game draws it: as opaque as it's bright - added to what's below with its
    alpha, it adds exactly its colour."""
    r, g, b, _ = image.convert("RGBa").split()
    a = ImageChops.lighter(r, ImageChops.lighter(g, b))
    return Image.merge("RGBa", (r, g, b, a)).convert("RGBA")


def thin(items, most):
    if len(items) <= most:
        return items
    return [items[i * len(items) // most] for i in range(most)]


def save(folder, key, frames, feet, meta, tiles):
    """Frames (all the same size, TAB's pixels) cut to what they show, made smaller and put side by side - `feet`: where
    the effect stands in a frame, `tiles`: how wide a frame is drawn for size 1. Writes `<key>.json` + `.png`."""
    box = None
    for f in frames:
        b = f.getchannel("A").getbbox()
        if b:
            box = b if box is None else (min(box[0], b[0]), min(box[1], b[1]), max(box[2], b[2]), max(box[3], b[3]))
    if box is None:
        print(f"  {key}: nothing to see - left out")
        return
    full_w = frames[0].width
    tab_ppt = full_w / tiles  # TAB's pixels per tile width, drawn at size 1
    # As sharp as PIXELS_PER_TILE where it's mostly drawn `typical` tile widths wide (the whole frame) - never bigger
    # than TAB's, never a frame over MAX_FRAME.
    typical = meta.pop("typical_tiles")
    w, h = box[2] - box[0], box[3] - box[1]
    scale = min(1.0, PIXELS_PER_TILE * typical / full_w, MAX_FRAME / max(w, h))
    while True:
        fw, fh = max(1, round(w * scale)) + 2 * PAD, max(1, round(h * scale)) + 2 * PAD
        _, width, height = layout([(fw, fh)] * len(frames))
        if (width <= MAX_SIDE and height <= MAX_SIDE) or scale < 0.02:
            break
        scale *= 0.9
    columns = max(1, width // fw)
    rows = math.ceil(len(frames) / columns)
    sheet = Image.new("RGBA", (columns * fw, rows * fh), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        f = premultiplied_resize(f.crop(box), (fw - 2 * PAD, fh - 2 * PAD))
        # See-through pixels black, so no colour bleeds into the edge when drawn smaller.
        f = Image.composite(f, Image.new("RGBA", f.size, (0, 0, 0, 0)), f.getchannel("A").point(lambda v: 255 if v else 0))
        sheet.paste(f, ((i % columns) * fw + PAD, (i // columns) * fh + PAD))
    sheet.save(os.path.join(folder, f"{key}.png"), optimize=True)
    out = {
        "image": f"{key}.png",
        "frame": [fw, fh],
        "columns": columns,
        "frames": len(frames),
        "feet": [round((feet[0] - box[0]) * scale + PAD, 1), round((feet[1] - box[1]) * scale + PAD, 1)],
        "pixels_per_tile": round(tab_ppt * scale, 3),
        **meta,
    }
    with open(os.path.join(folder, f"{key}.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1)
    print(f"  {key}: {len(frames)} frames of {fw} x {fh}, {sheet.width} x {sheet.height} pixels")


def sprite_frames(tab, atlas, name):
    paths, step = Frames.of(tab, atlas, name.rstrip("_"))
    return paths, step


def effect(tab, project, folder, key, template, typical):
    clip = merged(project, template, "DXComponentAtlasClip+TTemplateConfiguration")
    state = merged(project, template, "DXComponentDefaultVisualState+TTemplateConfiguration")
    paths, _ = sprite_frames(tab, clip["File"], clip["Name"])
    if not paths:
        print(f"  {key}: no frames of {clip['Name']} - left out")
        return
    look = (num(state.get("Saturation"), 1), num(state.get("Contrast"), 1), num(state.get("Brightness"), 1), num(state.get("HueRotation")))
    add = state.get("BlendMode") == "Add"
    frames = []
    for path in thin(paths, MAX_FRAMES):
        f = tinted(Image.open(path).convert("RGBA"), look, argb(state.get("ColorHue")))
        frames.append(as_light(f) if add else f)
    scaling = num(clip.get("SceneScalingFactor"), 1)
    off_x, off_y = num(clip.get("SceneOffsetFactorX")), num(clip.get("SceneOffsetFactorY"))
    w, h = frames[0].size
    # `DXComponentVisual.UpdateArea`: the bottom middle on the lowest corner of the square, moved by the offset.
    feet = (w * (0.5 - off_x), h * (1 - off_y))
    meta = {"typical_tiles": typical, "duration": num(clip.get("Duration"), 1000) / 1000, "loop": clip.get("Loop") == "True"}
    if clip.get("VanishFactorTime"):
        meta["vanish"] = num(clip["VanishFactorTime"])
    if add:
        meta["add"] = True
    opacity = num(state.get("Opacity"), 1)
    if opacity < 1:
        meta["opacity"] = opacity
    save(folder, key, frames, feet, meta, scaling)


def shot(tab, folder, key, sprite, frame_ms, duration_ms, size, contrast, distance, height):
    paths, step = sprite_frames(tab, "Effects_A", sprite)
    look = (1, contrast, 1, 0)
    frames = [as_light(tinted(Image.open(p).convert("RGBA"), look, None)) for p in thin(paths, MAX_FRAMES)]
    w, h = frames[0].size
    # The flash points to the right in its picture; it starts at its left middle - that point sits at the muzzle.
    duration = (duration_ms if duration_ms else len(paths) * step * frame_ms) / 1000
    meta = {"typical_tiles": size[0], "duration": duration, "loop": False, "add": True, "turns": True,
            "squash": round(size[1] / size[0] * w / h, 3), "ahead": distance, "height": height}
    save(folder, key, frames, (0, h / 2), meta, size[0])


def picture(tab, folder, key, category, stem, look, tiles, meta):
    path = single(os.path.join(tab, "Images"), category, stem)
    if not path:
        print(f"  {key}: no picture {stem} - left out")
        return
    image = tinted(Image.open(path).convert("RGBA"), look, None)
    w, h = image.size
    save(folder, key, [image], (w / 2, h / 2), {"typical_tiles": tiles, **meta}, tiles)


def code_sprite(tab, folder, key, sprite, typical, loop):
    """A sprite TAB's code draws itself (SPRITES): its frames at 30 ms each, standing on its bottom middle."""
    paths, step = sprite_frames(tab, "Effects_A", sprite)
    if not paths:
        print(f"  {key}: no frames of {sprite} - left out")
        return
    first = Image.open(paths[0]).convert("RGBA")
    add = first.getchannel("A").getextrema()[0] == 255
    frames = [Image.open(p).convert("RGBA") for p in thin(paths, MAX_FRAMES)]
    if add:
        frames = [as_light(f) for f in frames]
    w, h = frames[0].size
    meta = {"typical_tiles": typical, "duration": round(len(paths) * step * 0.03, 3), "loop": loop}
    if add:
        meta["add"] = True
    save(folder, key, frames, (w / 2, h), meta, typical)


def animation(tab, folder, key, name, tiles, frame_ms):
    """A GIF of TAB's (`Animations\\<name>\\`): pictures on black, added - `tiles` wide, standing on their middle."""
    path = os.path.join(tab, "Animations", name)
    if not os.path.isdir(path):
        print(f"  {key}: no Animations\\{name} - left out")
        return
    files = sorted(os.path.join(path, f) for f in os.listdir(path) if f.lower().endswith(".png"))
    frames = [as_light(Image.open(f).convert("RGBA")) for f in thin(files, MAX_FRAMES)]
    w, h = frames[0].size
    meta = {"typical_tiles": tiles, "duration": round(len(files) * frame_ms / 1000, 3), "loop": True, "add": True}
    save(folder, key, frames, (w / 2, h / 2), meta, tiles)


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    game = sys.argv[2] if len(sys.argv) > 2 else DEFAULT_GAME
    if not os.path.isdir(os.path.join(tab, "Sprites")):
        sys.exit(f"No Sprites folder in {tab} - give the _assets folder of the extracted game.")
    print("Reading ZXGame.dxprj ...")
    project = Project(game)
    effects = os.path.join(ASSETS, "effects")
    arrows = os.path.join(ASSETS, "projectiles")
    os.makedirs(effects, exist_ok=True)
    os.makedirs(arrows, exist_ok=True)
    print(f"From {tab} to {os.path.abspath(effects)} and {os.path.abspath(arrows)}:")
    for key, (template, typical) in EFFECTS.items():
        effect(tab, project, effects, key, template, typical)
    for key, params in SHOTS.items():
        shot(tab, effects, key, *params)
    for key, (stem, look) in STAINS.items():
        picture(tab, effects, key, "Effects", stem, look, 1.0, {"duration": 0, "loop": False, "ground": True})
    for key, (sprite, typical, loop) in SPRITES.items():
        code_sprite(tab, effects, key, sprite, typical, loop)
    for key, (name, tiles, frame_ms) in ANIMATIONS.items():
        animation(tab, effects, key, name, tiles, frame_ms)
    for key, (stem, tiles, meta) in PICTURES.items():
        # `look` (saturation, contrast, brightness, hue) is for the colouring, not for the JSON.
        meta = dict(meta)
        look = meta.pop("look", (1, 1, 1, 0))
        picture(tab, effects, key, "Effects", stem, look, tiles, {"duration": 0, "loop": False, **meta})
    for key, (stem, length) in ARROWS.items():
        picture(tab, arrows, key, "Proyectiles", stem, (1, 1, 1, 0), length, {"duration": 0, "loop": False, "turns": True})


if __name__ == "__main__":
    main()
