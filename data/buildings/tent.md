# Tent

Gebäude - Datei `tent.json`, Schlüssel in der Karte (Modul Data): `buildings.tent.<wert>`. Die Zeiten stehen in Sekunden, Entfernungen in Kacheln.

| Wert | Standard | Bedeutung |
|---|---|---|
| `name` | `"Tent"` | Name im Spiel, Buchstaben fürs Symbol |
| `letter` | `"T"` | Name im Spiel, Buchstaben fürs Symbol |
| `skins` | `["buildings/tent_1", "buildings/tent_2", "buildings/tent_3", "buildings/tent_4"]` | Nur die Optik: seine **Aussehen** in `assets\` (`"buildings/tent_1"`, aus `tools\tab_buildings.py`, siehe `assets\README.md`). Jedes Gebäude trägt eins davon, je nach Bauplatz – die Fischerhütte das, das zu ihrem Wasser schaut (TABs vier). Fehlt die Datei, wird es übersprungen. Weglassen oder keins da = Klötze |
| `explosion` | `["wood_big"]` | Nur die Optik: wie es **fällt** – Effekte in `assets\effects\` (`"wood_big"`, `"stone_medium"` …, TABs `ExplosionOnDestroy`); zwei = abwechselnd. Dörfer: dasselbe je Größe in `data\doom.json` |
| `size` | `[2, 2]` | `[Breite, Tiefe]` in Kacheln (das Tor lässt sich drehen) |
| `hp` | `45` | Lebenspunkte – zusammen mit `shield` TABs `Life` |
| `colonists` | `4` | Kolonisten darin: zahlen Gold und werden bei der Infektion zu Zombies. Wie viele herauskommen, wie TAB: `colonists` + `worker_cost` − `workers` + `extra_infected`, jeder zufällig aus `infected` (`zombies.json`), alle laufen zum Kommandozentrum |
| `extra_infected` | `2` | So viele Zombies kommen bei der Infektion **zusätzlich** heraus (TABs `ExtraUnitsWhenInfected`; Zelt 2 → 4 insgesamt, Steinhaus 8 → 16). Weglassen = 0 |
| `workers` | `2` | **Arbeiter**, die es gibt (Zelt, Kommandozentrum) – infiziert keine |
| `worker_cost` | `0` | Arbeiter, die es braucht, solange es steht. Nicht genug frei: kann nicht gebaut werden (im Editor egal) |
| `cost` | `30` | Preis in Gold, bezahlt beim Platzieren (im Editor kostenlos) |
| `energy_use` | `1` | Strom, den es braucht. Gebaut (oder mit Y wieder an) nur, wenn sein Netz so viel übrig hat; fällt danach Strom weg, arbeitet es anteilig langsamer |
| `food` | `4` | Nahrung pro Abrechnung – ohne sie zahlen die Kolonisten kein Gold (Zelt) |
| `gold` | `8` | Gold pro Abrechnung, mal Stromanteil – Häuser nur, wenn ihre Kolonisten **satt** sind (wie TAB: Zelt 8, Steinhaus 40, Kommandozentrum 200 ohne zu essen) |
| `build_seconds` | `20` | **Bauzeit** wie TAB: so lange baut es sich selbst, nachdem es platziert ist – kurz, länger je größer und je höher im Techtree. Bis dahin tut es nichts (keine Arbeiter, kein Gold, kein Strom, keine Ausbildung), nimmt aber schon seine Arbeiter, und Zombies können es treffen. Weglassen oder 0 = steht sofort. Im Editor steht alles sofort |
| `repair_cost` | `50` | Gold für eine **volle** Reparatur (von 0 auf volle HP + Schild); eine kleinere kostet ihren Anteil. Bezahlt beim Start. Später kommen Holz, Stein … dazu |
| `sight` | `6.0` | Sichtweite ab der Mitte |
| `height` | `0.45` | Nur die Optik: wie hoch es gezeichnet wird |
| `joins` | `false` | Nur die Optik: verbindet sich mit Mauern zu einer Linie |
| `armor` | `0` | Rüstung der **Lebenspunkte** in Punkten wie StarCraft 2: so viele Punkte gehen von jedem Treffer ab, der die HP erreicht (mindestens 1 kommt durch). TABs Gebäude haben keine – weglassen = 0 |
| `shield_armor` | `0` | Rüstung des **Schilds** in Punkten (wie die Schildrüstung der Protoss): so viele Punkte gehen von jedem Treffer ab, der den Schild trifft; die HP behalten ihre eigene `armor`. Weglassen = 0 |
| `attributes` | `["armored", "structure"]` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = gepanzert und ein Gebäude |
| `shield` | `15` | Schild in Punkten, **zusätzlich** zu den HP – TABs `DeffensesLife` (Zelt 15, Kaserne 400, Kommandozentrum 2500). Zombies schlagen erst darauf. Bricht er, ist das Gebäude **infiziert** und seine Leute kommen als Zombies heraus. Ohne Kolonisten ist er nur zusätzliches Leben. Bricht der des Kommandozentrums, ist die Runde verloren (TAB). 0 = kein Schild. Im Spiel ein Balken mit den HP: der Schild graublau dahinter |
| `upgrades` | `{ "structure_armor": { "armor": 1, "shield_armor": 1 } }` | Was **eine Stufe** einer Upgrade-Reihe (`data\upgrades.json`) diesem Ding gibt – flach wie in SC2 (Marine +1, Marauder +2): `{ "infantry_weapons": { "damage": 2 } }`. Möglich: `hp`, `shield`, `damage`, `bonus_damage` (zu jedem Zusatzschaden von `bonus`), `armor`, `shield_armor` (Punkte), `range`, `sight`, `speed`, `attack_speed` (Prozentpunkte). Weglassen = keine Reihe, keine Wirkung. Die Stufe steigt durch eine Forschung mit `upgrade`, durch die Karte (Start) oder die Trigger-Aktion Research |
