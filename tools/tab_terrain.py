"""Turns TAB's landscape pictures into ours: assets\\tab\\terrain\\<landscape>\\ (explained in assets\\README.md).

TAB's pictures are copyrighted - only for us, on this PC (assets\\tab\\ is in .gitignore). This script makes them anew
from your own copy of the game:

    python tools\\tab_terrain.py [assets folder] [game folder]

`assets folder`: the `_assets` folder of the extracted game (its `Images\\...` as single PNGs, like for tab_skins.py).
`game folder`: where TAB is installed - its `ZXGame.dxprj` (a zip with one big XML: every template of the game) says
which picture each thing uses, how big it is and how TAB colours it.

What TAB's map drawer does (`ZX.GameSystems\\ZXMapDrawer.cs`, `ZXMapTheme*.cs`) and what we take from it, per landscape:
1. The ground: one earth texture over the whole map, the grass texture over it where there's grass (soft at the edge,
   TAB's mask), water below where there's water. Each texture repeats every so many tiles (its `TileSize`).
2. Mountains: a rock piece on every 2 x 2 block, bigger deep inside a range (BR, TM, AL, DS: the same piece scaled,
   FA, VO: a taller piece per height).
3. Trees: one per forest tile, picked by weight - other trees on grass than on bare earth; deep in a forest bigger.
4. Herbs on the grass, and the ore lumps: four small ones per ore tile. An oil source on every 2 x 2 oil node.
   Coal TAB doesn't have: ours, its stone lumps blackened and broken (`coal_lump`).
5. The big rocks (`BRMountainC2/C3/C4`, 3 x 3, 4 x 4 and 5 x 5 tiles, three of each): TAB's generator never puts them
   down, only its hand-made maps - ours in the editor. Coloured like the landscape's mountains (`geology`, TAB's
   `SetGeologyStyle`).
6. The floor under a building (`ZX.Components\\CFloorUndeBuilding.cs`): TAB's stone pavement `Floor A`, coloured like the
   landscape's `FloorBuilding_*` template, repeating every 8 tiles - and its three soft masks (`MasksTerrainSmall`,
   `MaskTerrainMedium`, `TerrainMaskLarge`, for buildings 1, 2 and 3+ tiles wide) side by side in one picture.
Every picture coloured like TAB's `DefaultVisualState` (saturation, contrast, brightness, hue; a tree's random hue as
three variants). The water is a picture TAB composes of layers (`Clips`) - here mixed once into one texture, close to
it (the recipes below, read from the clips by hand).
"""

import base64
import json
import math
import os
import random
import sys
import xml.etree.ElementTree as ET
import zipfile

from PIL import Image, ImageChops, ImageEnhance

from tab_skins import colour, layout

DEFAULT_TAB = r"E:\Decoded_Software\TheyAreBillions_Decompiled\_assets"
DEFAULT_GAME = r"C:\SteamLibrary\steamapps\common\They Are Billions"
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "terrain")

# Pixels per tile width the pictures on the ground get (trees, rocks, herbs - TAB's are bigger, we shrink them). Twice
# what a tile is at zoom 1: inside a forest or a mountain range they're drawn up to 2.5 times as big.
PIXELS_PER_TILE = 160
# Ground textures at most this big (TAB's are up to 4096).
MAX_TEXTURE = 2048
# Transparent pixels around every picture in the atlas.
PAD = 2

# The six landscapes (`ZXMapTheme` and its subclasses): which templates they use. Weights like TAB's dictionaries.
BR_TREES = {"Tree_A": 10, "Tree_B": 10, "Tree_C": 10}
THEMES = {
    "BR": dict(earth="TerrainEarth_BR", grass="TerrainGrass_BR", herb="Grass_BR", pieces=["BRPiece1"], scale_pieces=True,
               trees_dirt=BR_TREES, trees_grass=dict(BR_TREES, Tree_D=100), max_tree=2.0),
    "TM": dict(earth="TerrainEarth_TM", grass="TerrainGrass_TM", herb="Grass_TM", pieces=["TMPiece1"], scale_pieces=True,
               trees_dirt={f"TreeSmall{c}": 30 for c in "ABCD"}, trees_grass={f"TreeSmall{c}": 30 for c in "ABCD"},
               max_tree=1.5),
    "AL": dict(earth="TerrainSnowA", grass="TerrainGrass_AL", herb="Grass_AL", pieces=["ALPiece1"], scale_pieces=True,
               trees_dirt=dict({f"TreeSnow{c}": 30 for c in "ABCD"}, TreeSnowE=50), trees_grass=dict(BR_TREES, Tree_D=100),
               max_tree=2.0),
    "DS": dict(earth="TerrainEarth_DS", grass="TerrainGrass_DS", herb="Grass_DS", pieces=["DSPiece1"], scale_pieces=True,
               trees_dirt={"TreeDesert_A": 30, "TreeDesert_B": 30, "TreeDesert_C": 30},
               trees_grass={"TreeDesert_A": 30, "TreeDesert_B": 30, "TreeDesert_C": 30, "TreeDesert_D": 5, "TreeDesert_G": 4},
               max_tree=1.3),
    "FA": dict(earth="TerrainEarth_FA", grass="TerrainGrass_FA", herb="Grass_FA",
               pieces=["FAPiece1", "FAPiece2", "FAPiece3", "FAPiece4", "FAPiece3"], scale_pieces=False,
               trees_dirt={"TreeFall_A": 30, "TreeFall_B": 30, "TreeFall_C": 10, "TreeFall_D": 10, "TreeFall_E": 10,
                           "TreeFallDead_A": 100, "TreeFallDead_B": 50, "TreeFallDead_C": 50, "FARock": 30},
               trees_grass={"TreeFall_A": 30, "TreeFall_B": 30, "TreeFall_C": 80, "TreeFall_D": 30, "TreeFall_E": 30,
                            "TreeFallDead_A": 5, "TreeFallDead_B": 10, "TreeFallDead_C": 10, "FARock": 30},
               max_tree=1.4),
    # VO's fifth height is a volcano (an animation) - its fourth piece instead.
    "VO": dict(earth="TerrainEarth_VO", grass="TerrainGrass_VO", herb="Grass_FA",
               pieces=["VOPiece1", "VOPiece2", "VOPiece3", "VOPiece4", "VOPiece4"], scale_pieces=False,
               trees_dirt={**{f"VODeadTree{c}": 10 for c in "ABCDE"}, "VODeadTreeF": 3, "VODeadTreeG": 5,
                           "VODeadTreeH": 5, "VODeadTreeI": 5, "VORock": 3},
               trees_grass={**{f"VODeadTree{c}": 10 for c in "ABCDE"}, "VODeadTreeF": 3, "VODeadTreeG": 20,
                            "VODeadTreeH": 20, "VODeadTreeI": 20, "VORock": 2},
               max_tree=1.8),
}
ORES = {"stone": "MineralStoneA", "iron": "MineralIronA", "gold": "MineralGoldA"}
# The big rocks per size (tiles): TAB's templates, three pictures each.
BIG_ROCKS = {3: "BRMountainC2_", 4: "BRMountainC3_", 5: "BRMountainC4_"}
# How a landscape colours the big rocks (`ZXMapTheme*.SetGeologyStyle`): (saturation, contrast, brightness, hue) - it
# replaces the template's own. FA and VO don't change it: the template's (1, 0.9, 1, 15).
GEOLOGY = {"BR": (0.5, 0.55, 1.05, 0), "AL": (0.2, 0.9, 1.07, 200), "DS": (1.5, 0.55, 1.1, 0), "TM": (1.0, 0.9, 1.0, 15)}

# The water: TAB composes it of layers (its clips `Water_BR`, `Water_AL` ...): the earth picture tinted, a water picture
# added on top, animated. Per layer here: picture, repeats every so many tiles, (saturation, contrast, brightness,
# hue), colour multiplied (r, g, b), how it's put on ("normal" / "add"), opacity. `period`: the texture covers this many
# tiles. The numbers are TAB's (BR's and AL's were by eye once, while our colouring spread the contrast around the
# middle grey - with TAB's own colouring, `colour_matrix`, TAB's numbers fit). TM and DS add an animation we don't
# have - Water_E instead. VO's darkening layer (`Square_Black`, blend "Darkness") is left out.
WATER = {
    "BR": dict(period=30, layers=[("Terrains/TerrainEarth_A.png", 30, (10, 0.4, 1, 0), (50, 140, 220), "normal", 1),
                                  ("Water/Water_G.png", 10, (1, 0.08, 1.6, 0), None, "add", 1)]),
    "AL": dict(period=30, layers=[("Terrains/TerrainEarth_A.png", 30, (0.3, 1, 1.13, 0), (30, 144, 255), "normal", 1),
                                  ("Water/Water_D.png", 15, (0.1, 0.6, 0.2, 0), None, "add", 1)]),
    "VO": dict(period=30, layers=[("Terrains/VOEarthTerrain.png", 30, (0, 1, 0.9, 150), None, "normal", 1),
                                  ("Water/Water_A.png", 5, (1, 1, 1, 0), (46, 139, 87), "add", 0.3),
                                  ("Water/Water_A.png", 30, (1, 1, 1, 0), (102, 205, 170), "add", 0.6)]),
    "TM": dict(period=10, layers=[("Terrains/TerrainEarth_A.png", 5, (0.4, 1, 0.7, -20), (60, 140, 200), "normal", 1),
                                  ("Water/Water_E.png", 10, (1, 1, 1, 0), None, "add", 0.3)]),
}
# The floor under the buildings: the template per landscape (`ZXMapTheme*.TemplateFloorBuilding`) - FA and VO keep
# the default (BR's). Its masks by the building's smaller side: 1, 2, 3 and more tiles.
FLOORS = {"BR": "FloorBuilding_BR", "AL": "FloorBuilding_AL", "DS": "FloorBuilding_DS", "TM": "FloorBuilding_TM",
          "FA": "FloorBuilding_BR", "VO": "FloorBuilding_BR"}
FLOOR_MASKS = ["Masks/MasksTerrainSmall.png", "Masks/MaskTerrainMedium.png", "Masks/TerrainMaskLarge.png"]
FLOOR_MASK_SIZE = 512

WATER["DS"] = WATER["TM"]
WATER["FA"] = WATER["BR"]  # FA keeps the default (BR) water


def read_project(game):
    """TAB's templates by name and its pictures by number, out of ZXGame.dxprj."""
    with zipfile.ZipFile(os.path.join(game, "ZXGame.dxprj")) as z:
        root = ET.fromstring(z.read(z.namelist()[0]))
    images, templates = {}, {}
    for c in root.iter("Complex"):
        p = c.find("Properties")
        if p is None:
            continue
        vals = {s.get("name"): s.get("value") for s in p if s.tag == "Simple"}
        if "FileName" in vals and "ID" in vals and "IDProject" in vals:
            images[vals["ID"]] = vals["FileName"]
        if "ClassName" in vals and "Name" in vals:
            templates.setdefault(vals["Name"], c)
    return images, templates


def template(images, templates, name):
    """What we need of a template: its picture(s), size, scaling, offset and colouring."""
    c = templates[name]
    out = {"name": name, "size": 1.0, "scaling": 1.0, "offset": 0.0, "look": (1, 1, 1, 0), "hues": (0, 0), "tint": None,
           "tiles": None, "image": None, "random": None}
    for cfg in c.iter("Complex"):
        kind = (cfg.get("type") or "").split(",")[0].split(".")[-1]
        props = cfg.find("Properties")
        if props is None:
            continue
        d = {s.get("name"): s.get("value") for s in props if s.tag == "Simple"}
        num = lambda k, default: float(d.get(k, default).replace(",", "."))
        if kind == "DXComponentImage+TTemplateConfiguration":
            out["image"] = images.get(d.get("IDImage"))
            out["scaling"] = num("SceneScalingFactor", "1")
            out["offset"] = num("SceneOffsetFactorY", "0")
            # Its random pictures can be named right here (FA's and VO's mountain pieces: `VOPiece1` = VOPiece1A-F).
            if d.get("RandomImageName"):
                out["random"] = d["RandomImageName"]
        elif kind == "DXComponentDefaultVisualState+TTemplateConfiguration":
            out["look"] = (num("Saturation", "1"), num("Contrast", "1"), num("Brightness", "1"), num("HueRotation", "0"))
            lo, hi = d.get("RandomHueRotation", "0;0").split(";")
            out["hues"] = (float(lo), float(hi))
            out["tiles"] = float(d.get("TileSize", "1;1").split(";")[0])
            hue = d.get("ColorHue")
            if hue and hue != "/////w==":
                a, r, g, b = base64.b64decode(hue)
                out["tint"] = (r, g, b)
        elif kind.endswith("RandomImageTemplateConfiguration"):
            out["random"] = d.get("ImageName")
    sizes = [s.get("value") for s in c.iter("Simple") if s.get("name") == "Size"]
    if sizes:
        out["size"] = float(sizes[-1].split(";")[0].replace(",", "."))
    return out


def picture_files(tab, t):
    """The PNG(s) of a template in the extracted game: its picture - or all of its random ones (`BRPiece1_A` ...)."""
    images = os.path.join(tab, "Images")
    if t["random"]:
        folder = os.path.dirname(t["image"].replace("\\", "/")).split("Images/", 1)[1]
        found = []
        for entry in sorted(os.listdir(os.path.join(images, folder))):
            stem = os.path.splitext(entry)[0]
            if stem.startswith(t["random"]) and len(stem) <= len(t["random"]) + 2:
                found.append(single(images, folder, stem))
        return [f for f in found if f]
    rel = t["image"].replace("\\", "/").split("Images/", 1)[1]
    folder, file = os.path.split(rel)
    return [f for f in [single(images, folder, os.path.splitext(file)[0])] if f]


def single(images, folder, stem):
    """`Images\\<folder>\\<stem>.png` - or in its own folder, `Images\\<folder>\\<stem>\\<stem>.png`."""
    for path in (os.path.join(images, folder, stem + ".png"), os.path.join(images, folder, stem, stem + ".png"),
                 os.path.join(images, folder, stem + ".jpg")):
        if os.path.isfile(path):
            return path
    return None


def tinted(image, look, tint, hue=0.0):
    """TAB's colouring (`colour_matrix`): hue (plus a random turn), saturation, contrast, brightness and the colour
    `tint` (TAB's `ColorHue`) - all in one matrix, like TAB."""
    s, c, b, h = look
    return colour(image.convert("RGBA"), (s, c, b, h + hue), tint)


def texture(tab, t, out):
    """A ground texture: TAB's picture coloured, at most MAX_TEXTURE big. Returns how many tiles it covers."""
    path = picture_files(tab, t)[0]
    image = tinted(Image.open(path), t["look"], t["tint"])
    if image.width > MAX_TEXTURE:
        image = image.resize((MAX_TEXTURE, MAX_TEXTURE), Image.LANCZOS)
    image.convert("RGB").save(out, optimize=True)
    return t["tiles"]


def water(tab, recipe, out):
    """The water texture: TAB's clip layers mixed into one picture covering `period` tiles."""
    period = recipe["period"]
    side = min(MAX_TEXTURE, period * 64)
    canvas = Image.new("RGB", (side, side), (0, 0, 0))
    for path, tiles, look, tint, mode, opacity in recipe["layers"]:
        layer = tinted(Image.open(os.path.join(tab, "Images", path)), look, tint).convert("RGB")
        size = max(1, round(side * tiles / period))
        layer = layer.resize((size, size), Image.LANCZOS)
        tiled = Image.new("RGB", (side, side))
        for y in range(0, side, size):
            for x in range(0, side, size):
                tiled.paste(layer, (x, y))
        if mode == "add":
            canvas = ImageChops.add(canvas, ImageEnhance.Brightness(tiled).enhance(opacity))
        else:
            canvas = Image.blend(canvas, tiled, opacity)
    canvas.save(out, optimize=True)
    return period


class Atlas:
    """The pictures standing on the ground (trees, rocks, herbs, ore), each cut to what's in it and made smaller - all
    in one picture, so they're drawn without switching textures."""

    def __init__(self):
        self.pictures = []  # (image, width in tiles, offset share, feet share)

    def add(self, image, width_tiles, offset):
        box = image.getchannel("A").getbbox() or (0, 0, image.width, image.height)
        # The bottom middle of TAB's whole frame stands on the ground; where is that in the cut picture?
        full_w, full_h = image.size
        scale = min(1.0, PIXELS_PER_TILE * width_tiles / full_w)
        cut = image.crop(box)
        w, h = max(1, round(cut.width * scale)), max(1, round(cut.height * scale))
        cut = cut.convert("RGBa").resize((w, h), Image.LANCZOS).convert("RGBA")
        # The feet: the frame's bottom middle - TAB moves the frame down by `offset` of its height (`DXComponentVisual`:
        # `bottomCenter.Y += SceneOffsetFactorY * height`), so the feet are that much higher in it. In the cut
        # picture's pixels.
        feet_x = (full_w / 2 - box[0]) * scale
        feet_y = (full_h * (1 - offset) - box[1]) * scale
        self.pictures.append({"image": cut, "feet": (feet_x, feet_y), "pixels_per_tile": full_w * scale / width_tiles})
        return len(self.pictures) - 1

    def add_placed(self, image, feet, pixels_per_tile):
        """A picture already at its size, its feet at `feet` (pixels) - cut to what's in it."""
        box = image.getchannel("A").getbbox() or (0, 0, image.width, image.height)
        feet = (feet[0] - box[0], feet[1] - box[1])
        self.pictures.append({"image": image.crop(box), "feet": feet, "pixels_per_tile": pixels_per_tile})
        return len(self.pictures) - 1

    def save(self, out):
        blocks = [(p["image"].width + 2 * PAD, p["image"].height + 2 * PAD) for p in self.pictures]
        # Tall pictures first fill the shelves better - but the numbers stay.
        order = sorted(range(len(blocks)), key=lambda i: -blocks[i][1])
        places, width, height = layout([blocks[i] for i in order])
        sheet = Image.new("RGBA", (width, height), (0, 0, 0, 0))
        meta = [None] * len(blocks)
        for i, (x, y) in zip(order, places):
            p = self.pictures[i]
            sheet.paste(p["image"], (x + PAD, y + PAD))
            w, h = p["image"].size
            meta[i] = {"at": [x + PAD, y + PAD, w, h], "feet": [round(p["feet"][0], 1), round(p["feet"][1], 1)],
                       "pixels_per_tile": round(p["pixels_per_tile"], 2)}
        sheet.save(out, optimize=True)
        return meta, (width, height)


def objects(tab, images, templates, atlas, name, hue_variants=True):
    """The pictures of one template in the atlas: each of its random pictures, and for a random hue three variants."""
    t = template(images, templates, name)
    lo, hi = t["hues"]
    hues = sorted({lo, (lo + hi) / 2, hi}) if hue_variants and lo != hi else [(lo + hi) / 2]
    found = []
    for path in picture_files(tab, t):
        base = Image.open(path).convert("RGBA")
        for hue in hues:
            found.append(atlas.add(tinted(base, t["look"], t["tint"], hue), t["size"] * t["scaling"], t["offset"]))
    if not found:
        print(f"    {name}: no picture found - left out")
    return found


# TAB's oil source (`Geology.OilSource`, 2 x 2 tiles, one per `NCellsPerOilSource` block) is a clip: on a canvas of
# 400 x 400 units the pool (`OilLiquidA`) and over it the well (`Oil_A`) - each with its area (x, y, width, height in
# those units) - and smoke rising (an animation, left out). The canvas is the 2 x 2 at `SceneScalingFactor` 0.7, moved
# by its `SceneOffsetFactor` (-0.05, 0.01).
OIL_CANVAS = 400
OIL_LAYERS = [("Geology/OilLiquidA/OilLiquidA.png", (-47, -118, 537, 537)),
              ("Geology/Oil_A/Oil_A.png", (-27.85, -20.04, 503.85, 418.04))]
OIL_WIDTH = 2 * 0.7  # tile widths the canvas is wide
OIL_OFFSET = (-0.05, 0.01)


def oil(tab, atlas):
    """TAB's oil source as one picture: its layers put together the way its clip has them."""
    k = PIXELS_PER_TILE * OIL_WIDTH / OIL_CANVAS  # pixels per unit
    left, top = min(a[0] for _, a in OIL_LAYERS), min(a[1] for _, a in OIL_LAYERS)
    right = max(a[0] + a[2] for _, a in OIL_LAYERS)
    bottom = max(a[1] + a[3] for _, a in OIL_LAYERS)
    canvas = Image.new("RGBA", (round((right - left) * k), round((bottom - top) * k)), (0, 0, 0, 0))
    for path, (x, y, w, h) in OIL_LAYERS:
        full = os.path.join(tab, "Images", path)
        if not os.path.isfile(full):
            print(f"    oil: {path} not found - left out")
            return []
        layer = Image.open(full).convert("RGBa").resize((round(w * k), round(h * k)), Image.LANCZOS).convert("RGBA")
        canvas.alpha_composite(layer, (round((x - left) * k), round((y - top) * k)))
    # The canvas' bottom middle stands on the ground - moved by the offset, so the feet are the other way in it.
    feet = ((OIL_CANVAS / 2 - OIL_OFFSET[0] * OIL_CANVAS - left) * k, (OIL_CANVAS * (1 - OIL_OFFSET[1]) - top) * k)
    return [atlas.add_placed(canvas, feet, PIXELS_PER_TILE)]


# Coal TAB doesn't have - ours is made of its stone lumps (`MineralStoneB` ... `E`): black with a glossy edge, cracked,
# with chipped edges and crumbs around it.
COAL_FROM = "MineralStone"
COAL_VARIANTS = "BCDE"


def coal_colour(lum):
    """Grey (0-255) -> coal: nearly black, only the light rims of the stone a cold glossy grey."""
    t = (lum / 255) ** 2.6
    return (int(8 + 105 * t), int(8 + 108 * t), int(10 + 120 * t))


def coal_lump(stone, seed):
    """One of TAB's stone lumps turned into coal - deterministic for `seed`."""
    from PIL import ImageDraw, ImageFilter
    rnd = random.Random(seed)
    image = stone.convert("RGBA")
    w, h = image.size
    grey = image.convert("L")
    # Stretch the stone's brightness first, so its rims stay visible after the darkening.
    lo, hi = grey.getextrema()
    grey = grey.point(lambda v: max(0, min(255, (v - lo) * 255 // max(1, hi - lo))))
    lut = [coal_colour(v) for v in range(256)]
    rgb = Image.merge("RGB", [grey.point([c[i] for c in lut]) for i in range(3)])
    alpha = image.getchannel("A")
    solid = alpha.load()
    paint = ImageDraw.Draw(rgb)
    inside = [(x, y) for y in range(2, h - 2, 3) for x in range(2, w - 2, 3) if solid[x, y] > 230]
    rim = [(x, y) for (x, y) in inside if min(solid[max(0, x - 7), y], solid[min(w - 1, x + 7), y],
                                               solid[x, max(0, y - 5)], solid[x, min(h - 1, y + 5)]) < 60]

    # Fractures: short zigzag lines through the lumps - black, the edge below them catching the light.
    for _ in range(len(inside) // 150 + 5):
        x, y = rnd.choice(inside)
        angle = rnd.uniform(0, 6.283)
        points = [(x, y)]
        for _ in range(rnd.randint(2, 5)):
            angle += rnd.uniform(-0.8, 0.8)
            step = rnd.uniform(5, 11)
            x, y = x + step * math.cos(angle), y + step * math.sin(angle)
            points.append((x, y))
        width = rnd.choice((2, 2, 3))
        paint.line([(px, py + width) for px, py in points], fill=coal_colour(rnd.randint(150, 200)), width=1)
        paint.line(points, fill=coal_colour(0), width=width)

    # Chipped edges: notches in the rim of the lumps, in their own shadow.
    notches = Image.new("L", (w, h), 255)
    notch = ImageDraw.Draw(notches)
    for _ in range(len(rim) // 8):
        x, y = rnd.choice(rim)
        r = rnd.uniform(3, 6)
        corners = [(x + r * rnd.uniform(0.5, 1.2) * math.cos(a), y + r * rnd.uniform(0.5, 1.2) * math.sin(a))
                   for a in sorted(rnd.uniform(0, 6.283) for _ in range(5))]
        notch.polygon(corners, fill=0)
        paint.polygon(corners, fill=(0, 0, 0))
    alpha = ImageChops.lighter(ImageChops.multiply(alpha, notches),
                               ImageChops.darker(alpha, ImageChops.invert(notches).point(lambda v: v * 235 // 255)))

    # Crumbs: small splinters around the lumps, each with a glossy top edge and a soft shadow.
    crumbs = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    shadow = Image.new("L", (w, h), 0)
    draw, shade = ImageDraw.Draw(crumbs), ImageDraw.Draw(shadow)
    for _ in range(rnd.randint(16, 24)):
        x, y = rnd.choice(rim)
        a = rnd.uniform(0, 6.283)
        d = rnd.uniform(8, 24)
        x, y = x + d * math.cos(a), y + d * math.sin(a) * 0.6
        if not (6 < x < w - 6 and 6 < y < h - 6):
            continue
        r = rnd.uniform(3.5, 8.0)
        corners = [(x + r * rnd.uniform(0.6, 1.3) * math.cos(b), y + r * rnd.uniform(0.4, 0.9) * math.sin(b))
                   for b in sorted(rnd.uniform(0, 6.283) for _ in range(rnd.randint(4, 6)))]
        shade.polygon([(px + 1.5, py + 2.5) for px, py in corners], fill=160)
        draw.polygon(corners, fill=coal_colour(rnd.randint(30, 90)) + (255,))
        top = min(corners, key=lambda p: p[1])
        side = min(corners, key=lambda p: p[0])
        draw.line([side, top], fill=coal_colour(rnd.randint(180, 240)) + (255,), width=1)
    shadow = shadow.filter(ImageFilter.GaussianBlur(1.5))

    out = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    out.alpha_composite(Image.merge("RGBA", (*Image.new("RGB", (w, h), (0, 0, 0)).split(), shadow)))
    lumps = rgb.copy()
    lumps.putalpha(alpha)
    out.alpha_composite(lumps)
    out.alpha_composite(crumbs)
    return out


def coal(tab, images, templates, atlas):
    """Our coal lumps in the atlas - sized and placed like TAB's stone ones."""
    t = template(images, templates, COAL_FROM + "A")
    found = []
    for k, letter in enumerate(COAL_VARIANTS):
        path = single(os.path.join(tab, "Images"), "Geology", f"{COAL_FROM}{letter}")
        if path:
            lump = coal_lump(Image.open(path), 1000 + k)
            found.append(atlas.add(lump, t["size"] * t["scaling"], t["offset"]))
    if not found:
        print("    coal: no stone picture found - left out")
    return found


def big_rocks(tab, images, templates, atlas, key):
    """The big rocks in the atlas: per size (3, 4, 5 tiles) its pictures, coloured for the landscape."""
    out = []
    for size, stem in BIG_ROCKS.items():
        found = []
        for letter in "ABC":
            t = template(images, templates, stem + letter)
            for path in picture_files(tab, t):
                look = GEOLOGY.get(key, t["look"])
                found.append(atlas.add(tinted(Image.open(path), look, None), t["size"] * t["scaling"], t["offset"]))
        if not found:
            print(f"    big rocks {size} x {size}: no picture found - left out")
        out.append(found)
    return out


def weighted(tab, images, templates, atlas, weights, cache):
    """A tree dictionary of TAB's -> [[picture, weight] ...]: a template's weight split over its pictures."""
    out = []
    for name, weight in weights.items():
        if weight <= 0:
            continue
        if name not in cache:
            cache[name] = objects(tab, images, templates, atlas, name)
        pics = cache[name]
        for pic in pics:
            out.append([pic, round(weight / len(pics), 3)])
    return out


def floor_masks(tab, out):
    """TAB's three floor masks (grey: how much floor shows) side by side, each FLOOR_MASK_SIZE big - one picture, so
    the game can pick one per building without switching textures."""
    side = FLOOR_MASK_SIZE
    sheet = Image.new("L", (side * len(FLOOR_MASKS), side))
    for i, rel in enumerate(FLOOR_MASKS):
        mask = Image.open(os.path.join(tab, "Images", rel)).convert("L").resize((side, side), Image.LANCZOS)
        sheet.paste(mask, (i * side, 0))
    sheet.save(out, optimize=True)
    return "floor_masks.png"


def convert(tab, images, templates, key, theme):
    folder = os.path.join(OUT, key)
    os.makedirs(folder, exist_ok=True)
    meta = {}
    for layer in ("earth", "grass"):
        t = template(images, templates, theme[layer])
        meta[layer] = {"image": f"{layer}.png", "tiles": texture(tab, t, os.path.join(folder, f"{layer}.png"))}
    meta["water"] = {"image": "water.png", "tiles": water(tab, WATER[key], os.path.join(folder, "water.png"))}

    t = template(images, templates, FLOORS[key])
    meta["floor"] = {"image": "floor.png", "tiles": texture(tab, t, os.path.join(folder, "floor.png"))}
    meta["floor_masks"] = floor_masks(tab, os.path.join(folder, "floor_masks.png"))

    atlas, cache = Atlas(), {}
    meta["pieces"] = [objects(tab, images, templates, atlas, p, hue_variants=False) for p in theme["pieces"]]
    meta["scale_pieces"] = theme["scale_pieces"]
    meta["trees_dirt"] = weighted(tab, images, templates, atlas, theme["trees_dirt"], cache)
    meta["trees_grass"] = weighted(tab, images, templates, atlas, theme["trees_grass"], cache)
    meta["max_tree"] = theme["max_tree"]
    meta["herbs"] = objects(tab, images, templates, atlas, theme["herb"])
    for ore, name in ORES.items():
        meta[ore] = objects(tab, images, templates, atlas, name, hue_variants=False)
    meta["oil"] = oil(tab, atlas)
    meta["coal"] = coal(tab, images, templates, atlas)
    meta["big_rocks"] = big_rocks(tab, images, templates, atlas, key)
    meta["pictures"], size = atlas.save(os.path.join(folder, "objects.png"))
    meta["objects"] = "objects.png"
    with open(os.path.join(folder, "terrain.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=1)
    print(f"  {key}: {len(meta['pictures'])} pictures, atlas {size[0]} x {size[1]}")


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    game = sys.argv[2] if len(sys.argv) > 2 else DEFAULT_GAME
    if not os.path.isdir(os.path.join(tab, "Images")):
        sys.exit(f"No Images folder in {tab} - give the _assets folder of the extracted game.")
    if not os.path.isfile(os.path.join(game, "ZXGame.dxprj")):
        sys.exit(f"No ZXGame.dxprj in {game} - give the folder TAB is installed in.")
    print("Reading ZXGame.dxprj ...")
    images, templates = read_project(game)
    print(f"From {tab} to {os.path.abspath(OUT)}:")
    for key, theme in THEMES.items():
        convert(tab, images, templates, key, theme)


if __name__ == "__main__":
    main()
