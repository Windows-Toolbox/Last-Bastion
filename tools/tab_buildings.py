"""Turns TAB's buildings into ours: assets\\buildings\\<skin>.json + .png (explained in assets\\README.md).

TAB's pictures are copyrighted - only for us, on this PC (assets\\buildings\\ is in .gitignore). This script makes them anew
from your own copy of the game:

    python tools\\tab_buildings.py [assets folder] [game folder]

`assets folder`: the `_assets` folder of the extracted game (its `Sprites\\` and `Images\\`, like for tab_skins.py).
`game folder`: where TAB is installed - its `ZXGame.dxprj` holds the templates and clips that say how a building is
put together.

What TAB does and what we take from it:
1. A building is a *clip* (`DXProjectClip`): a canvas with layers on it - the body, its lit windows (`@Light...`),
   turning parts (`@Engine...`: the saw, the windmill's sails), flags, smoke. Each layer has its place on the canvas, its
   colouring and its blend mode (normal, or added: light). We put the layers together like TAB draws them, in their
   order (`PositionZ`).
2. Three looks per building, like TAB (`Building.OnUpdateSceneObjectParallel`, `CBuildable`):
   - working: lights on, engines and flags moving, smoke,
   - idle (switched off, a neutral one): no lights, no smoke, standing still, paler (saturation 0.5, contrast 0.8),
   - being built: TAB's green ghost (contrast 5, coloured lime, half see-through).
   A look is a list of *parts*, back to front, like TAB's layers: what stands still is put together into one picture,
   every moving layer (the saw, the sails, a flag, the smoke) is a part of its own with all of TAB's frames at its own
   speed, and every light (`BlendMode.Add`) too - the game adds it to what's below, so it glows. That way only the
   small moving bits need many frames, not the whole building.
3. Where it stands (`DXComponentVisual`, `BottomCenterToCellCorner`): the canvas is `width x SceneScalingFactor` tile
   widths wide, its bottom middle on the lowest corner of the building's site, moved by `SceneOffsetFactor` of its size.
   `width` is OUR width of the building (data\\buildings.json) - where ours is smaller than TAB's (the workshops), the
   picture is smaller too and fits the site.
4. Walls (`CWall`): a pillar on every tile, and from it a piece of wall to the neighbours (`@Left`, `@Right`,
   `@Horizontal`, `@Vertical` of the clips `WoodWalls` / `StoneWalls`, placed relative to `@Pillar`). The gate: two
   pillars and its door (clip `WoodGate` / `StoneGate`). Towers: the body and its front wall apart - the units inside
   stand between them (`Tower.cs`).
5. The ballista and the machine gun turn (`CRotatingBuilding`): their gun is a part whose frames go once round the
   circle - the game picks the one facing the target. The ballista also has its shot (`@EngineAttack`, `Ballista.cs`):
   frames per 18 degrees, played once per shot.
6. Several looks of one building (TAB's `RandomClipName`: four tents, ten stone houses ...) become several skins; the
   game gives every building one of them. The fishing hut: one per side the water lies on (`FishermanCottage.cs`).
"""

import base64
import json
import math
import os
import re
import sys
import xml.etree.ElementTree as ET
import zipfile

from PIL import Image, ImageChops, ImageStat

from tab_skins import colour, layout
from tab_terrain import DEFAULT_GAME, DEFAULT_TAB, single, tinted

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "buildings")

# Pixels per tile width (like the figures, tab_skins.py).
PIXELS_PER_TILE = 80
# A tile's height on screen: TAB's tiles are sqrt(2) times as wide as high (`DXWorldGrid.IsometricTileSize`).
TILE_HEIGHT = PIXELS_PER_TILE / 2 ** 0.5
# At most this many frames per moving part - TAB's have up to 91 (the machine gun's turn), all of them fit.
MAX_FRAMES = 96
# Transparent pixels around every picture in the sheet.
PAD = 2
SQRT2 = math.sqrt(2)
# TAB's looks when a building is switched off, and while it's being built (`CBuildable`: contrast 5, Color.Lime, opacity
# 0.5).
IDLE_LOOK = (0.5, 0.8)
GHOST_CONTRAST = 5.0
GHOST_TINT = (0, 255, 0)
GHOST_OPACITY = 0.5
# The layers only a working building shows (`Building.cs`, `CSwitchableLight.cs`).
WORKING_ONLY = ("@Light", "@Bright", "@Smoke", "@Effect")

# Our building -> TAB's template. Our own buildings TAB doesn't have get one that looks fitting: the charcoal burner
# TAB's power plant (a chimney that smokes), the munitions factory TAB's warehouse.
BUILDINGS = {
    "command_center": "CommandCenter",
    "barracks": "SoldiersCenter",
    "tent": "TentHouse",
    "sawmill": "Sawmill",
    "mine": "Quarry",
    "hunter_cottage": "HunterCottage",
    "fishing_hut": "FishermanCottage",
    "farm": "Farm",
    "windmill": "MillWood",
    "charcoal_burner": "PowerPlant",
    "wood_workshop": "WoodWorkshop",
    "tesla_tower": "EnergyWoodTower",
    "stone_workshop": "StoneWorkshop",
    "munitions_factory": "WareHouse",
    "stone_house": "StoneHouse",
    "market": "Market",
    "bank": "Bank",
    "inn": "TheInn",
    "foundry": "Foundry",
    "ballista": "Ballista",
    "machine_gun_tower": "MachineGun",
    "tower": "WatchTowerWood",
    "stone_tower": "WatchTowerStone",
}
# TAB's buildings we don't have yet (docs\\tab-assets.md steps 5-7) - at TAB's size (its `Size`), since they aren't in
# data\\buildings.json yet. The quarry looks like our mine (both TAB's `Quarry`); the power plant and the warehouse are
# also our charcoal burner and munitions factory, but there 2 tiles wide - here at TAB's 3 and 4.
TAB_BUILDINGS = {
    "wood_house": ("CottageHouse", 2),
    "power_plant": ("PowerPlant", 3),
    "oil_platform": ("OilPlatform", 2),
    "quarry": ("Quarry", 2),
    "big_quarry": ("AdvancedQuarry", 2),
    "big_farm": ("AdvancedFarm", 2),
    "big_mill": ("MillIron", 2),
    "warehouse": ("WareHouse", 4),
    "engineering_center": ("AdvancedUnitCenter", 3),
    "shocking_tower": ("ShockingTower", 2),
    "executor": ("Executor", 2),
    "radar": ("RadarTower", 1),
    "lookout_tower": ("LookoutTower", 1),
    "spire": ("TheSpire", 5),
    "academy": ("TheAcademy", 4),
    "victorious": ("TheVictorious", 4),
    "transmutator": ("TheTransmutator", 5),
    "crystal_palace": ("TheCrystalPalace", 6),
}
# Pictures, not clips: one per random picture (`DXComponentRandomImage`) - template, tiles wide.
IMAGES = {"stakes": ("TrapStakes", 1), "barrel": ("ExplosiveBarrel", 1), "blade_trap": ("TrapBlades", 1),
          "mine_trap": ("TrapMine", 1)}
# Things on the map that aren't buildings (assets\\decor\\, same JSON): template, tiles wide. Wheat on the farm's
# fields (`CFarmer`), loot (`Pickable*`), treasure ruins per landscape, TAB's power line towers in the forest
# (`ZXMapTheme.Decorate`), ruins of tents and a well.
DECOR = {
    "wheat": ("Wheat_A", 1),
    "loot_wood": ("PickableWood", 1), "loot_stone": ("PickableStone", 1), "loot_iron": ("PickableIron", 1),
    "loot_oil": ("PickableOil", 1), "loot_gold": ("PickableGold", 1), "loot_food": ("PickableFood", 1),
    "loot_energy": ("PickableEnergy", 1), "loot_science": ("PickableSciencePoint", 1),
    **{f"treasure_ruin_{t.lower()}": (f"RuinTreasureA_{t}", 3) for t in ("BR", "TM", "AL", "DS", "VO")},
    "power_tower_high": ("TensionTowerHigh", 2), "power_tower_high_flip": ("TensionTowerHighFlip", 2),
    "power_tower_medium": ("TensionTowerMedium", 2), "power_tower_medium_flip": ("TensionTowerMediumFlip", 2),
    "power_tower_low": ("TensionTowerLow", 2), "power_tower_low_flip": ("TensionTowerLowFlip", 2),
    "tent_ruin_a": ("TentRuinA", 2), "tent_ruin_a_flip": ("TentRuinAFlip", 2), "tent_ruin_b": ("TentRuinB", 2),
    "tent_ruin_c": ("TentRuinC", 2), "well_ruin": ("WellRuinA", 2),
    # The railway (tab_trains.py makes the trains): pieces of track, buffer stops, curves of 6 x 6 tiles, the arches
    # of a railway bridge (8 x 1, 1 x 8, 1 x 4), barriers, a level crossing, a trolley, mine wagons, a lamp.
    "rail_x": ("RailWayCellH", 1), "rail_y": ("RailWayCellV", 1),
    **{f"rail_end_{d.lower()}": (f"RailWayTerminator{d}", 1) for d in "NESW"},
    **{f"rail_curve_{d.lower()}": (f"RailWayArc90{d}", 6) for d in ("NE", "NW", "SE", "SW")},
    "rail_arch_a": ("RailwayArcA", (8, 1)), "rail_arch_b": ("RailwayArcB", (1, 8)), "rail_arch_c": ("RailwayArcC", (1, 4)),
    "rail_barrier_a": ("RailwayBarrierA", 1), "rail_barrier_b": ("RailwayBarrierB", 1),
    "rail_crossing": ("RailwayLevelA", 1), "rail_trolley": ("RailwayTrolleyAFlip", 1),
    "mine_wagon_a": ("MineWagonA", 1), "mine_wagon_b": ("MineWagonB", 1), "train_lamp": ("TrainLampA", 1),
}
DECOR_OUT = os.path.join(os.path.dirname(OUT), "decor")
# The fishing hut: TAB's four clips, in the order the game picks them (water above, left, below, right).
FISHING = ["FishermanCottageA", "FishermanCottageB", "FishermanCottageC", "FishermanCottageD"]
# The houses of the villages of doom (data\\doom.json, `skins`), at TAB's size.
DOOM = {"doom_small": ("DoomBuildingSmall", 2), "doom_medium": ("DoomBuildingMedium", 3), "doom_big": ("DoomBuildingLarge", 4)}
# Walls and gates: template, its clip of pieces, gate clip.
WALLS = {"wall": ("WallWood", "WoodWalls"), "stone_wall": ("WallStone", "StoneWalls")}
GATES = {"gate": ("GateWood", "Buildings/WoodPillar_B", "WoodWalls", "WoodGate"),
         "stone_gate": ("GateStone", "Buildings/StonePillar_B", "StoneWalls", "StoneGate")}
# Layers TAB hides by itself: the command center's telescope (only with a mod), the ballista shooting (only while it
# shoots - a part of its gun, see TURRETS).
HIDDEN = ("Telescope", "@EngineAttack")
# The buildings that turn (`CRotatingBuilding`): the layer of their gun, the layer of its shot (or None), how TAB picks
# the gun's frame for an angle (`Entity_EventOnUpdateSceneObjectParallel`) and the command whose `TimeAction` one shot
# takes. The ballista: frame = n * (angle - 90) / 360, a machine weapon: frame = n * (1 - angle / 360) - here as (angle
# of frame 0, +1 clockwise / -1 counter-clockwise). Angles in TAB's degrees like the figures': 0 = lower right on
# screen, 45 = down, 90 = lower left.
TURRETS = {
    "ballista": ("@EngineNormal", "@EngineAttack", 90, 1, "BallistaAttack"),
    "machine_gun_tower": ("@Engine", None, 0, -1, None),
    "executor": ("@Engine", None, 90, 1, None),
}


def num(text, default=0.0):
    return float(text.replace(",", ".")) if text not in (None, "") else default


def pair(text):
    a, b = text.split(";")
    return num(a), num(b)


def kind(e):
    return (e.get("type") or "").split(",")[0].split(".")[-1]


def props(e):
    p = e.find("Properties")
    return {} if p is None else {s.get("name"): s.get("value") for s in p if s.tag == "Simple"}


def argb(text):
    """TAB's `ColorHue` (base64 ARGB) -> (r, g, b), or None for white."""
    if not text or text == "/////w==":
        return None
    a, r, g, b = base64.b64decode(text)
    return (r, g, b)


class Project:
    """Everything of ZXGame.dxprj we need: templates and clips by name, sprites and pictures by number."""

    def __init__(self, game):
        with zipfile.ZipFile(os.path.join(game, "ZXGame.dxprj")) as z:
            root = ET.fromstring(z.read(z.namelist()[0]))
        self.by_id, self.templates, self.clips, self.images = {}, {}, {}, {}
        for c in root.iter("Complex"):
            v = props(c)
            if "ID" in v:
                self.by_id.setdefault(v["ID"], c)
            if "FileName" in v and "ID" in v and "IDProject" in v:
                self.images[v["ID"]] = v["FileName"]
            if "ClassName" in v and "Name" in v:
                self.templates.setdefault(v["Name"], c)
            if "TypeOfClip" in v and "Name" in v:
                self.clips.setdefault(v["Name"], c)

    def config(self, template, name):
        """A template's configuration of this kind (`DXComponentClip+TTemplateConfiguration` ...) as a dict."""
        for cfg in self.templates[template].iter("Complex"):
            if kind(cfg) == name:
                return props(cfg)
        return {}

    def look(self, template):
        """The template's `DefaultVisualState`: (saturation, contrast, brightness, hue), tint."""
        d = self.config(template, "DXComponentDefaultVisualState+TTemplateConfiguration")
        look = (num(d.get("Saturation"), 1), num(d.get("Contrast"), 1), num(d.get("Brightness"), 1), num(d.get("HueRotation")))
        return look, argb(d.get("ColorHue"))

    def image_file(self, tab, image_id):
        """`Images\\...png` of a picture's number - or None."""
        name = self.images.get(image_id)
        if not name:
            return None
        rel = name.replace("\\", "/").split("Images/", 1)[1]
        folder, file = os.path.split(rel)
        return single(os.path.join(tab, "Images"), folder, os.path.splitext(file)[0])


class Layer:
    """One object of a clip, as it is in its first frame."""

    def __init__(self, project, obj, state):
        o = props(obj)
        self.name = o.get("Name") or ""
        self.z = num(o.get("PositionZ"))
        self.blend = o.get("BlendMode") or "Normal"
        self.loop = o.get("Loop") == "True"
        self.frame_ms = num(o.get("FrameTime"), 30)
        self.area = tuple(num(v) for v in state["Area"].split(";"))
        self.visible = state.get("Visible", "True") == "True"
        self.opacity = num(state.get("Opacity"), 1)
        self.look = (num(state.get("Saturation"), 1), num(state.get("Contrast"), 1), num(state.get("Brightness"), 1),
                     num(state.get("HueRotation")))
        self.tint = argb(state.get("ColorHue"))
        self.sprite = self.image = self.clip = None
        what = kind(obj)
        if what == "DXProjectObject_Sprite" and o.get("IDSprite") in project.by_id:
            s = props(project.by_id[o["IDSprite"]])
            self.sprite = (s.get("AtlasFileName"), s.get("SpriteName"))
        elif what == "DXProjectObject_Image":
            self.image = o.get("IDImage")
        elif what == "DXProjectObject_Clip" and o.get("IDClip") in project.by_id:
            self.clip = project.by_id[o["IDClip"]]


def layers(project, clip):
    """The objects of a clip that are in its first frame, back to front."""
    p = clip.find("Properties")
    frames = p.find("Dictionary[@name='Frames']").find("Items")
    first = {}
    for item in frames:
        if item.find("Simple").get("value") != "0":
            continue
        for inst in item.iter("Item"):
            key, state = inst.find("Simple"), inst.find(".//Complex[@name='State']")
            if key is not None and state is not None:
                first[key.get("value")] = props(state)
    out = []
    for obj in p.find("Dictionary[@name='Objects']").iter("Complex"):
        if kind(obj).startswith("DXProjectObject") and props(obj).get("ID") in first:
            out.append(Layer(project, obj, first[props(obj)["ID"]]))
    return sorted(out, key=lambda l: l.z)


def clip_size(clip):
    return pair(props(clip)["Size"])


class Frames:
    """The pictures of a sprite (`Sprites\\<atlas>\\<name>\\`), in order - and how many of TAB's frames each one is (the
    extracted game keeps only every other one of some)."""

    cache = {}

    @classmethod
    def of(cls, tab, atlas, name):
        key = (atlas, name)
        if key not in cls.cache:
            name = name.strip()  # TAB has "StoneHouseB_Light light " with a space at the end
            folder = os.path.join(tab, "Sprites", atlas, name)
            found = []
            if os.path.isdir(folder):
                pattern = re.compile(re.escape(name) + r"(?:_(\d+))?\.png$")
                for file in os.listdir(folder):
                    m = pattern.match(file)
                    if m:
                        found.append((int(m.group(1) or 0), os.path.join(folder, file)))
            found.sort()
            step = found[1][0] - found[0][0] if len(found) > 1 else 1
            paths = [p for _, p in found]
            if closes_circle(paths):
                paths = paths[:-1]
            cls.cache[key] = (paths, step)
        return cls.cache[key]


def closes_circle(paths):
    """Does the last picture show (almost) the same as the first? Many of TAB's loops end where they began - the radar
    of the command center, a turning gun. Played over and over, that picture would stand there twice in a row: a jerk
    once every turn. Almost the same: less than a third as different as two pictures next to each other."""
    if len(paths) < 3:
        return False

    def small(path):
        return Image.open(path).convert("RGBA").resize((128, 128), Image.BILINEAR)

    def apart(a, b):
        return sum(ImageStat.Stat(ImageChops.difference(a, b)).mean)

    first, second, last = small(paths[0]), small(paths[1]), small(paths[-1])
    neighbours = apart(first, second)
    return neighbours > 0 and apart(last, first) < neighbours / 3


def loop_seconds(tab, project, clip, show, animated=False):
    """How long the longest moving layer of a clip takes for one loop (0: nothing moves) - and whether a machine moves,
    not just flags and smoke."""
    longest, machine = 0.0, False
    for layer in layers(project, clip):
        if not layer.visible or not show(layer.name):
            continue
        if layer.sprite and (layer.loop or animated):
            paths, step = Frames.of(tab, *layer.sprite)
            if len(paths) > 1:
                longest = max(longest, len(paths) * step * layer.frame_ms / 1000)
                machine = machine or not (animated or "Flag" in layer.name or "Smoke" in layer.name)
        elif layer.clip is not None and layer.loop:
            longest = max(longest, loop_seconds(tab, project, layer.clip, show, True)[0])
    return longest, machine


def premultiplied_resize(image, size):
    return image.convert("RGBa").resize(size, Image.LANCZOS).convert("RGBA")


def add_light(canvas, light, at):
    """TAB's added layer (`BlendMode.Add`): its colour added to what's below - on the transparent canvas it becomes as
    opaque as it's bright, so drawn over the ground it still glows."""
    region = (at[0], at[1], at[0] + light.width, at[1] + light.height)
    below = canvas.crop(region).convert("RGBa")
    lit = light.convert("RGBa")
    r, g, b, a = below.split()
    lr, lg, lb, _ = lit.split()
    r, g, b = ImageChops.add(r, lr), ImageChops.add(g, lg), ImageChops.add(b, lb)
    a = ImageChops.lighter(a, ImageChops.lighter(lr, ImageChops.lighter(lg, lb)))
    canvas.paste(Image.merge("RGBa", (r, g, b, a)).convert("RGBA"), region)


def key_of(layer):
    """A layer told apart from the others of its clip (two may have the same name)."""
    return (layer.name, layer.z)


def render(tab, project, clip, scale, time, show, animated=False, only=None, forced=None):
    """The clip's first frame at `time` seconds (None: everything standing still), `scale` pixels per unit of its
    canvas - only the layers `show(name)` wants, and of the clip itself only those in `only` (their `key_of`; None:
    all). `forced`: {key_of: picture file} - that layer shows this picture (a turning gun's frame). Returns the picture
    and where the canvas' top left corner is in it."""
    drawn = []
    for layer in layers(project, clip):
        if not layer.visible or not show(layer.name) or layer.opacity <= 0:
            continue
        if only is not None and key_of(layer) not in only:
            continue
        x, y, w, h = layer.area
        box = (math.floor(x * scale), math.floor(y * scale), max(1, round(w * scale)), max(1, round(h * scale)))
        if layer.clip is not None:
            child_w, _ = clip_size(layer.clip)
            picture, (ox, oy) = render(tab, project, layer.clip, scale * w / child_w, time, show, animated or layer.loop)
            if picture is None:
                continue
            drawn.append((layer, picture, (box[0] - ox, box[1] - oy)))
            continue
        path = None
        if forced and key_of(layer) in forced:
            path = forced[key_of(layer)]
        elif layer.sprite:
            paths, step = Frames.of(tab, *layer.sprite)
            if not paths:
                print(f"    {layer.sprite[1]}: no pictures - left out")
                continue
            index = 0
            if time is not None and (layer.loop or animated) and len(paths) > 1:
                index = int(time * 1000 / (layer.frame_ms * step)) % len(paths)
            path = paths[index]
        elif layer.image:
            path = project.image_file(tab, layer.image)
        if not path:
            continue
        picture = premultiplied_resize(Image.open(path).convert("RGBA"), box[2:])
        picture = tinted(picture, layer.look, layer.tint)
        if layer.opacity < 1:
            picture.putalpha(picture.getchannel("A").point(lambda v: round(v * layer.opacity)))
        drawn.append((layer, picture, box[:2]))
    if not drawn:
        return None, (0, 0)
    x0 = min(at[0] for _, _, at in drawn)
    y0 = min(at[1] for _, _, at in drawn)
    x1 = max(at[0] + p.width for _, p, at in drawn)
    y1 = max(at[1] + p.height for _, p, at in drawn)
    canvas = Image.new("RGBA", (x1 - x0, y1 - y0), (0, 0, 0, 0))
    for layer, picture, (x, y) in drawn:
        if layer.blend == "Add":
            add_light(canvas, picture, (x - x0, y - y0))
        else:
            canvas.alpha_composite(picture, (x - x0, y - y0))
    return canvas, (-x0, -y0)


def ghost(picture):
    """TAB's look while it's being built: contrast 5, coloured lime, half see-through."""
    g = tinted(picture, (1, GHOST_CONTRAST, 1, 0), GHOST_TINT)
    g.putalpha(picture.getchannel("A").point(lambda v: round(v * GHOST_OPACITY)))
    return g


class Sheet:
    """The pictures of one skin, each with where its feet are, packed into one PNG."""

    def __init__(self):
        self.pictures = []  # (image, feet x, feet y)

    def add(self, image, feet):
        box = image.getchannel("A").getbbox() or (0, 0, 1, 1)
        # Where it's see-through, black: otherwise the colour there would bleed into the edge when drawn smaller.
        image = Image.composite(image, Image.new("RGBA", image.size, (0, 0, 0, 0)), image.getchannel("A").point(lambda v: 255 if v else 0))
        self.pictures.append((image.crop(box), feet[0] - box[0], feet[1] - box[1]))
        return len(self.pictures) - 1

    def save(self, key, meta, out=OUT):
        blocks = [(p.width + 2 * PAD, p.height + 2 * PAD) for p, _, _ in self.pictures]
        order = sorted(range(len(blocks)), key=lambda i: -blocks[i][1])
        places, width, height = layout([blocks[i] for i in order])
        sheet = Image.new("RGBA", (width, height), (0, 0, 0, 0))
        rects = [None] * len(blocks)
        for i, (x, y) in zip(order, places):
            p, fx, fy = self.pictures[i]
            sheet.paste(p, (x + PAD, y + PAD))
            rects[i] = [x + PAD, y + PAD, p.width, p.height, round(fx, 1), round(fy, 1)]
        sheet.save(os.path.join(out, f"{key}.png"), optimize=True)
        meta = {"image": f"{key}.png", "pixels_per_tile": PIXELS_PER_TILE, **meta, "pictures": rects}
        with open(os.path.join(out, f"{key}.json"), "w", encoding="utf-8") as f:
            json.dump(meta, f, indent=1)
        print(f"  {key}: {len(rects)} pictures, {width} x {height} pixels")


def plain(i, ghost_i):
    """The looks of something that never moves: one picture working and idle, its ghost while being built."""
    return {"working": [{"frames": [i]}], "idle": [{"frames": [i]}], "building": [{"frames": [ghost_i]}]}


def motion(tab, project, layer):
    """How a layer moves while the building works: (frames, seconds per loop) - (1, 0) if it stands still. A layer
    that is a clip (a flag on its pole) moves as a whole, with the frames of its longest moving sprite."""
    if layer.sprite and layer.loop:
        paths, step = Frames.of(tab, *layer.sprite)
        if len(paths) > 1:
            return len(paths), len(paths) * step * layer.frame_ms / 1000
    if layer.clip is not None and layer.loop:
        seconds, _ = loop_seconds(tab, project, layer.clip, lambda name: True, True)
        frames = [len(Frames.of(tab, *l.sprite)[0]) for l in layers(project, layer.clip) if l.sprite]
        if seconds > 0 and max(frames, default=0) > 1:
            return max(frames), seconds
    return 1, 0.0


def numbered(paths):
    """The frame numbers in TAB's file names (`..._0003.png` -> 3)."""
    return [int(re.search(r"(\d+)\.png$", p).group(1)) for p in paths]


def command_ms(tab, command):
    """A command's `TimeAction` in ZXRules (`Commands.tsv`): how long one shot takes - None if it isn't there."""
    path = os.path.join(tab, "Data", "ZXRules", "Commands.tsv")
    if not command or not os.path.isfile(path):
        return None
    with open(path, encoding="utf-8") as f:
        rows = [line.rstrip("\n").split("\t") for line in f]
    head = rows[0]
    for row in rows[1:]:
        if row and row[0] == command:
            return int(num(row[head.index("TimeAction")]))
    return None


def parts(tab, project, clip, scale, add, working, show, turret):
    """One look of a clip as parts, back to front (see the top). `add(picture, origin)` puts a picture into the sheet in
    this look's colours and returns its number. `turret`: the TURRETS entry of a building that turns, or None."""
    out, group = [], []
    gun = turret[0] if turret else None

    def flush():
        if group:
            picture, origin = render(tab, project, clip, scale, None, show, only={key_of(l) for l in group})
            if picture is not None:
                out.append({"frames": [add(picture, origin)]})
            group.clear()

    def one(layer, time=None, forced=None):
        picture, origin = render(tab, project, clip, scale, time, lambda name: True, only={key_of(layer)}, forced=forced)
        return None if picture is None else add(picture, origin)

    for layer in layers(project, clip):
        if not layer.visible or layer.opacity <= 0:
            continue
        if layer.name == gun:
            flush()
            out.append(gun_part(tab, project, clip, layer, turret, one))
            continue
        if not show(layer.name) or (not working and layer.name.startswith(WORKING_ONLY)):
            continue
        frames, seconds = motion(tab, project, layer) if working else (1, 0.0)
        frames = min(frames, MAX_FRAMES)
        lit = working and layer.blend == "Add"
        if frames == 1 and not lit:
            group.append(layer)
            continue
        flush()
        pictures = [one(layer, k * seconds / frames if frames > 1 else None) for k in range(frames)]
        pictures = [i for i in pictures if i is not None]
        if not pictures:
            continue
        part = {"frames": pictures}
        if frames > 1:
            part["fps"] = round(frames / seconds, 3)
        if lit:
            part["add"] = True
        out.append(part)
    flush()
    return out


def gun_part(tab, project, clip, layer, turret, one):
    """The gun of a building that turns: its frames once round the circle - `turn_from` the angle of the first one,
    `turn` how many degrees further each next one is (less than 0: counter-clockwise). And the ballista's shot: for
    every 18 degrees from 0 the frames of one shot, played once in `shot_ms`."""
    _, shot, start, direction, command = turret
    paths, step = Frames.of(tab, *layer.sprite)
    n = len(paths) * step  # TAB's frames - the extracted game keeps only every `step`th
    first = numbered(paths)[0]
    part = {
        "frames": [one(layer, forced={key_of(layer): p}) for p in paths],
        "turn_from": round(start + direction * 360 * first / n, 3),
        "turn": round(direction * 360 * step / n, 4),
    }
    attack = next((l for l in layers(project, clip) if l.name == shot), None) if shot else None
    if attack is not None and attack.sprite:
        name = attack.sprite[1].strip()
        folder = os.path.join(tab, "Sprites", attack.sprite[0], name)
        angles = []
        for angle in range(0, 360, 18):
            files = sorted(f for f in os.listdir(folder) if f.startswith(f"{name}_{angle:03d}_"))
            angles.append([one(attack, forced={key_of(attack): os.path.join(folder, f)}) for f in files])
        part["shot"] = angles
        part["shot_ms"] = command_ms(tab, command) or 1000
    return part


def looks(tab, project, sheet, clip, scale, feet, look, tint, show=lambda name: True, turret=None):
    """The three looks of one clip (or of the layers `show` picks): working, idle, being built - added to the sheet.
    Returns {"working": [parts], "idle": [parts], "building": [parts]} - None if there's no picture at all."""
    saturation, contrast, brightness, hue = look

    def adder(sat_con, make=lambda p: p):
        def add(picture, origin):
            picture = make(tinted(picture, (sat_con[0], sat_con[1], brightness, hue), tint))
            return sheet.add(picture, (feet[0] + origin[0], feet[1] + origin[1]))
        return add

    still = lambda name: show(name) and not name.startswith(WORKING_ONLY)
    whole, origin = render(tab, project, clip, scale, None, still)
    if whole is None:
        return None
    working = parts(tab, project, clip, scale, adder((saturation, contrast)), True, show, turret)
    idle = parts(tab, project, clip, scale, adder(IDLE_LOOK), False, show, turret)
    building = adder((saturation, contrast), ghost)(whole, origin)
    return {"working": working, "idle": idle, "building": [{"frames": [building]}]}


def clip_building(tab, project, key, template, clip_names, width):
    """A building that is a clip - one skin per clip (TAB's random ones). Placed like `DXComponentVisual.UpdateArea`
    does it: the canvas `width x SceneScalingFactor` tile widths wide, moved by `SceneOffsetFactor`."""
    cfg = project.config(template, "DXComponentClip+TTemplateConfiguration")
    scaling, off_x, off_y = num(cfg.get("SceneScalingFactor"), 1), num(cfg.get("SceneOffsetFactorX")), num(cfg.get("SceneOffsetFactorY"))
    look, tint = project.look(template)
    for n, name in enumerate(clip_names, 1):
        clip = project.clips[name]
        cw, ch = clip_size(clip)
        scale = min(1.0, PIXELS_PER_TILE * width * scaling / cw)
        # The lowest corner of the site, in pixels from the canvas' top left corner.
        feet = (cw * scale * (0.5 - off_x), ch * scale * (1 - off_y))
        sheet = Sheet()
        meta = {"size": width}
        is_tower = any(l.name == "@Front" for l in layers(project, clip))
        if is_tower:
            meta["body"] = looks(tab, project, sheet, clip, scale, feet, look, tint, lambda name: name != "@Front")
            meta["front"] = looks(tab, project, sheet, clip, scale, feet, look, tint, lambda name: name == "@Front")
            # Units inside stand on the tower, their feet a bit below the middle of its front wall (`Tower.cs` puts them
            # between body and front). Lifted this many tile widths from the middle of the site.
            front = next(l for l in layers(project, clip) if l.name == "@Front")
            stand = (front.area[1] + 0.6 * front.area[3]) * scale
            meta["roof"] = round((feet[1] - TILE_HEIGHT / 2 - stand) / PIXELS_PER_TILE, 3)
        else:
            meta["body"] = looks(tab, project, sheet, clip, scale, feet, look, tint,
                                 lambda name: not name.startswith(HIDDEN) and "Telescope" not in name, TURRETS.get(key))
        if meta["body"] is None:
            print(f"  {key}: no pictures for {name} - left out")
            continue
        sheet.save(key if len(clip_names) == 1 else f"{key}_{n}", meta)


def variants(project, template):
    """A template's clips: its own, or all of its random ones (`RandomClipName`: `Tent`, `TentB` ...)."""
    cfg = project.config(template, "DXComponentClip+TTemplateConfiguration")
    prefix = cfg.get("RandomClipName")
    if prefix:
        found = sorted(n for n in project.clips if n.startswith(prefix) and len(n) <= len(prefix) + 2)
        if found:
            return found
    return [props(project.by_id[cfg["IDClip"]])["Name"]]


def image_template(tab, project, template):
    """A template that is a picture (`DXComponentImage`, maybe random): its files, scaling and offset."""
    cfg = project.config(template, "DXComponentImage+TTemplateConfiguration")
    rnd = project.config(template, "DXComponentRandomImage+RandomImageTemplateConfiguration").get("ImageName")
    first = project.image_file(tab, cfg.get("IDImage"))
    files = [first] if first else []
    if rnd and first:
        # The random ones lie next to the first: `TrapStakes1` ... `TrapStakes4` (each maybe in its own folder).
        images = os.path.join(tab, "Images")
        folder = os.path.relpath(os.path.dirname(first), images)
        if os.path.splitext(os.path.basename(first))[0] == os.path.basename(folder):
            folder = os.path.dirname(folder)
        found = []
        for entry in sorted(os.listdir(os.path.join(images, folder))):
            stem = os.path.splitext(entry)[0]
            if re.fullmatch(re.escape(rnd) + r"\d*", stem):
                path = single(images, folder, stem)
                if path:
                    found.append(path)
        files = found or files
    return files, num(cfg.get("SceneScalingFactor"), 1), num(cfg.get("SceneOffsetFactorX")), num(cfg.get("SceneOffsetFactorY"))


def picture_on_site(path, width_tiles, off_x, off_y):
    """A picture TAB puts on a site: `width_tiles` wide, its feet (the site's lowest corner) moved by the offset."""
    image = Image.open(path).convert("RGBA")
    w = round(width_tiles * PIXELS_PER_TILE)
    h = max(1, round(image.height * w / image.width))
    image = premultiplied_resize(image, (w, h))
    return image, (w * (0.5 - off_x), h * (1 - off_y))


def image_building(tab, project, key, template, size=1, out=OUT):
    """A building (or a thing on the map) that is one picture: `size x SceneScalingFactor` tile widths wide. `size` can
    be (x, y) for a site that isn't square (TAB: as wide as its x side, its feet still the site's lowest corner)."""
    site = None
    if isinstance(size, tuple):
        site, size = list(size), size[0]
    if template not in project.templates:
        print(f"  {key}: no template {template} - left out")
        return
    files, scaling, off_x, off_y = image_template(tab, project, template)
    if not files:
        print(f"  {key}: no picture of {template} - left out")
        return
    look, tint = project.look(template)
    for n, path in enumerate(files, 1):
        image, feet = picture_on_site(path, size * scaling, off_x, off_y)
        sheet = Sheet()
        normal = tinted(image, look, tint)
        meta = {"size": size, **({"site": site} if site else {}),
                "body": plain(sheet.add(normal, feet), sheet.add(ghost(normal), feet))}
        sheet.save(key if len(files) == 1 else f"{key}_{n}", meta, out)


def pieces(project, tab, sheet, clip_name, pillar_box, feet, look, tint, names):
    """Pieces of a clip placed relative to its `@Pillar` the way `CloneObjectProportionalTo` does: `pillar_box` is the
    pillar's rectangle in pixels (x, y, width, height), `feet` the point they all stand on. Returns {name: looks}."""
    by_name = {l.name: l for l in layers(project, project.clips[clip_name])}
    px, py, pw, ph = by_name["@Pillar"].area
    bx, by, bw, bh = pillar_box
    kx, ky = bw / pw, bh / ph
    out = {}
    for ours, theirs in names.items():
        layer = by_name[theirs]
        path = project.image_file(tab, layer.image)
        x, y, w, h = layer.area
        rect = (bx + (x - px) * kx, by + (y - py) * ky, w * kx, h * ky)
        image = premultiplied_resize(Image.open(path).convert("RGBA"), (max(1, round(rect[2])), max(1, round(rect[3]))))
        image = tinted(image, look, tint)
        at = (feet[0] - rect[0], feet[1] - rect[1])
        out[ours] = plain(sheet.add(image, at), sheet.add(ghost(image), at))
    return out


PIECES = {"left": "@Left", "right": "@Right", "horizontal": "@Horizontal", "vertical": "@Vertical"}


def wall(tab, project, key, template, clip_name):
    """A wall: its pillar and the four pieces towards its neighbours - all standing on the lowest corner of its tile."""
    files, scaling, off_x, off_y = image_template(tab, project, template)
    look, tint = project.look(template)
    pillar, feet = picture_on_site(files[0], scaling, off_x, off_y)
    sheet = Sheet()
    normal = tinted(pillar, look, tint)
    meta = {"size": 1, "pieces": {"pillar": plain(sheet.add(normal, feet), sheet.add(ghost(normal), feet))}}
    # The stone wall's pillar in a straight line (`SetPillarOrnament`): StonePillar_C, in the same place.
    if template == "WallStone":
        path = single(os.path.join(tab, "Images"), "Buildings", "StonePillar_C")
        straight = tinted(premultiplied_resize(Image.open(path).convert("RGBA"), pillar.size), look, tint)
        meta["pieces"]["pillar_straight"] = plain(sheet.add(straight, feet), sheet.add(ghost(straight), feet))
    # The pillar's rectangle with the feet at (0, 0) - the pieces are placed relative to it.
    box = (-feet[0], -feet[1], pillar.width, pillar.height)
    meta["pieces"].update(pieces(project, tab, sheet, clip_name, box, (0, 0), look, tint, PIECES))
    sheet.save(key, meta)


def gate(tab, project, key, template, pillar_image, walls_clip, gate_clip):
    """A gate (`Gate.cs`): a pillar on both end tiles - half a tile wide, its bottom middle a quarter tile from the
    tile's middle towards its lowest corner - and the door between them, placed relative to the first pillar. The
    pillars join the walls next to them with the wall pieces."""
    look, tint = project.look(template)
    folder, stem = pillar_image.split("/")
    path = single(os.path.join(tab, "Images"), folder, stem)
    image = Image.open(path).convert("RGBA")
    w = round(0.5 * PIXELS_PER_TILE)
    h = round(image.height * w / image.width)
    pillar = tinted(premultiplied_resize(image, (w, h)), look, tint)
    # The tile's lowest corner is a quarter tile further down the screen than the pillar's bottom middle: (0.25, 0.25)
    # tiles in the world are 0.25 tile heights on screen.
    feet = (w / 2, h + TILE_HEIGHT / 4)
    sheet = Sheet()
    meta = {"size": 1, "pieces": {"pillar": plain(sheet.add(pillar, feet), sheet.add(ghost(pillar), feet))}}
    box = (-feet[0], -feet[1], w, h)
    meta["pieces"].update(pieces(project, tab, sheet, gate_clip, box, (0, 0), look, tint,
                                 {"door_x": "@GateRight", "door_y": "@GateLeft"}))
    meta["pieces"].update(pieces(project, tab, sheet, walls_clip, box, (0, 0), look, tint, PIECES))
    sheet.save(key, meta)


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    game = sys.argv[2] if len(sys.argv) > 2 else DEFAULT_GAME
    if not os.path.isdir(os.path.join(tab, "Sprites")):
        sys.exit(f"No Sprites folder in {tab} - give the _assets folder of the extracted game.")
    if not os.path.isfile(os.path.join(game, "ZXGame.dxprj")):
        sys.exit(f"No ZXGame.dxprj in {game} - give the folder TAB is installed in.")
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, "..", "data", "buildings.json"), encoding="utf-8") as f:
        sizes = {k: v["size"][0] for k, v in json.load(f).items() if isinstance(v, dict) and "size" in v}
    print("Reading ZXGame.dxprj ...")
    project = Project(game)
    os.makedirs(OUT, exist_ok=True)
    print(f"From {tab} to {os.path.abspath(OUT)}:")
    only = set(sys.argv[3:])  # for trying: only these skins
    for key, template in BUILDINGS.items():
        if only and key not in only:
            continue
        clips = FISHING if key == "fishing_hut" else variants(project, template)
        clip_building(tab, project, key, template, clips, sizes[key])
    for key, (template, width) in TAB_BUILDINGS.items():
        if not only or key in only:
            clip_building(tab, project, key, template, variants(project, template), width)
    for key, (template, width) in DOOM.items():
        if not only or key in only:
            clip_building(tab, project, key, template, variants(project, template), width)
    for key, (template, width) in IMAGES.items():
        if not only or key in only:
            image_building(tab, project, key, template, width)
    os.makedirs(DECOR_OUT, exist_ok=True)
    for key, (template, width) in DECOR.items():
        if not only or key in only:
            image_building(tab, project, key, template, width, DECOR_OUT)
    for key, (template, clip) in WALLS.items():
        if not only or key in only:
            wall(tab, project, key, template, clip)
    for key, parts in GATES.items():
        if not only or key in only:
            gate(tab, project, key, *parts)


if __name__ == "__main__":
    main()
