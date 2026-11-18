# Merc soldier

Einheit - Datei `merc_soldier.json`, Schlüssel in der Karte (Modul Data): `units.merc_soldier.<wert>`. Die Zeiten stehen in Sekunden, Entfernungen in Kacheln.

| Wert | Standard | Bedeutung |
|---|---|---|
| `name` | `"Merc soldier"` | Name im Spiel, 1–2 Buchstaben fürs Symbol |
| `letter` | `"MS"` | Name im Spiel, 1–2 Buchstaben fürs Symbol |
| `requires` | `"MunitionsFactory"` | Muss erst stehen, damit sie ausgebildet werden kann – Soldat und Scharfschütze: `"MunitionsFactory"`. Weglassen = nichts. Dazu kann eine Forschung nötig sein (`research.json`) |
| `trained_in` | `"Inn"` | Wo sie ausgebildet wird: `"Barracks"` (weglassen) oder `"Inn"` (die Söldner) |
| `mercenary` | `true` | `true` = Söldner: bei Schulden geht er zuerst, schon ab Stufe 2 (`colony.json`), und trägt eine goldene Marke. Weglassen = `false` |
| `cost` | `600` | Preis in Gold. Bezahlt, sobald sie in die Bauschlange kommt; Abbrechen gibt alles zurück (wie StarCraft) |
| `cost_iron` | `5` | Dazu Holz und Eisen (aus der Gießerei) aus dem Vorrat, wie TAB: Ranger 2 Holz, Soldat 2 Eisen, Scharfschütze beides, Söldner je 5. Abbrechen gibt sie zurück. Weglassen = 0 |
| `worker_cost` | `0` | **Arbeiter**, die sie braucht – ab der Bauschlange, solange sie lebt (stirbt sie, sind sie wieder frei) |
| `upkeep` | `6` | **Unterhalt:** Gold pro Abrechnung, solange sie lebt (noch nicht in der Bauschlange). Reicht das Gold nicht: Schulden (`colony.json`) |
| `food` | `1` | **Nahrung** pro Abrechnung, solange sie lebt – wie TAB (Soldat 1). Die Einheiten essen vor den Häusern; fehlt Nahrung, passiert ihnen nichts, aber den Häusern fehlt sie. Weglassen = 0 |
| `hp` | `296` | Lebenspunkte – TABs HP ÷ (1 − TABs Rüstungsanteil), so braucht jede Waffe so viele Treffer wie in TAB (Soldat 120 → 198, Riese 10.000 → 14.235) |
| `armor` | `0` | Rüstung in **Punkten** wie StarCraft 2: so viele Punkte gehen von **jedem** Zombie-Treffer ab, mindestens 1 kommt immer durch (überall 0: TABs Rüstungsanteil steckt in `hp` – nur Upgrades geben Punkte). Gift und Feuer ignorieren sie. Upgrades geben mehr (`armor` unter `upgrades`) |
| `attributes` | `["armored", "biological"]` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = keine |
| `bonus` | `{}` | **Zusatzschaden** je Eigenschaft des Ziels, auf `damage` drauf – wie SC2s „+10 gegen Gepanzert“: `{ "armored": 25 }`. Hat das Ziel mehrere der genannten Eigenschaften, zählen alle. Eine Upgrade-Stufe mit `bonus_damage` erhöht jeden dieser Zusätze. Weglassen = keiner |
| `damage` | `22` | Schaden pro Schuss |
| `range` | `5.0` | Reichweite |
| `sight` | `6.0` | Sichtweite (mindestens so weit wie die Reichweite) |
| `turn_around_seconds` | `0.35` | So lange braucht sie für eine halbe Drehung (180°). Erst wenn sie zum Ziel schaut, zielt sie |
| `raise_seconds` | `0.1` | Waffe heben, sobald sie steht und ein Ziel hat (Ranger 0,5, Soldat 0,2, Scharfschütze 0,5). Danach bleibt sie oben, solange sie schießt |
| `lower_seconds` | `0.05` | Waffe senken beim Gehen (Ranger 0,25, Soldat 0,1) – das Gehen wartet nicht darauf |
| `aim_seconds` | `0.0` | Zielen vor jedem Schuss, still auf dem Ziel (nur Scharfschütze 1,6). Neues Ziel: das Zielen fängt von vorne an |
| `windup_seconds` | `0.27` | Ausholen: vom Start der Schuss-Animation bis zum Schuss (Ranger und Soldat 0,2). Losgehen bricht den Schuss ab |
| `cooldown_seconds` | `0.4` | Nachladen nach dem Schuss, bevor sie wieder zielen kann – läuft auch beim Gehen (Medic: 0 = Strahl ohne Pause) |
| `speed` | `2.4` | Laufgeschwindigkeit |
| `build_seconds` | `14.0` | Ausbildungszeit in der Kaserne |
| `noise` | `3.0` | **Lärm pro Schuss** wie in TAB (Ranger 1, Soldat 3, Scharfschütze 10). Er sammelt sich dort, wo geschossen wird, und halbiert sich jede Sekunde – ein langer Kampf pendelt sich ein (siehe `sound.json`) |
| `heals` | `false` | `true` = schießt nicht, sondern richtet den Strahl auf eigene Einheiten (Medic) … |
| `venom_factor` | `0.82` | **Gift-Resistenz** wie TABs `VenomDamageFactor`: so viel vom Gift des Spitters kommt durch (TAB: Soldat 0,5, sonst 1 – hier mal den HP-Aufschlag, Soldat 0,83). Gegen Gift hilft keine Rüstung. Pflicht in jedem Block |
| `fire_factor` | `0.82` | **Feuer-Empfindlichkeit** wie TABs `FireDamageFactor`: so viel vom Feuer kommt an (Soldat 0,5, Lucifer 0 – sein Anzug, alle anderen 1). Pflicht in jedem Block |
| `regen` | `0.04` | **Selbstheilung** wie TABs `LifeRegenFactor`: dieser Anteil der vollen HP (aufgerundet) alle `regen_every_seconds`, sobald sie `regen_after_seconds` nicht getroffen wurde (Soldat 0,04, Ranger 0,01, Scharfschütze 0,005; Medic 0,01 und Speerträger 0,04 sind unsere). Pflicht in jedem Block |
| `skins` | `["units/soldier"]` | Nur die Optik: ihre **Aussehen** in `assets\` (`"units/ranger"` = `assets\units\ranger.json`, siehe `assets\README.md`). Jede Einheit trägt eins davon ihr Leben lang. Fehlt die Datei, wird es übersprungen. Weglassen = keins |
| `sprite` | `"test/figure"` | Ihr Bild, wenn keins der `skins` da ist (`"test/figure"` = das graue Testbild). Weglassen = Kästchen |
