"""Turns TAB's interface pictures into ours: assets\\ui\\ (explained in assets\\README.md).

TAB's pictures are copyrighted - only for us, on this PC (assets\\ui\\ is in .gitignore). This script makes them anew
from your own copy of the game:

    python tools\\tab_ui.py [assets folder]

`assets folder`: the `_assets` folder of the extracted game (its `Images\\` and `Cursors\\`, like for tab_skins.py).

What comes out (the game draws its own placeholder for whatever is missing):
- `panel.png`: TAB's bottom bar (`GUIBackgroundControls`, the clip `MapGUI` puts it at 0, 523 of a 1280 x 720 screen) -
  where the minimap, the clock, the selection and the stock go is in src\\hud.rs (`Layout`).
- `gear.png`, `button_square.png`, `button_round.png`, `button_rect.png`, `event_background.png`: the gears behind the
  clock, the buttons (`ButtonSquareSmall` = the command card, `ButtonRoundBig` = pause), TAB's frame of a control group
  and the dark-green circle behind an event's icon. `cell_available.png`: TAB's build grid (one free tile).
- `icons\\<TAB's name>.png`: command icons (attack, stop, the build menus ...), the stock's icons (`IconFGold` ...), the
  minimap's marks - at most 128 pixels.
- `events\\<TAB's name>.png`: the pictures of TAB's event list (`EventSwarm`, `EventInfo` ...).
- `portraits\\<our kind>.png`: who or what is selected - named like our kinds in Rust (`Soldier`, `MachineGunTower`,
  `Walker` ...), so the game finds them without a table. Units, zombies and loot as TAB's 3 : 4 portraits, buildings
  as TAB's square pictures of them. The medic and the spearman get their skins' paint (tab_skins.py: silver, marine).
- `backgrounds\\<TAB's name>.png`: the main menu's layers (`Background_Sky`, the three rows of the city, the ground,
  the colony), the logo, the news board and the weather's sky (`Sky_A`) - at most 2560 pixels wide, for menus later.
- `cursors\\<TAB's name>.png`: TAB's mouse pointers (`CursorArrowImage.png` ...) - where their tip is, is in
  src\\ui_art.rs (TAB's `ZXPalette`).
"""

import os
import shutil
import sys

from PIL import Image

from tab_skins import marine, silver
from tab_terrain import DEFAULT_TAB, single

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "ui")

# Single pictures: (folder in Images\, TAB's name) -> our file, at most this many pixels on the longer side (None: as
# it is).
SINGLES = {
    "panel": ("Interface", "GUIBackgroundControls", None),
    "gear": ("Interface", "Gear1", 256),
    "button_square": ("Interface", "ButtonSquareSmall", 128),
    "button_round": ("Interface", "ButtonRoundBig", 192),
    "button_rect": ("Interface", "ButtonRectangularSmall", 128),
    "event_background": ("Icons", "EventBackground", 96),
    # The build grid: one free tile, drawn on every tile you could build on around the building on the mouse.
    "cell_available": ("Interface", "CellAvailable", None),
}

# The icons we use (src\hud.rs `Button::icon`, the stock, the minimap).
ICONS = [
    # Units
    "IconTravel", "IconStop", "IconHold", "IconPatrol", "IconAttack", "IconLeaveObject", "IconDismissUnit",
    # Buildings
    "IconSetRallyPoint", "IconRepair", "IconDestroyBuilding", "IconExtractAll", "IconFResourcesToGold", "IconFWatchRange",
    "IconPause", "IconPlay", "IconBack", "CustomCommandGear",
    # The command center's build menus
    "IconDwellings", "IconResources", "IconEnergy", "IconIndustry", "IconMilitary", "IconDeffense", "IconWonders",
    # The stock and the clock
    "IconFColonist", "IconFWorker", "IconFFood", "IconFEnergy", "IconFGold", "IconFWood", "IconFStone", "IconFIron",
    "IconFOil", "IconFVeteran", "IconFArmor", "IconFAttackDamage", "IconFAttackRange", "IconFAttackSpeed",
    "IconFSpeed",
    # The right-hand buttons: camera to the base, the menu
    "IconMapColony",
    # The minimap: a swarm of a wave, a giant
    "IconMiniMap_Swarn", "IconInfectedGiant",
    # Trains (tab_trains.py): the one in the stock, the one for the editor's scripts
    "IconFTrain",
]
ICON_SIDE = 128
# TAB's options button is a picture of its own in Interface\.
INTERFACE_ICONS = ["IconOptions"]

EVENTS = ["EventSwarm", "EventInfo", "EventReward", "EventWarning", "EventGoalCompleted", "EventBuildingsAttacked",
          "EventSoldiersAttacked", "EventWallsAttacked", "EventFire", "EventInfection", "EventHealing", "EventTrainArrival"]
EVENT_SIDE = 96

# Our kind -> TAB's portrait (and our paint on it). The building pictures are the same buildings as in
# tab_buildings.py (the charcoal burner is TAB's power plant, the munitions factory its warehouse).
PORTRAITS = {
    # Units
    "Soldier": "Soldier", "Ranger": "Ranger", "Sniper": "Sniper",
    "Medic": ("Soldier", silver), "Spearman": ("Lucifer", marine),
    "MercSoldier": "Soldier", "MercRanger": "Ranger", "MercSniper": "Sniper",
    "Lucifer": "Lucifer", "Thanatos": "Thanatos", "Titan": "Titan",
    # Zombies
    "Walker": "ZombieWeakA", "Runner": "ZombieMediumA", "Fat": "ZombieStrongA", "Armored": "ZombieDressedA",
    "Spitter": "ZombieVenom", "Infested": "ZombieWorkerA", "Harpy": "ZombieHarpy", "Giant": "ZombieGiant",
    # Buildings
    "CommandCenter": "PortraitCommandCenter", "Barracks": "SoldiersCenter", "Tent": "TentHouse", "Wall": "WallWood",
    "Gate": "GateWood", "Tower": "WatchTowerWood", "Sawmill": "Sawmill", "Mine": "Quarry",
    "HunterCottage": "HunterCottage", "FishingHut": "FishermanCottage", "Farm": "Farm", "Windmill": "MillWood",
    "CharcoalBurner": "PowerPlant", "WoodWorkshop": "WoodWorkshop", "TeslaTower": "TeslaTower",
    "StoneWorkshop": "StoneWorkshop", "MunitionsFactory": "WareHouse", "Foundry": "Foundry", "StoneHouse": "StoneHouse",
    "StoneWall": "WallStone", "StoneTower": "WatchTowerStone", "Ballista": "Ballista", "MachineGunTower": "MachineGun",
    "Stakes": "TrapStakes", "Market": "Market", "Bank": "Bank", "Inn": "TheInn", "Barrel": "ExplosiveBarrel",
    # TAB's late buildings (docs\tab-assets.md steps 5-7)
    "WoodHouse": "CottageHouse", "PowerPlant": "PowerPlant", "OilPlatform": "OilPlatform", "Quarry": "Quarry",
    "BigQuarry": "AdvancedQuarry", "BigFarm": "AdvancedFarm", "BigMill": "MillIron", "Warehouse": "WareHouse",
    "EngineeringCenter": "AdvancedUnitCenter", "ShockingTower": "ShockingTower", "Executor": "Executor",
    "BladeTrap": "TrapBlades", "MineTrap": "TrapMine", "Radar": "RadarTower", "LookoutTower": "LookoutTower",
    "Spire": "TheSpire", "Academy": "TheAcademy", "Victorious": "TheVictorious", "Transmutator": "TheTransmutator",
    "CrystalPalace": "TheCrystalPalace",
    # The houses of the villages of doom (by their size) and the loot
    "DoomSmall": "DoomBuildingSmall", "DoomMedium": "DoomBuildingMedium", "DoomBig": "DoomBuildingLarge",
    "LootGold": "PickableGold", "LootWood": "PickableWood", "LootStone": "PickableStone", "LootIron": "PickableIron",
}
# Units and zombies: TAB's 375 x 500 portraits, made 150 x 200. Buildings: TAB's square pictures, 200 x 200.
PORTRAIT_SIDE = 200

# TAB's mouse pointers (Cursors\<name>Image.png).
CURSORS = ["CursorArrow", "CursorTarget", "CursorInvalid", "CursorEnter"]
CURSOR_SIDE = 64
# The menu's pictures (Images\Backgrounds, and Images\Weather\Sky_A): the layers behind the menus (src\home.rs `draw_tab_backdrop`).
BACKGROUNDS = [("Backgrounds", n) for n in ("Background_Sky", "Background_Row1", "Background_Row2", "Background_Row3",
                                          "Background_Ground", "Background_Colony", "LogoRender4K", "NewsBackground")]
BACKGROUNDS.append(("Weather", "Sky_A"))
BACKGROUND_SIDE = 2560


def fitted(image, side):
    """No bigger than `side` on its longer side (smaller ones stay as they are)."""
    if side is None or max(image.size) <= side:
        return image
    k = side / max(image.size)
    return image.resize((max(1, round(image.width * k)), max(1, round(image.height * k))), Image.LANCZOS)


def save(image, *path):
    target = os.path.join(OUT, *path)
    os.makedirs(os.path.dirname(target), exist_ok=True)
    image.save(target, optimize=True)


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    images = os.path.join(tab, "Images")
    if not os.path.isdir(images):
        sys.exit(f"No Images folder in {tab} - give the _assets folder of the extracted game.")
    # Made anew every time: a picture we no longer use doesn't stay behind.
    shutil.rmtree(OUT, ignore_errors=True)
    missing = []

    def load(folder, name):
        path = single(images, folder, name)
        if not path:
            missing.append(f"{folder}\\{name}")
            return None
        return Image.open(path).convert("RGBA")

    for ours, (folder, name, side) in SINGLES.items():
        if (image := load(folder, name)) is not None:
            save(fitted(image, side), f"{ours}.png")
    for name in ICONS:
        if (image := load("Icons", name)) is not None:
            save(fitted(image, ICON_SIDE), "icons", f"{name}.png")
    for name in INTERFACE_ICONS:
        if (image := load("Interface", name)) is not None:
            save(fitted(image, ICON_SIDE), "icons", f"{name}.png")
    for name in EVENTS:
        if (image := load("Icons", name)) is not None:
            save(fitted(image, EVENT_SIDE), "events", f"{name}.png")
    for ours, theirs in PORTRAITS.items():
        name, paint = theirs if isinstance(theirs, tuple) else (theirs, None)
        image = load("Portraits", name)
        if image is None:
            continue
        image = fitted(image, PORTRAIT_SIDE)
        if paint:
            image = paint(image)
        save(image, "portraits", f"{ours}.png")
    for folder, name in BACKGROUNDS:
        if (image := load(folder, name)) is not None:
            save(fitted(image, BACKGROUND_SIDE), "backgrounds", f"{name}.png")
    for name in CURSORS:
        path = os.path.join(tab, "Cursors", f"{name}Image.png")
        if not os.path.isfile(path):
            missing.append(f"Cursors\\{name}Image.png")
            continue
        save(fitted(Image.open(path).convert("RGBA"), CURSOR_SIDE), "cursors", f"{name}.png")

    count = sum(len(files) for _, _, files in os.walk(OUT))
    print(f"{count} pictures in {os.path.normpath(OUT)}")
    if missing:
        print("Not found (the game draws its placeholder):", ", ".join(missing))


if __name__ == "__main__":
    main()
