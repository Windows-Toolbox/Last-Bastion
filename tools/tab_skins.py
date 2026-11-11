"""Turns TAB's sprites into our skins: assets\\tab\\<skin>.json + .png (explained in assets\\README.md).

TAB's pictures are copyrighted - only for us, on this PC. That's why assets\\tab\\ is in .gitignore and this script
makes it anew from your own extracted copy:

    python tools\\tab_skins.py [folder] [skin ...]

`folder`: the `_assets` folder of the extracted game (default below). It needs `Sprites\\<atlas>\\<animation>\\*.png`,
one picture per frame, named `<name>_<animation>_<angle>_<frame>.png` (tools\\extract_assets.py of the decompiling
project writes them like that). `skin ...`: only these (keys of SKINS or ALIASES below) - default all.

What it does per skin:
1. Collects TAB's animations (Normal, Walk, Run, Attack, LoadAttack for our units, Special, Die) - 10 directions each, 18 degrees apart; the other
   half TAB draws mirrored, and so do we (`mirror` in the JSON).
2. Cuts away the empty border all frames share, makes them smaller (80 pixels per tile - less if the picture would
   get too big) and keeps at most `MAX_FRAMES` frames per animation - so all of them together fit the graphics card.
3. Colours them like TAB does in the game (its `DefaultVisualState`: saturation, contrast, brightness, hue).
4. Lays them out like assets\\README.md says (one row per direction, the frames side by side, the animations as blocks
   next to and below each other) and writes the JSON: where the feet are, how fast each animation plays (TAB's
   `TimeAni...` in Entities.tsv).
"""

import colorsys
import json
import math
import os
import re
import sys

from PIL import Image

DEFAULT_TAB = r"E:\Decoded_Software\TheyAreBillions_Decompiled\_assets"
ASSETS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
# Our units' skins go to assets\units\, the ravens and rats (only decoration) to assets\animals\, all others (the
# zombies) to assets\zombies\.
UNITS = ("ranger", "soldier", "soldier_silver", "soldier_marine", "sniper", "lucifer", "thanatos", "titan", "medic", "spearman")
ANIMALS = ("raven", "rat")

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

    def __init__(self, atlas, name, width, scaling, offset, look, times, speeds, attack_ms, frames=None, side=None, die_ms=1000,
                 load_ms=None, paint=None, size=1.0, fight_ms=None, more=None):
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
        self.load_ms = load_ms  # bringing the weapon up (the command's TimeLoad) - None: no LoadAttack taken
        self.paint = paint  # our own repaint after TAB's colours (a function frame -> frame) - None: TAB's look
        self.size = size  # drawn this much bigger than TAB draws it (a TAB figure worn by a smaller unit: below 1)
        self.fight_ms = fight_ms  # its second attack, up close (Thanatos hits with the launcher) - None: has none
        self.more = more or {}  # animations only it has: {ours: milliseconds} (the raven's take_off and fly)


# Our own armour paints (not TAB's): every pixel of one material gets a new colour from a ramp, by its brightness -
# so TAB's shading, edges and folds stay, only the material changes.

def ramp(stops, t):
    """The colour at t (0..1) on a ramp of (t, (r, g, b)) stops."""
    t = max(0.0, min(1.0, t))
    for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
        if t <= t1:
            k = (t - t0) / (t1 - t0) if t1 > t0 else 0
            return tuple(round(a + (b - a) * k) for a, b in zip(c0, c1))
    return stops[-1][1]


# Polished steel: dark, a bright band, a darker band (the horizon a shiny surface mirrors), then white highlights.
CHROME = [(0, (10, 12, 18)), (0.28, (48, 55, 68)), (0.48, (118, 126, 142)), (0.58, (84, 92, 108)),
          (0.76, (168, 176, 190)), (0.9, (222, 228, 238)), (1, (255, 255, 255))]
# Painted blue-grey armour plates (a StarCraft marine's) and the dark metal between them.
MARINE = [(0, (8, 12, 22)), (0.3, (30, 45, 78)), (0.55, (60, 88, 135)), (0.75, (105, 138, 185)),
          (0.9, (185, 205, 232)), (1, (240, 245, 252))]
GUNMETAL = [(0, (8, 9, 12)), (0.4, (32, 36, 44)), (0.8, (78, 84, 96)), (1, (140, 146, 156))]


def repaint(image, pick):
    """Every pixel through `pick(h, s, v)` (hue in degrees, saturation and brightness 0..1): a new (r, g, b), or None
    to keep it."""
    out = image.copy()
    src, dst = image.load(), out.load()
    for y in range(image.height):
        for x in range(image.width):
            r, g, b, a = src[x, y]
            if a == 0:
                continue
            h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
            new = pick(h * 360, s, v)
            if new is not None:
                dst[x, y] = new + (a,)
    return out


def silver(image):
    """TAB's soldier, its red plates polished steel (the black suit, gun and gas mask stay)."""
    return repaint(image, lambda h, s, v: ramp(CHROME, v / 0.9) if (h >= 340 or h <= 22) and s > 0.4 else None)


def marine(image):
    """TAB's Lucifer (the heavy suit) as a power-armoured marine: blue-grey plates, dark metal suit, amber visor."""
    def pick(h, s, v):
        if 35 <= h <= 70 and s > 0.6 and v > 0.5:  # visor and lights
            return 255, min(255, round(90 + 110 * v)), 30
        if (h >= 330 or h <= 50) and s > 0.08 and v > 0.04:  # all the brown: plates (pale) or suit (strong)
            return ramp(GUNMETAL, v / 0.75) if s > 0.48 else ramp(MARINE, v / 0.7)
        return None
    return repaint(image, pick)


# Our skin -> TAB's figure. Numbers from TAB's ZXGame.dxprj (templates: Width, SceneScalingFactor, SceneOffsetFactor,
# DefaultVisualState) and ZXRules (Entities: TimeAni..., WalkSpeed, RunSpeed; Commands: TimeAction). `die_ms`: its
# TimeAniDie where it isn't 1000. `load_ms`: the attack's TimeLoad - our units raise their weapon (src\sim\combat.rs).
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
    "ranger": Skin("HumansA", "Ranger", 0.45, 3.8, (0, 0.3), (1, 0.8, 1, 0), (3000, 4000, 550, 550), (0.5, 4), 1000, load_ms=500),
    "soldier": Skin("HumansA", "Soldier_A", 0.5, 1.8, (0, 0.06), (0.8, 1, 1, 0), (3000, 2000, 700, 700), (0.6, 2.4), 500, die_ms=1500, load_ms=200),
    # Our own soldier skins: the soldier in polished steel, and TAB's Lucifer suit (scaled to a soldier's size, its own
    # walk, the soldier's shot) as a power-armoured marine.
    "soldier_silver": Skin("HumansA", "Soldier_A", 0.5, 1.8, (0, 0.06), (0.8, 1, 1, 0), (3000, 2000, 700, 700), (0.6, 2.4), 500, die_ms=1500, load_ms=200, paint=silver),
    "soldier_marine": Skin("HumansB", "Lucifer", 0.8, 3.4, (-0.02, 0.31), (1, 0.8, 1, 0), (3000, 2500, 800, 800), (0.5, 1.8), 500, die_ms=1200, load_ms=200, paint=marine, size=0.8),
    "sniper": Skin("HumansA", "Sniper", 0.6, 3, (0, 0.28), (1, 0.8, 1.07, 0), (3000, 2500, 800, 800), (0.5, 1.8), 600, die_ms=1200, load_ms=500),
    # TAB's late units (no DefaultVisualState in their templates: TAB's colours unchanged). Thanatos: `attack` = the
    # rocket (ThanatosExtraAttack), `fight` = the blow up close (ThanatosAttack). Titan only runs (no Walk).
    "lucifer": Skin("HumansB", "Lucifer", 0.8, 3.4, (-0.02, 0.31), (1, 1, 1, 0), (3000, 2500, 800, 800), (0.5, 1.8), 500, die_ms=1200, load_ms=500),
    "thanatos": Skin("HumansC", "Thanatos", 0.7, 4.6, (0, 0.335), (1, 1, 1, 0), (3000, 2500, 800, 800), (0.5, 1.8), 600, die_ms=800, load_ms=1500, fight_ms=1000),
    "titan": Skin("BigUnitsA", "Titan", 0.9, 5.7, (-0.015, 0.35), (1, 1, 1, 0), (2000, 3000, 800, 800), (0.8, 3), 200, die_ms=800, load_ms=500),
    # Decoration (docs\tab-assets.md step 8): TAB's ravens (they hop, take off when someone comes, fly away - TimeAniFly,
    # TimeAniPrepareFly) and rats. Width = their Size 0.25.
    "raven": Skin("AnimalsA", "Raven", 0.25, 4.5, (-0.01, 0.35), (1, 2, 0.97, 0), (3000, 2000, 300, 2000), (0.75, 6), 1000,
                  more={"take_off": 500, "fly": 500}),
    "rat": Skin("AnimalsB", "Rat", 0.25, 2, (0, 0.28), (1, 0.6, 0.95, 0), (2000, 3000, 300, 150), (0.75, 2), 1000),
}

# Our units without a TAB figure wear one of the skins above for now - the same picture, a JSON of their own without
# `attack` and `load` (the medic's beam and the spear would look like a gunshot): they stand while they work.
ALIASES = {"medic": "soldier_silver", "spearman": "soldier_marine"}

# Our animation -> TAB's.
# "load" (bringing the weapon up) only for skins with a `load_ms`.
# "fight" (a second attack up close) only for skins with a `fight_ms`.
# "take_off", "fly" only for skins that list them in `more` (the raven).
ANIMATIONS = [("idle", "Normal"), ("walk", "Walk"), ("run", "Run"), ("attack", "Attack"), ("load", "LoadAttack"),
              ("fight", "Fight"), ("special", "Special"), ("die", "Die"), ("take_off", "PrepareFly"), ("fly", "Fly")]


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


def _product(a, b):
    """4 x 4 matrices multiplied, a · b (TAB's `DXHelper_Matrix.MatrixProduct`)."""
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def _identity():
    return [[1.0 if i == j else 0.0 for j in range(4)] for i in range(4)]


def _rotate(m, axis, s, c, left=False):
    """TAB's `XRotate`/`YRotate`/`ZRotate` (m · R) and `ZShear` (S · m, `left`)."""
    r = _identity()
    if axis == "x":
        r[1][1], r[1][2], r[2][1], r[2][2] = c, s, -s, c
    elif axis == "y":
        r[0][0], r[0][2], r[2][0], r[2][2] = c, -s, s, c
    elif axis == "z":
        r[0][0], r[0][1], r[1][0], r[1][1] = c, s, -s, c
    else:  # shear: s, c = dx, dy
        r[0][2], r[1][2] = s, c
        return _product(r, m)
    return _product(m, r)


def colour_matrix(look, tint=None):
    """TAB's colour matrix (`DXObjectState.UpdateColorMatrix`), number for number. Row i: how much of channel i (red,
    green, blue, alpha) goes into each channel. What it does: brightness b is added (+ b - 1, not multiplied), then each
    channel times the colour `tint`, then saturation (towards grey, TAB's eye weights), then times the contrast - TAB's
    contrast scales, it doesn't spread around the middle. A hue rotation turns the colour first (Haeberli's rotation
    that keeps the brightness)."""
    saturation, contrast, brightness, hue = look
    r, g, bl = (c / 255 for c in tint) if tint else (1.0, 1.0, 1.0)
    s, c, n = saturation, contrast, brightness - 1.0
    gr, gg, gb = (0.3086 * (1 - s), 0.6094 * (1 - s), 0.082 * (1 - s)) if s != 1 else (0.0, 0.0, 0.0)
    m = [
        [c * r * (gr + s), c * r * gr, c * r * gr, 0.0],
        [c * g * gg, c * g * (gg + s), c * g * gg, 0.0],
        [bl * c * gb, bl * c * gb, bl * c * (gb + s), 0.0],
        [c * r * n * (gr + s) + bl * c * n * gb + c * g * n * gg,
         c * g * n * (gg + s) + bl * c * n * gb + c * r * n * gr,
         bl * c * n * (gb + s) + c * g * n * gg + c * r * n * gr,
         1.0],
    ]
    if hue:
        turn = _rotate(_identity(), "x", 1 / math.sqrt(2), 1 / math.sqrt(2))
        ys, yc = -1 / math.sqrt(3), math.sqrt(2) / math.sqrt(3)
        turn = _rotate(turn, "y", ys, yc)
        rot = _product(_identity(), turn)
        # Where the grey axis (TAB's eye weights) lands - sheared back onto the z axis, turned about it, sheared back.
        p = [sum(w * rot[i][j] for i, w in enumerate((0.3086, 0.6094, 0.082))) + rot[3][j] for j in range(3)]
        zx, zy = -p[0] / p[2], -p[1] / p[2]
        rot = _rotate(rot, "shear", zx, zy)
        rot = _rotate(rot, "z", math.sin(math.radians(hue)), math.cos(math.radians(hue)))
        rot = _rotate(rot, "shear", -zx, -zy)
        rot = _rotate(rot, "y", -ys, yc)
        rot = _rotate(rot, "x", -1 / math.sqrt(2), 1 / math.sqrt(2))
        m = _product(rot, m)
    return m


def colour(image, look, tint=None):
    """TAB's DefaultVisualState on a picture (`colour_matrix`): a pixel (r, g, b, 1) times the matrix, like TAB's
    shader (`mul(T, ColorMatrix)`)."""
    m = colour_matrix(look, tint)
    alpha = image.getchannel("A")
    # PIL: out = a·r + b·g + c·b + d per channel, d in 0..255.
    affine = tuple(v for j in range(3) for v in (m[0][j], m[1][j], m[2][j], m[3][j] * 255))
    rgb = image.convert("RGB").convert("RGB", affine)
    rgb.putalpha(alpha)
    return rgb


def folder_of(key):
    """Where a skin goes: assets\\units\\ or assets\\zombies\\ (made if it isn't there)."""
    folder = os.path.join(ASSETS, "units" if key in UNITS else "animals" if key in ANIMALS else "zombies")
    os.makedirs(folder, exist_ok=True)
    return folder


def convert(tab, key, skin):
    folder = os.path.join(tab, "Sprites", skin.atlas)
    # Per animation: per row the frame paths (missing angles: the nearest one).
    animations = []
    for ours, theirs in ANIMATIONS:
        if (ours == "load" and skin.load_ms is None) or (ours == "fight" and skin.fight_ms is None):
            continue
        if ours in ("take_off", "fly") and ours not in skin.more:
            continue
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
    scale = min(1.0, PIXELS_PER_TILE * skin.size / tab_pixels_per_tile)
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
        "pixels_per_tile": round(tab_pixels_per_tile * scale / skin.size, 2),
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
                frame = colour(frame, skin.look)
                if skin.paint:
                    frame = skin.paint(frame)
                sheet.paste(frame, (left + c * fw + PAD, top + r * fh + PAD))
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
        elif ours == "load":
            animation["fps"] = round(n / (skin.load_ms / 1000), 2)
        elif ours == "fight":
            animation["fps"] = round(n / (skin.fight_ms / 1000), 2)
        elif ours == "special":
            animation["fps"] = round(n / (special_ms / 1000), 2)
        elif ours == "die":
            animation["fps"] = round(n / (skin.die_ms / 1000), 2)
        elif ours in skin.more:
            animation["fps"] = round(n / (skin.more[ours] / 1000), 2)
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


def alias(key, of):
    """`key`.json: the skin `of` (its picture, already made) without its attack - see ALIASES."""
    folder = folder_of(of)
    source = os.path.join(folder, f"{of}.json")
    if not os.path.isfile(source):
        print(f"  {key}: {of} isn't made yet - left out")
        return
    with open(source, encoding="utf-8") as f:
        meta = json.load(f)
    for animation in ("attack", "load", "fight"):
        meta.pop(animation, None)
    with open(os.path.join(folder_of(key), f"{key}.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2)
    print(f"  {key}: {of}.png without attack")


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    if not os.path.isdir(os.path.join(tab, "Sprites")):
        sys.exit(f"No Sprites folder in {tab} - give the _assets folder of the extracted game.")
    print(f"From {tab} to {os.path.abspath(ASSETS)}:")
    wanted = sys.argv[2:]
    for key, skin in SKINS.items():
        if not wanted or key in wanted:
            convert(tab, key, skin)
    for key, of in ALIASES.items():
        if not wanted or key in wanted or of in wanted:
            alias(key, of)


if __name__ == "__main__":
    main()
