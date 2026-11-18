# Allgemeine Werte: buildings

Gelten für alle in `data\buildings\` zugleich, nicht für ein einzelnes Ding. Schlüssel in der Karte: `buildings.<wert>`.

| Wert | Standard | Bedeutung |
|---|---|---|
| `sell_from_percent` | `80` | Märkte verkaufen nur, was über so viel Prozent des Lagers liegt, zu diesen Preisen (Gold pro Einheit: `wood`, `stone`, `iron_ore`, `coal`, `food`, `iron` – das Eisen der Gießerei – und `oil`) |
| `sell_prices` | `{ "wood": 2, "stone": 3, "iron_ore": 3, "coal": 3, "food": 1, "iron": 10, "oil": 10 }` | Märkte verkaufen nur, was über so viel Prozent des Lagers liegt, zu diesen Preisen (Gold pro Einheit: `wood`, `stone`, `iron_ore`, `coal`, `food`, `iron` – das Eisen der Gießerei – und `oil`) |
| `blast` | `{ "cost": 40, "cost_coal": 2, "build_seconds": 8.0, "damage": 1000, "radius": 2.5, "noise": 400, "carry_slo...` | Sprengfass: `cost` Gold + `cost_coal` Kohle und `build_seconds` in der Munitionsfabrik. Geht es hoch: `damage` an allen bis `radius` Kacheln (Zombies und Einheiten minus Rüstung, Gebäude wie ein Treffer, Häuser der Dörfer wie ein Schuss), so laut wie `noise`. Wer eins trägt, läuft `carry_slow_percent` langsamer und schießt nicht |
| `scan` | `{ "radius": 10.0, "seconds": 10.0, "charges": 2, "every_seconds": 120.0, "power": 30 }` | Scan des **Radars**: zeigt `seconds` lang einen Kreis von `radius` Kacheln (der Ring; das Licht reicht 2 Kacheln weiter) – auch über Wald. Höchstens `charges` Ladungen, am Anfang keine, alle `every_seconds` eine neue, solange ein Radar läuft – und solange eine lädt, braucht er `power` Strom mehr |
| `trap_every_seconds` | `0.5` | So oft treffen Fallen die Zombies auf ihnen |
| `tower_slots` | `4` | So viele Einheiten passen in einen Turm |
| `tower_bonus` | `3.0` | So viel weiter schießen und sehen Einheiten im Turm |
| `zombie_free_radius` | `4` | So viele Kacheln um einen Bauplatz darf kein Zombie stehen |
| `replace_refund_percent` | `50` | Wird ein Gebäude überbaut (Tor auf Mauern), kommt so viel Prozent seines Preises zurück. Unter 100 bleiben, sonst wären „erst Mauern, dann Tor“ billiger als das Tor |
| `demolish_refund_percent` | `50` | **Abreißen** (Entf): so viel Prozent des Preises kommen zurück – Gold, Holz, Stein, Kohle, Eisen, abgerundet (TABs `FactorResourcesReturn`: 50) |
| `demolish_unfinished_refund_percent` | `50` | … eines Gebäudes, das **noch gebaut wird** (TAB: auch 50) |
| `repair_seconds` | `10` | So viele Sekunden dauert eine volle Reparatur – eine kleinere entsprechend kürzer |
| `build_start_percent` | `20` | Ein Gebäude im Bau fängt mit so viel Prozent seiner HP an und wächst bis voll – erst die HP, dann der Schild. Infiziert werden kann es erst fertig. Ein Treffer im Bau bleibt (später reparieren) |
