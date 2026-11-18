# Gebäude (`data\buildings\`)

Jedes Ding hat eine eigene Datei: `<name>.json` (die Werte) und `<name>.md` (die Werte mit Bedeutung, zum Lesen). In `_general.json` stehen die Werte, die für alle zugleich gelten. Das Spiel setzt alle Dateien des Ordners beim Laden zu einem Block zusammen (`units.soldier.hp` ist also dasselbe wie früher in `units\` – so heißen die Werte auch im Editor-Modul Data). **Ein neues Ding** braucht seine `.json` (mit `name`) hier, eine Zeile in `src\data\files.rs` und – wie bisher – seinen Eintrag im Code (`kinds.rs`); ein Test merkt, wenn eins davon fehlt.


Eine Datei pro Typ, mit ihrer `.md` daneben: `command_center`, `barracks` (die Kaserne), `tent`, `wall`, `gate`, `tower`,
`sawmill`, `mine`, `hunter_cottage`, `fishing_hut`, `farm`, `windmill`, `charcoal_burner`, `wood_workshop`,
`tesla_tower`, `stone_workshop`, `munitions_factory`, `stone_house`, `stone_wall`, `stone_tower`, `stakes` (die
Pfahlfalle), `market`, `bank`, `inn` (das Gasthaus – bildet die Söldner aus), `barrel` (das Sprengfass), `foundry` (die
Gießerei), `ballista` (die Balliste), `machine_gun_tower` (der MG-Turm).

| Wert | Bedeutung |
|---|---|
| `name`, `letter` | Name im Spiel, Buchstaben fürs Symbol |
| `size` | `[Breite, Tiefe]` in Kacheln (das Tor lässt sich drehen) |
| `explosion` | Nur die Optik: wie es **fällt** – Effekte in `assets\effects\` (`"wood_big"`, `"stone_medium"` …, TABs `ExplosionOnDestroy`); zwei = abwechselnd. Dörfer: dasselbe je Größe in `data\doom.json` |
| `skins` | Nur die Optik: seine **Aussehen** in `assets\` (`"buildings/tent_1"`, aus `tools\tab_buildings.py`, siehe `assets\README.md`). Jedes Gebäude trägt eins davon, je nach Bauplatz – die Fischerhütte das, das zu ihrem Wasser schaut (TABs vier). Fehlt die Datei, wird es übersprungen. Weglassen oder keins da = Klötze |
| `hp` | Lebenspunkte – zusammen mit `shield` TABs `Life` |
| `armor` | Rüstung der **Lebenspunkte** in Punkten wie StarCraft 2: so viele Punkte gehen von jedem Treffer ab, der die HP erreicht (mindestens 1 kommt durch). TABs Gebäude haben keine – weglassen = 0 |
| `shield_armor` | Rüstung des **Schilds** in Punkten (wie die Schildrüstung der Protoss): so viele Punkte gehen von jedem Treffer ab, der den Schild trifft; die HP behalten ihre eigene `armor`. Weglassen = 0 |
| `attributes` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = gepanzert und ein Gebäude |
| `shield` | Schild in Punkten, **zusätzlich** zu den HP – TABs `DeffensesLife` (Zelt 15, Kaserne 400, Kommandozentrum 2500). Zombies schlagen erst darauf. Bricht er, ist das Gebäude **infiziert** und seine Leute kommen als Zombies heraus. Ohne Kolonisten ist er nur zusätzliches Leben. Bricht der des Kommandozentrums, ist die Runde verloren (TAB). 0 = kein Schild. Im Spiel ein Balken mit den HP: der Schild graublau dahinter |
| `colonists` | Kolonisten darin: zahlen Gold und werden bei der Infektion zu Zombies. Wie viele herauskommen, wie TAB: `colonists` + `worker_cost` − `workers` + `extra_infected`, jeder zufällig aus `infected` (`zombies\`), alle laufen zum Kommandozentrum |
| `extra_infected` | So viele Zombies kommen bei der Infektion **zusätzlich** heraus (TABs `ExtraUnitsWhenInfected`; Zelt 2 → 4 insgesamt, Steinhaus 8 → 16). Weglassen = 0 |
| `workers` | **Arbeiter**, die es gibt (Zelt, Kommandozentrum) – infiziert keine |
| `worker_cost` | Arbeiter, die es braucht, solange es steht. Nicht genug frei: kann nicht gebaut werden (im Editor egal) |
| `cost` | Preis in Gold, bezahlt beim Platzieren (im Editor kostenlos) |
| `cost_wood`, `cost_stone`, `cost_coal`, `cost_iron` | Dazu Holz, Stein, Kohle und Eisen (aus der Gießerei) aus dem Vorrat (weglassen = 0) |
| `requires` | Muss erst stehen, z. B. `"WoodWorkshop"` (Name des Typs wie in den Kartendateien; weglassen = nichts) – wie TABs `BuildingRequisite`. Dazu kann eine Forschung nötig sein (`research.json`) |
| `energy` | Strom, den es ins Netz gibt (Kommandozentrum, Windmühle, Köhlerei) |
| `energy_use` | Strom, den es braucht. Gebaut (oder mit Y wieder an) nur, wenn sein Netz so viel übrig hat; fällt danach Strom weg, arbeitet es anteilig langsamer |
| `power_radius` | Kommandozentrum, Tesla-Turm: so weit (Kacheln ab der Mitte) reicht ihr Netz. Wer Strom erzeugt, braucht oder trägt, muss darin stehen |
| `burns_wood` | Holz pro Abrechnung – ohne Holz kein Strom (Köhlerei, Kraftwerk wie TAB) |
| `food` | Nahrung pro Abrechnung – ohne sie zahlen die Kolonisten kein Gold (Zelt) |
| `food_made` | Nahrung pro Abrechnung aus dem Nichts, ohne Kacheln (Kommandozentrum 20 wie TAB; ein Zelt isst 4) |
| `gold` | Gold pro Abrechnung, mal Stromanteil – Häuser nur, wenn ihre Kolonisten **satt** sind (wie TAB: Zelt 8, Steinhaus 40, Kommandozentrum 200 ohne zu essen) |
| `upkeep` | **Unterhalt:** Gold pro Abrechnung, solange es läuft – aus (Y) oder infiziert: nichts. Weglassen = 0 |
| `gather_range` | Sammler: so viele Kacheln um seine Fläche herum arbeitet es, auf jeder Seite – ein Quadrat (Mine 2 × 2 mit 2 → 6 × 6 wie TAB) |
| `gather_below` | Sammler: `true` = die Kacheln unter ihm zählen mit (Mine und Steinbrüche: das Erz darunter, die Ölplattform: ihre Quelle). Weglassen = `false`. Bei Sägewerk, Jäger und Fischer ohne Wirkung – sie stehen nie auf Wald oder Wasser |
| `trap_damage`, `trap_wear` | Falle: Schaden an jedem Zombie darauf (minus Rüstung) alle `trap_every_seconds`, und so viele eigene HP kostet sie jeder Treffer |
| `slow_percent` | Falle: wer darauf steht – Zombies und deine Einheiten –, läuft so viel Prozent langsamer (TABs Pfähle: 50). Weglassen = 0 |
| `trade_range`, `house_bonus_percent`, `colonists_per_good` | Markt: seine Häuser sind die, deren Mitte auf ihm oder bis so viele Kacheln drumherum liegt (2 → 7 × 7) – jedes Haus nur für einen Markt, den ältesten. Sie zahlen so viel Prozent mehr Gold, und für je so viele Kolonisten darin verkauft er 1 Ware pro Abrechnung |
| `gold_storage`, `credit` | Bank: so viel mehr Gold passt ins Lager, und so viel **Dispo** gibt sie – mit Shift darf man so weit ins Minus kaufen, ohne Schuldenstrafen (Zinsen: `colony.json`) |
| `barrel_storage` | Munitionsfabrik: so viele Sprengfässer fasst ihr Lager |
| `noise` | **Lärm pro Arbeitstakt** (`work_every_seconds` in `sound.json`), solange es arbeitet – mal Stromanteil, aus/infiziert/ohne Holz (Köhlerei) oder ohne Erz (Gießerei) still. Jägerhütte 50, Sägewerk und Munitionsfabrik 100, Mine, Köhlerei und Gießerei 150. Weglassen = 0 |
| `smelts`, `smelt_ore`, `smelt_coal` | Gießerei: so viel **Eisen** schmilzt sie bei jeder Abrechnung höchstens (mal Stromanteil), aus je so viel Eisenerz und Kohle – nur so viel, wie im Vorrat ist |
| `weapon` | Ein Turm, der **selbst schießt** (Balliste, MG-Turm, Shocking Tower, Executor): `damage` pro Schuss (minus Rüstung), `bonus` (Zusatzschaden je Eigenschaft des Zombies, z. B. Balliste `{ "armored": 50 }`), `range` (Kacheln ab der Mitte), `reload_seconds` (bei wenig Strom länger), `noise` pro Schuss, `toughest`: `true` = auf den Zombie mit den meisten HP in Reichweite, sonst auf den nächsten; `all_in_range`: `true` = auf **alle** in Reichweite auf einmal (Shocking Tower); `splash`: trifft jeden so viele Kacheln um sein Ziel (Executor 0,6). Weglassen = schießt nicht |
| `cost_oil` | Dazu **Öl** aus dem Vorrat (TABs späte Gebäude). Weglassen = 0 |
| `burns_coal` | Kohle pro Abrechnung – ohne Kohle kein Strom (wie `burns_wood`; zurzeit keins – für ein späteres Kohlekraftwerk) |
| `converts` | Wandelt bei jeder Abrechnung Waren um, solange es läuft (mal Stromanteil): `takes` und `makes`, je `[["Wood", 20], …]` – der Transmutator Holz, Stein und Eisen in Öl, der Victorious verbraucht Öl. Nur wenn alles da ist |
| `goods_storage` | Lagerhaus: so viel mehr passt von jeder Ware ins Lager – nicht Essen, nicht Gold (TABs `ResourcesStorage` 50) |
| `boost_range`, `gather_boost_percent`, `gold_boost_percent` | TABs Gebäude, die Nachbarn helfen: wessen ganzer Bauplatz in seinem Bauplatz plus `boost_range` Kacheln liegt, sammelt so viel Prozent mehr (Lagerhaus: 12 Kacheln, 20 %) bzw. zahlt als Haus so viel Prozent mehr Gold (Victorious: 24, 20 %) |
| `sees_through_forest` | Sieht **über Wald** (TABs `WatchThroughOpaque`: Radar, Aussichtsturm, Wunder) |
| `only_one` | Nur **eins** davon darf stehen (TABs `MaxInstances` 1 – die Wunder) |
| `veteran_units` | Solange es läuft, ist **jede** deiner Einheiten ein Veteran (die Akademie, TABs `MakeSoldiersVeteran`) – Söldner nicht |
| `mine` | Die **Mine** (Falle): tritt ein Zombie drauf, geht sie nach ihrer Verzögerung (0–3, auf ihrer Karte wie TABs `DelayActivation`) × `delay_step_seconds` hoch – `damage` an allen bis `radius` Kacheln (Zombies, Einheiten, Gebäude, wie ein Fass) – und ist weg |
| `gather_per_tile` | Sammler: so viel bringt jede passende Kachel pro Abrechnung (bei vollem Strom). Was passt, steht im Code: Sägewerk Wald, Jäger Wald, Gras und Erde (jede mit dem Anteil der Karte, `hunting_*` bei `game_maps.json` – TABs 0,8 × Landschaftsfaktor), Fischer Wasser, Mine Erz und Gold (auch unter sich), Farm Gras (nicht Erde, nicht Erz – und sie selbst steht nur auf Gras) |
| `gold_per_tile` | Mine: so viel **Gold** bringt jede Goldkachel pro Abrechnung, direkt ins Gold (TAB: das 20-Fache einer Steinkachel – 10) |
| `build_seconds` | **Bauzeit** wie TAB: so lange baut es sich selbst, nachdem es platziert ist – kurz, länger je größer und je höher im Techtree. Bis dahin tut es nichts (keine Arbeiter, kein Gold, kein Strom, keine Ausbildung), nimmt aber schon seine Arbeiter, und Zombies können es treffen. Weglassen oder 0 = steht sofort. Im Editor steht alles sofort |
| `repair_cost` | Gold für eine **volle** Reparatur (von 0 auf volle HP + Schild); eine kleinere kostet ihren Anteil. Bezahlt beim Start. Später kommen Holz, Stein … dazu |
| `sight` | Sichtweite ab der Mitte |
| `height` | Nur die Optik: wie hoch es gezeichnet wird |
| `joins` | Nur die Optik: verbindet sich mit Mauern zu einer Linie |

Dazu für alle:

| Wert | Bedeutung |
|---|---|
| `tower_slots` | So viele Einheiten passen in einen Turm |
| `tower_bonus` | So viel weiter schießen und sehen Einheiten im Turm |
| `zombie_free_radius` | So viele Kacheln um einen Bauplatz darf kein Zombie stehen |
| `replace_refund_percent` | Wird ein Gebäude überbaut (Tor auf Mauern), kommt so viel Prozent seines Preises zurück. Unter 100 bleiben, sonst wären „erst Mauern, dann Tor“ billiger als das Tor |
| `demolish_refund_percent` | **Abreißen** (Entf): so viel Prozent des Preises kommen zurück – Gold, Holz, Stein, Kohle, Eisen, abgerundet (TABs `FactorResourcesReturn`: 50) |
| `demolish_unfinished_refund_percent` | … eines Gebäudes, das **noch gebaut wird** (TAB: auch 50) |
| `repair_seconds` | So viele Sekunden dauert eine volle Reparatur – eine kleinere entsprechend kürzer |
| `build_start_percent` | Ein Gebäude im Bau fängt mit so viel Prozent seiner HP an und wächst bis voll – erst die HP, dann der Schild. Infiziert werden kann es erst fertig. Ein Treffer im Bau bleibt (später reparieren) |
| `trap_every_seconds` | So oft treffen Fallen die Zombies auf ihnen |
| `sell_from_percent`, `sell_prices` | Märkte verkaufen nur, was über so viel Prozent des Lagers liegt, zu diesen Preisen (Gold pro Einheit: `wood`, `stone`, `iron_ore`, `coal`, `food`, `iron` – das Eisen der Gießerei – und `oil`) |
| `blast` | Sprengfass: `cost` Gold + `cost_coal` Kohle und `build_seconds` in der Munitionsfabrik. Geht es hoch: `damage` an allen bis `radius` Kacheln (Zombies und Einheiten minus Rüstung, Gebäude wie ein Treffer, Häuser der Dörfer wie ein Schuss), so laut wie `noise`. Wer eins trägt, läuft `carry_slow_percent` langsamer und schießt nicht |
| `scan` | Scan des **Radars**: zeigt `seconds` lang einen Kreis von `radius` Kacheln (der Ring; das Licht reicht 2 Kacheln weiter) – auch über Wald. Höchstens `charges` Ladungen, am Anfang keine, alle `every_seconds` eine neue, solange ein Radar läuft – und solange eine lädt, braucht er `power` Strom mehr |

## Upgrades

| Wert | Bedeutung |
|---|---|
| `upgrades` | Was **eine Stufe** einer Upgrade-Reihe (`data\upgrades.json`) diesem Ding gibt – flach wie in SC2 (Marine +1, Marauder +2): `{ "infantry_weapons": { "damage": 2 } }`. Möglich: `hp`, `shield`, `damage`, `bonus_damage` (zu jedem Zusatzschaden von `bonus`), `armor`, `shield_armor` (Punkte), `range`, `sight`, `speed`, `attack_speed` (Prozentpunkte). Weglassen = keine Reihe, keine Wirkung. Die Stufe steigt durch eine Forschung mit `upgrade`, durch die Karte (Start) oder die Trigger-Aktion Research |
