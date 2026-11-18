# Thanatos

Einheit - Datei `thanatos.json`, Schlüssel in der Karte (Modul Data): `units.thanatos.<wert>`. Die Zeiten stehen in Sekunden, Entfernungen in Kacheln.

| Wert | Standard | Bedeutung |
|---|---|---|
| `name` | `"Thanatos"` | Name im Spiel, 1–2 Buchstaben fürs Symbol |
| `letter` | `"Th"` | Name im Spiel, 1–2 Buchstaben fürs Symbol |
| `requires` | `"Foundry"` | Muss erst stehen, damit sie ausgebildet werden kann – Soldat und Scharfschütze: `"MunitionsFactory"`. Weglassen = nichts. Dazu kann eine Forschung nötig sein (`research.json`) |
| `trained_in` | `"EngineeringCenter"` | Wo sie ausgebildet wird: `"Barracks"` (weglassen) oder `"Inn"` (die Söldner) |
| `cost` | `600` | Preis in Gold. Bezahlt, sobald sie in die Bauschlange kommt; Abbrechen gibt alles zurück (wie StarCraft) |
| `cost_iron` | `10` | Dazu Holz und Eisen (aus der Gießerei) aus dem Vorrat, wie TAB: Ranger 2 Holz, Soldat 2 Eisen, Scharfschütze beides, Söldner je 5. Abbrechen gibt sie zurück. Weglassen = 0 |
| `cost_oil` | `20` | **Öl**: so viel kostet sie (TAB: Lucifer 15, Thanatos 20, Titan 40) und so viel verbraucht sie pro Abrechnung, solange sie lebt (TABs `OilGen`: 1, Titan 2) – wie ihr Essen. Weglassen = 0 |
| `worker_cost` | `1` | **Arbeiter**, die sie braucht – ab der Bauschlange, solange sie lebt (stirbt sie, sind sie wieder frei) |
| `upkeep` | `16` | **Unterhalt:** Gold pro Abrechnung, solange sie lebt (noch nicht in der Bauschlange). Reicht das Gold nicht: Schulden (`colony.json`) |
| `food` | `2` | **Nahrung** pro Abrechnung, solange sie lebt – wie TAB (Soldat 1). Die Einheiten essen vor den Häusern; fehlt Nahrung, passiert ihnen nichts, aber den Häusern fehlt sie. Weglassen = 0 |
| `oil` | `1` | **Öl**: so viel kostet sie (TAB: Lucifer 15, Thanatos 20, Titan 40) und so viel verbraucht sie pro Abrechnung, solange sie lebt (TABs `OilGen`: 1, Titan 2) – wie ihr Essen. Weglassen = 0 |
| `hp` | `337` | Lebenspunkte – TABs HP ÷ (1 − TABs Rüstungsanteil), so braucht jede Waffe so viele Treffer wie in TAB (Soldat 120 → 198, Riese 10.000 → 14.235) |
| `armor` | `0` | Rüstung in **Punkten** wie StarCraft 2: so viele Punkte gehen von **jedem** Zombie-Treffer ab, mindestens 1 kommt immer durch (überall 0: TABs Rüstungsanteil steckt in `hp` – nur Upgrades geben Punkte). Gift und Feuer ignorieren sie. Upgrades geben mehr (`armor` unter `upgrades`) |
| `attributes` | `["armored", "mechanical"]` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = keine |
| `bonus` | `{}` | **Zusatzschaden** je Eigenschaft des Ziels, auf `damage` drauf – wie SC2s „+10 gegen Gepanzert“: `{ "armored": 25 }`. Hat das Ziel mehrere der genannten Eigenschaften, zählen alle. Eine Upgrade-Stufe mit `bonus_damage` erhöht jeden dieser Zusätze. Weglassen = keiner |
| `damage` | `70` | Schaden pro Schuss |
| `range` | `10.0` | Reichweite |
| `min_range` | `3.0` | Nichts näher als so viele Kacheln (Thanatos' Rakete 3, TABs `MinTargetDistance`) – außer sein `melee` reicht hin. Weglassen = 0 |
| `splash` | `1.8` | **Flächenschaden** (TABs `EffectRadius`, `MinFactorDamageInArea`): der Schuss trifft jeden in so vielen Kacheln um den Einschlag, am Rand nur `edge_factor` davon (Titan 0,8 / 1, Thanatos' Rakete 1,8 / 0,2). Weglassen = nur das Ziel |
| `edge_factor` | `0.2` | **Flächenschaden** (TABs `EffectRadius`, `MinFactorDamageInArea`): der Schuss trifft jeden in so vielen Kacheln um den Einschlag, am Rand nur `edge_factor` davon (Titan 0,8 / 1, Thanatos' Rakete 1,8 / 0,2). Weglassen = nur das Ziel |
| `projectile_speed` | `12.0` | **Pfeil:** fliegt so viele Kacheln pro Sekunde (TABs `ProyectileSpeed`, Ranger 12) und trifft erst beim Ankommen – immer das Ziel, auf das er geschossen wurde, wenn es dann noch lebt. Gezielt wird wie in TAB dorthin, wo der Zombie sein wird. Weglassen = Kugel, trifft sofort |
| `melee` | `{ "damage": 20, "range": 1.5, "cone_degrees": 120.0, "edge_factor": 0.25, "windup_seconds": 0.53, "cooldown...` | **Zweiter Angriff aus der Nähe** (Thanatos schlägt mit dem Werfer): `damage`, `range`, `cone_degrees`, `edge_factor`, `windup_seconds`, `cooldown_seconds` – benutzt, wenn das Ziel so nah ist |
| `sight` | `12.0` | Sichtweite (mindestens so weit wie die Reichweite) |
| `turn_around_seconds` | `0.5` | So lange braucht sie für eine halbe Drehung (180°). Erst wenn sie zum Ziel schaut, zielt sie |
| `raise_seconds` | `1.5` | Waffe heben, sobald sie steht und ein Ziel hat (Ranger 0,5, Soldat 0,2, Scharfschütze 0,5). Danach bleibt sie oben, solange sie schießt |
| `lower_seconds` | `1.0` | Waffe senken beim Gehen (Ranger 0,25, Soldat 0,1) – das Gehen wartet nicht darauf |
| `aim_seconds` | `2.4` | Zielen vor jedem Schuss, still auf dem Ziel (nur Scharfschütze 1,6). Neues Ziel: das Zielen fängt von vorne an |
| `windup_seconds` | `0.13` | Ausholen: vom Start der Schuss-Animation bis zum Schuss (Ranger und Soldat 0,2). Losgehen bricht den Schuss ab |
| `cooldown_seconds` | `0.8` | Nachladen nach dem Schuss, bevor sie wieder zielen kann – läuft auch beim Gehen (Medic: 0 = Strahl ohne Pause) |
| `speed` | `1.8` | Laufgeschwindigkeit |
| `build_seconds` | `36.4` | Ausbildungszeit in der Kaserne |
| `noise` | `500.0` | **Lärm pro Schuss** wie in TAB (Ranger 1, Soldat 3, Scharfschütze 10). Er sammelt sich dort, wo geschossen wird, und halbiert sich jede Sekunde – ein langer Kampf pendelt sich ein (siehe `sound.json`) |
| `heals` | `false` | `true` = schießt nicht, sondern richtet den Strahl auf eigene Einheiten (Medic) … |
| `venom_factor` | `1.35` | **Gift-Resistenz** wie TABs `VenomDamageFactor`: so viel vom Gift des Spitters kommt durch (TAB: Soldat 0,5, sonst 1 – hier mal den HP-Aufschlag, Soldat 0,83). Gegen Gift hilft keine Rüstung. Pflicht in jedem Block |
| `fire_factor` | `1.35` | **Feuer-Empfindlichkeit** wie TABs `FireDamageFactor`: so viel vom Feuer kommt an (Soldat 0,5, Lucifer 0 – sein Anzug, alle anderen 1). Pflicht in jedem Block |
| `regen` | `0.01` | **Selbstheilung** wie TABs `LifeRegenFactor`: dieser Anteil der vollen HP (aufgerundet) alle `regen_every_seconds`, sobald sie `regen_after_seconds` nicht getroffen wurde (Soldat 0,04, Ranger 0,01, Scharfschütze 0,005; Medic 0,01 und Speerträger 0,04 sind unsere). Pflicht in jedem Block |
| `skins` | `["units/thanatos"]` | Nur die Optik: ihre **Aussehen** in `assets\` (`"units/ranger"` = `assets\units\ranger.json`, siehe `assets\README.md`). Jede Einheit trägt eins davon ihr Leben lang. Fehlt die Datei, wird es übersprungen. Weglassen = keins |
| `sprite` | `"test/figure"` | Ihr Bild, wenn keins der `skins` da ist (`"test/figure"` = das graue Testbild). Weglassen = Kästchen |
