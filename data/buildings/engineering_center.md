# Engineering Center

Gebäude - Datei `engineering_center.json`, Schlüssel in der Karte (Modul Data): `buildings.engineering_center.<wert>`. Die Zeiten stehen in Sekunden, Entfernungen in Kacheln.

| Wert | Standard | Bedeutung |
|---|---|---|
| `name` | `"Engineering Center"` | Name im Spiel, Buchstaben fürs Symbol |
| `letter` | `"EC"` | Name im Spiel, Buchstaben fürs Symbol |
| `skins` | `["buildings/engineering_center"]` | Nur die Optik: seine **Aussehen** in `assets\` (`"buildings/tent_1"`, aus `tools\tab_buildings.py`, siehe `assets\README.md`). Jedes Gebäude trägt eins davon, je nach Bauplatz – die Fischerhütte das, das zu ihrem Wasser schaut (TABs vier). Fehlt die Datei, wird es übersprungen. Weglassen oder keins da = Klötze |
| `explosion` | `["stone_big"]` | Nur die Optik: wie es **fällt** – Effekte in `assets\effects\` (`"wood_big"`, `"stone_medium"` …, TABs `ExplosionOnDestroy`); zwei = abwechselnd. Dörfer: dasselbe je Größe in `data\doom.json` |
| `size` | `[3, 3]` | `[Breite, Tiefe]` in Kacheln (das Tor lässt sich drehen) |
| `hp` | `500` | Lebenspunkte – zusammen mit `shield` TABs `Life` |
| `colonists` | `0` | Kolonisten darin: zahlen Gold und werden bei der Infektion zu Zombies. Wie viele herauskommen, wie TAB: `colonists` + `worker_cost` − `workers` + `extra_infected`, jeder zufällig aus `infected` (`zombies.json`), alle laufen zum Kommandozentrum |
| `workers` | `0` | **Arbeiter**, die es gibt (Zelt, Kommandozentrum) – infiziert keine |
| `worker_cost` | `30` | Arbeiter, die es braucht, solange es steht. Nicht genug frei: kann nicht gebaut werden (im Editor egal) |
| `cost` | `3000` | Preis in Gold, bezahlt beim Platzieren (im Editor kostenlos) |
| `cost_stone` | `40` | Dazu Holz, Stein, Kohle und Eisen (aus der Gießerei) aus dem Vorrat (weglassen = 0) |
| `cost_iron` | `40` | Dazu Holz, Stein, Kohle und Eisen (aus der Gießerei) aus dem Vorrat (weglassen = 0) |
| `cost_oil` | `20` | Dazu **Öl** aus dem Vorrat (TABs späte Gebäude). Weglassen = 0 |
| `requires` | `"Foundry"` | Muss erst stehen, z. B. `"WoodWorkshop"` (Name des Typs wie in den Kartendateien; weglassen = nichts) – wie TABs `BuildingRequisite`. Dazu kann eine Forschung nötig sein (`research.json`) |
| `upkeep` | `100` | **Unterhalt:** Gold pro Abrechnung, solange es läuft – aus (Y) oder infiziert: nichts. Weglassen = 0 |
| `energy_use` | `50` | Strom, den es braucht. Gebaut (oder mit Y wieder an) nur, wenn sein Netz so viel übrig hat; fällt danach Strom weg, arbeitet es anteilig langsamer |
| `build_seconds` | `56.0` | **Bauzeit** wie TAB: so lange baut es sich selbst, nachdem es platziert ist – kurz, länger je größer und je höher im Techtree. Bis dahin tut es nichts (keine Arbeiter, kein Gold, kein Strom, keine Ausbildung), nimmt aber schon seine Arbeiter, und Zombies können es treffen. Weglassen oder 0 = steht sofort. Im Editor steht alles sofort |
| `sight` | `7.0` | Sichtweite ab der Mitte |
| `height` | `0.8` | Nur die Optik: wie hoch es gezeichnet wird |
| `repair_cost` | `50` | Gold für eine **volle** Reparatur (von 0 auf volle HP + Schild); eine kleinere kostet ihren Anteil. Bezahlt beim Start. Später kommen Holz, Stein … dazu |
| `joins` | `false` | Nur die Optik: verbindet sich mit Mauern zu einer Linie |
| `armor` | `0` | Rüstung der **Lebenspunkte** in Punkten wie StarCraft 2: so viele Punkte gehen von jedem Treffer ab, der die HP erreicht (mindestens 1 kommt durch). TABs Gebäude haben keine – weglassen = 0 |
| `shield_armor` | `0` | Rüstung des **Schilds** in Punkten (wie die Schildrüstung der Protoss): so viele Punkte gehen von jedem Treffer ab, der den Schild trifft; die HP behalten ihre eigene `armor`. Weglassen = 0 |
| `attributes` | `["armored", "structure"]` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = gepanzert und ein Gebäude |
| `shield` | `500` | Schild in Punkten, **zusätzlich** zu den HP – TABs `DeffensesLife` (Zelt 15, Kaserne 400, Kommandozentrum 2500). Zombies schlagen erst darauf. Bricht er, ist das Gebäude **infiziert** und seine Leute kommen als Zombies heraus. Ohne Kolonisten ist er nur zusätzliches Leben. Bricht der des Kommandozentrums, ist die Runde verloren (TAB). 0 = kein Schild. Im Spiel ein Balken mit den HP: der Schild graublau dahinter |
| `upgrades` | `{ "structure_armor": { "armor": 1, "shield_armor": 1 } }` | Was **eine Stufe** einer Upgrade-Reihe (`data\upgrades.json`) diesem Ding gibt – flach wie in SC2 (Marine +1, Marauder +2): `{ "infantry_weapons": { "damage": 2 } }`. Möglich: `hp`, `shield`, `damage`, `bonus_damage` (zu jedem Zusatzschaden von `bonus`), `armor`, `shield_armor` (Punkte), `range`, `sight`, `speed`, `attack_speed` (Prozentpunkte). Weglassen = keine Reihe, keine Wirkung. Die Stufe steigt durch eine Forschung mit `upgrade`, durch die Karte (Start) oder die Trigger-Aktion Research |
