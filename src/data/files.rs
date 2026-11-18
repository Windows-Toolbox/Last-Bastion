//! The files of the folders `data\units\`, `data\buildings\` and `data\zombies\` as they were when the program was built.
//! `include_str!` only takes a written-out path, so every file is named here – a new one goes in as well (a test
//! compares these lists with the folders). One line per file, sorted.

/// `data\units\`: the name of the file without `.json`, and its text. `_general` holds the loose values.
pub const UNITS: &[(&str, &str)] = &[
    ("_general", include_str!("../../data/units/_general.json")),
    ("lucifer", include_str!("../../data/units/lucifer.json")),
    ("medic", include_str!("../../data/units/medic.json")),
    ("merc_ranger", include_str!("../../data/units/merc_ranger.json")),
    ("merc_sniper", include_str!("../../data/units/merc_sniper.json")),
    ("merc_soldier", include_str!("../../data/units/merc_soldier.json")),
    ("ranger", include_str!("../../data/units/ranger.json")),
    ("sniper", include_str!("../../data/units/sniper.json")),
    ("soldier", include_str!("../../data/units/soldier.json")),
    ("spearman", include_str!("../../data/units/spearman.json")),
    ("thanatos", include_str!("../../data/units/thanatos.json")),
    ("titan", include_str!("../../data/units/titan.json")),
];

/// `data\buildings\`: the name of the file without `.json`, and its text. `_general` holds the loose values.
pub const BUILDINGS: &[(&str, &str)] = &[
    ("_general", include_str!("../../data/buildings/_general.json")),
    ("academy", include_str!("../../data/buildings/academy.json")),
    ("ballista", include_str!("../../data/buildings/ballista.json")),
    ("bank", include_str!("../../data/buildings/bank.json")),
    ("barracks", include_str!("../../data/buildings/barracks.json")),
    ("barrel", include_str!("../../data/buildings/barrel.json")),
    ("big_farm", include_str!("../../data/buildings/big_farm.json")),
    ("big_mill", include_str!("../../data/buildings/big_mill.json")),
    ("big_quarry", include_str!("../../data/buildings/big_quarry.json")),
    ("blade_trap", include_str!("../../data/buildings/blade_trap.json")),
    ("charcoal_burner", include_str!("../../data/buildings/charcoal_burner.json")),
    ("command_center", include_str!("../../data/buildings/command_center.json")),
    ("crystal_palace", include_str!("../../data/buildings/crystal_palace.json")),
    ("engineering_center", include_str!("../../data/buildings/engineering_center.json")),
    ("executor", include_str!("../../data/buildings/executor.json")),
    ("farm", include_str!("../../data/buildings/farm.json")),
    ("fishing_hut", include_str!("../../data/buildings/fishing_hut.json")),
    ("foundry", include_str!("../../data/buildings/foundry.json")),
    ("gate", include_str!("../../data/buildings/gate.json")),
    ("hunter_cottage", include_str!("../../data/buildings/hunter_cottage.json")),
    ("inn", include_str!("../../data/buildings/inn.json")),
    ("lookout_tower", include_str!("../../data/buildings/lookout_tower.json")),
    ("machine_gun_tower", include_str!("../../data/buildings/machine_gun_tower.json")),
    ("market", include_str!("../../data/buildings/market.json")),
    ("mine_trap", include_str!("../../data/buildings/mine_trap.json")),
    ("mine", include_str!("../../data/buildings/mine.json")),
    ("munitions_factory", include_str!("../../data/buildings/munitions_factory.json")),
    ("oil_platform", include_str!("../../data/buildings/oil_platform.json")),
    ("power_plant", include_str!("../../data/buildings/power_plant.json")),
    ("quarry", include_str!("../../data/buildings/quarry.json")),
    ("radar", include_str!("../../data/buildings/radar.json")),
    ("sawmill", include_str!("../../data/buildings/sawmill.json")),
    ("shocking_tower", include_str!("../../data/buildings/shocking_tower.json")),
    ("spire", include_str!("../../data/buildings/spire.json")),
    ("stakes", include_str!("../../data/buildings/stakes.json")),
    ("stone_house", include_str!("../../data/buildings/stone_house.json")),
    ("stone_tower", include_str!("../../data/buildings/stone_tower.json")),
    ("stone_wall", include_str!("../../data/buildings/stone_wall.json")),
    ("stone_workshop", include_str!("../../data/buildings/stone_workshop.json")),
    ("tent", include_str!("../../data/buildings/tent.json")),
    ("tesla_tower", include_str!("../../data/buildings/tesla_tower.json")),
    ("tower", include_str!("../../data/buildings/tower.json")),
    ("train_station", include_str!("../../data/buildings/train_station.json")),
    ("transmutator", include_str!("../../data/buildings/transmutator.json")),
    ("victorious", include_str!("../../data/buildings/victorious.json")),
    ("wall", include_str!("../../data/buildings/wall.json")),
    ("warehouse", include_str!("../../data/buildings/warehouse.json")),
    ("windmill", include_str!("../../data/buildings/windmill.json")),
    ("wood_house", include_str!("../../data/buildings/wood_house.json")),
    ("wood_workshop", include_str!("../../data/buildings/wood_workshop.json")),
];

/// `data\zombies\`: the name of the file without `.json`, and its text. `_general` holds the loose values.
pub const ZOMBIES: &[(&str, &str)] = &[
    ("_general", include_str!("../../data/zombies/_general.json")),
    ("armored", include_str!("../../data/zombies/armored.json")),
    ("fat", include_str!("../../data/zombies/fat.json")),
    ("giant", include_str!("../../data/zombies/giant.json")),
    ("harpy", include_str!("../../data/zombies/harpy.json")),
    ("infested", include_str!("../../data/zombies/infested.json")),
    ("runner", include_str!("../../data/zombies/runner.json")),
    ("spitter", include_str!("../../data/zombies/spitter.json")),
    ("walker", include_str!("../../data/zombies/walker.json")),
];

/// The built-in files of a folder type (`None`: it is a single file).
pub fn of(folder: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match folder {
        "units" => Some(UNITS),
        "buildings" => Some(BUILDINGS),
        "zombies" => Some(ZOMBIES),
        _ => None,
    }
}
