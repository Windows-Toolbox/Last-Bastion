# Einheiten (`data\units\`)

Jedes Ding hat eine eigene Datei: `<name>.json` (die Werte) und `<name>.md` (die Werte mit Bedeutung, zum Lesen). In `_general.json` stehen die Werte, die für alle zugleich gelten. Das Spiel setzt alle Dateien des Ordners beim Laden zu einem Block zusammen (`units.soldier.hp` ist also dasselbe wie früher in `units\` – so heißen die Werte auch im Editor-Modul Data). **Ein neues Ding** braucht seine `.json` (mit `name`) hier, eine Zeile in `src\data\files.rs` und – wie bisher – seinen Eintrag im Code (`kinds.rs`); ein Test merkt, wenn eins davon fehlt.


Eine Datei pro Typ, mit ihrer `.md` daneben: `soldier`, `ranger`, `sniper`, `medic`, `spearman` (der Speerträger) – und die Söldner
`merc_soldier`, `merc_ranger`, `merc_sniper` (aus dem Gasthaus) – und TABs späte Einheiten `lucifer`, `thanatos`,
`titan` (aus dem Ingenieurszentrum). Ranger, Soldat, Scharfschütze, Lucifer, Thanatos und Titan haben TABs Werte
(`Entities` und `Commands` in `ZXRules`), die Söldner unsere 2,5×-Regel, Medic und Speerträger sind unsere.
Dazu der Block `veterans` (unten) und für alle:

- `regen_every_seconds` (1), `regen_after_seconds` (2): **Selbstheilung** wie TAB – so oft heilen sich Einheiten
  **und Zombies** um ihr `regen`, aber erst so lange nach dem letzten Treffer. Schlafende Zombies heilen auch.
- `dismiss_refund_percent` (50): **Entlassen** (Entf) gibt so viel Prozent ihres Holzes und Eisens zurück,
  aufgerundet (halbe 3 Holz = 2) – kein Gold.
- `search_seconds` (0,35): So oft sieht sich eine Einheit **ohne Ziel** nach einem neuen um – wie TAB (1/3 s). Nach
  einem Kill steht sie also kurz still, die Waffe bleibt oben. Im Angriffsmarsch und auf Patrouille sucht sie jeden
  Tick (sonst liefe sie bis zur nächsten Suche weiter).

Ein Schuss geht wie in TAB: **Waffe heben** (`raise_seconds`, nur wenn sie unten ist) und gleichzeitig **drehen** →
**nachladen** abwarten (zählt ab dem letzten Schuss) → **zielen** (`aim_seconds`, nur der Scharfschütze) →
**ausholen** (`windup_seconds`, die Schuss-Animation läuft) → Schuss. Ein Schuss dauert also
`aim_seconds + windup_seconds + cooldown_seconds` – zusammen TABs Zeit zwischen zwei Schüssen.

**Animation Cancelling:** Wer beim Ausholen losläuft, bricht den Schuss ab (kein Schaden, kein Nachladen). Wer gleich
nach dem Schuss losläuft, verliert nichts: das Nachladen läuft beim Gehen weiter. Beim Gehen sinkt die Waffe
(`lower_seconds`); nach einem kurzen Schritt ist sie nur halb unten und schneller wieder oben. Ein neuer Befehl –
auch derselbe Angriff nochmal – fängt Zielen und Ausholen von vorn an.

Die Zeiten von Ranger, Soldat und Scharfschütze sind TABs (`Commands.tsv`): heben = `TimeLoad`, senken = `TimeUnload`,
zielen = `TimePreAction`, ausholen = `TimeAction × TimeFactorAction`, nachladen = der Rest von `TimeAction` +
`TimePostAction`. Die Söldner haben TABs Veteranen-Zeiten.

**So wie TAB sie spielt:** TABs Einheiten und Zombies denken nur alle 133 ms (jedes 4. seiner 30 KI-Bilder pro
Sekunde) – jede Wartezeit endet erst an so einem Takt, und der nächste Schuss beginnt einen Takt danach. Die Zeiten
hier sind darum schon die gespielten (einmal so umgerechnet, das Spiel rechnet nichts mehr um): Soldat 0,2 + 0,3 s aus
`Commands.tsv` → **0,27 + 0,53 s**, ein Schuss alle 0,8 s; Walker 0,53 s ausholen, alle 1,2 s ein Schlag. Wer zielt,
ausholt oder nachlädt, wie lange, steht also genau so hier – ändern, wie man will.

| Wert | Bedeutung |
|---|---|
| `name`, `letter` | Name im Spiel, 1–2 Buchstaben fürs Symbol |
| `cost` | Preis in Gold. Bezahlt, sobald sie in die Bauschlange kommt; Abbrechen gibt alles zurück (wie StarCraft) |
| `cost_wood`, `cost_iron` | Dazu Holz und Eisen (aus der Gießerei) aus dem Vorrat, wie TAB: Ranger 2 Holz, Soldat 2 Eisen, Scharfschütze beides, Söldner je 5. Abbrechen gibt sie zurück. Weglassen = 0 |
| `worker_cost` | **Arbeiter**, die sie braucht – ab der Bauschlange, solange sie lebt (stirbt sie, sind sie wieder frei) |
| `upkeep` | **Unterhalt:** Gold pro Abrechnung, solange sie lebt (noch nicht in der Bauschlange). Reicht das Gold nicht: Schulden (`colony.json`) |
| `food` | **Nahrung** pro Abrechnung, solange sie lebt – wie TAB (Soldat 1). Die Einheiten essen vor den Häusern; fehlt Nahrung, passiert ihnen nichts, aber den Häusern fehlt sie. Weglassen = 0 |
| `hp` | Lebenspunkte – TABs HP ÷ (1 − TABs Rüstungsanteil), so braucht jede Waffe so viele Treffer wie in TAB (Soldat 120 → 198, Riese 10.000 → 14.235) |
| `armor` | Rüstung in **Punkten** wie StarCraft 2: so viele Punkte gehen von **jedem** Zombie-Treffer ab, mindestens 1 kommt immer durch (überall 0: TABs Rüstungsanteil steckt in `hp` – nur Upgrades geben Punkte). Gift und Feuer ignorieren sie. Upgrades geben mehr (`armor` unter `upgrades`) |
| `attributes` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = keine |
| `bonus` | **Zusatzschaden** je Eigenschaft des Ziels, auf `damage` drauf – wie SC2s „+10 gegen Gepanzert“: `{ "armored": 25 }`. Hat das Ziel mehrere der genannten Eigenschaften, zählen alle. Eine Upgrade-Stufe mit `bonus_damage` erhöht jeden dieser Zusätze. Weglassen = keiner |
| `venom_factor` | **Gift-Resistenz** wie TABs `VenomDamageFactor`: so viel vom Gift des Spitters kommt durch (TAB: Soldat 0,5, sonst 1 – hier mal den HP-Aufschlag, Soldat 0,83). Gegen Gift hilft keine Rüstung. Pflicht in jedem Block |
| `regen` | **Selbstheilung** wie TABs `LifeRegenFactor`: dieser Anteil der vollen HP (aufgerundet) alle `regen_every_seconds`, sobald sie `regen_after_seconds` nicht getroffen wurde (Soldat 0,04, Ranger 0,01, Scharfschütze 0,005; Medic 0,01 und Speerträger 0,04 sind unsere). Pflicht in jedem Block |
| `damage` | Schaden pro Schuss |
| `range` | Reichweite |
| `sight` | Sichtweite (mindestens so weit wie die Reichweite) |
| `turn_around_seconds` | So lange braucht sie für eine halbe Drehung (180°). Erst wenn sie zum Ziel schaut, zielt sie |
| `raise_seconds` | Waffe heben, sobald sie steht und ein Ziel hat (Ranger 0,5, Soldat 0,2, Scharfschütze 0,5). Danach bleibt sie oben, solange sie schießt |
| `lower_seconds` | Waffe senken beim Gehen (Ranger 0,25, Soldat 0,1) – das Gehen wartet nicht darauf |
| `aim_seconds` | Zielen vor jedem Schuss, still auf dem Ziel (nur Scharfschütze 1,6). Neues Ziel: das Zielen fängt von vorne an |
| `windup_seconds` | Ausholen: vom Start der Schuss-Animation bis zum Schuss (Ranger und Soldat 0,2). Losgehen bricht den Schuss ab |
| `cooldown_seconds` | Nachladen nach dem Schuss, bevor sie wieder zielen kann – läuft auch beim Gehen (Medic: 0 = Strahl ohne Pause) |
| `projectile_speed` | **Pfeil:** fliegt so viele Kacheln pro Sekunde (TABs `ProyectileSpeed`, Ranger 12) und trifft erst beim Ankommen – immer das Ziel, auf das er geschossen wurde, wenn es dann noch lebt. Gezielt wird wie in TAB dorthin, wo der Zombie sein wird. Weglassen = Kugel, trifft sofort |
| `speed` | Laufgeschwindigkeit |
| `build_seconds` | Ausbildungszeit in der Kaserne |
| `requires` | Muss erst stehen, damit sie ausgebildet werden kann – Soldat und Scharfschütze: `"MunitionsFactory"`. Weglassen = nichts. Dazu kann eine Forschung nötig sein (`research.json`) |
| `trained_in` | Wo sie ausgebildet wird: `"Barracks"` (weglassen) oder `"Inn"` (die Söldner) |
| `mercenary` | `true` = Söldner: bei Schulden geht er zuerst, schon ab Stufe 2 (`colony.json`), und trägt eine goldene Marke. Weglassen = `false` |
| `noise` | **Lärm pro Schuss** wie in TAB (Ranger 1, Soldat 3, Scharfschütze 10). Er sammelt sich dort, wo geschossen wird, und halbiert sich jede Sekunde – ein langer Kampf pendelt sich ein (siehe `sound.json`) |
| `heals` | `true` = schießt nicht, sondern richtet den Strahl auf eigene Einheiten (Medic) … |
| `heal_per_second` | … und heilt so viele HP pro Sekunde … |
| `heal_after_seconds` | … aber nur eine Einheit, die so lange nicht getroffen wurde – kein Heilen mitten im Kampf (nicht wie SC2) |
| `knockback` | Nahkampf (Speerträger): jeder Treffer stößt den Zombie so viele Kacheln zurück – einen großen weniger (Walker-Größe ganz, Fat ≈ halb, Riese ≈ ein Viertel). Weglassen = 0 |
| `cost_oil`, `oil` | **Öl**: so viel kostet sie (TAB: Lucifer 15, Thanatos 20, Titan 40) und so viel verbraucht sie pro Abrechnung, solange sie lebt (TABs `OilGen`: 1, Titan 2) – wie ihr Essen. Weglassen = 0 |
| `fire_factor` | **Feuer-Empfindlichkeit** wie TABs `FireDamageFactor`: so viel vom Feuer kommt an (Soldat 0,5, Lucifer 0 – sein Anzug, alle anderen 1). Pflicht in jedem Block |
| `splash`, `edge_factor` | **Flächenschaden** (TABs `EffectRadius`, `MinFactorDamageInArea`): der Schuss trifft jeden in so vielen Kacheln um den Einschlag, am Rand nur `edge_factor` davon (Titan 0,8 / 1, Thanatos' Rakete 1,8 / 0,2). Weglassen = nur das Ziel |
| `min_range` | Nichts näher als so viele Kacheln (Thanatos' Rakete 3, TABs `MinTargetDistance`) – außer sein `melee` reicht hin. Weglassen = 0 |
| `cone_degrees`, `friendly_fire`, `burn_chance` | **Feuerkegel** (Lucifer): trifft jeden in einem Kegel so breit (20°) und so lang wie `range`; `friendly_fire` = auch eigene Einheiten darin (TABs `FriendlyDamage`); so ein Anteil der getroffenen Zombies fängt Feuer (TABs `BurnFactor` 0,7) |
| `melee` | **Zweiter Angriff aus der Nähe** (Thanatos schlägt mit dem Werfer): `damage`, `range`, `cone_degrees`, `edge_factor`, `windup_seconds`, `cooldown_seconds` – benutzt, wenn das Ziel so nah ist |
| `skins` | Nur die Optik: ihre **Aussehen** in `assets\` (`"units/ranger"` = `assets\units\ranger.json`, siehe `assets\README.md`). Jede Einheit trägt eins davon ihr Leben lang. Fehlt die Datei, wird es übersprungen. Weglassen = keins |
| `sprite` | Ihr Bild, wenn keins der `skins` da ist (`"test/figure"` = das graue Testbild). Weglassen = Kästchen |

**Veteranen** (`veterans`): jeder Zombie, der stirbt, gibt seine Erfahrung (`xp` in `zombies\`), geteilt unter
deinen Einheiten im Umkreis – ein Veteran nimmt einen halben Anteil. Söldner nehmen nichts (sie sind schon besser).

| Wert | Bedeutung |
|---|---|
| `xp` | So viel Erfahrung, und die Einheit ist **Veteran** – silberne Marke, desertiert bei Schulden dreimal seltener |
| `share_radius` | Einheiten in so vielen Kacheln um den toten Zombie teilen sich seine Erfahrung |
| `hp_percent`, `damage_percent` | So viel Prozent mehr HP (kommen sofort dazu) und Schaden hat ein Veteran |

## Upgrades

| Wert | Bedeutung |
|---|---|
| `upgrades` | Was **eine Stufe** einer Upgrade-Reihe (`data\upgrades.json`) diesem Ding gibt – flach wie in SC2 (Marine +1, Marauder +2): `{ "infantry_weapons": { "damage": 2 } }`. Möglich: `hp`, `shield`, `damage`, `bonus_damage` (zu jedem Zusatzschaden von `bonus`), `armor`, `shield_armor` (Punkte), `range`, `sight`, `speed`, `attack_speed` (Prozentpunkte). Weglassen = keine Reihe, keine Wirkung. Die Stufe steigt durch eine Forschung mit `upgrade`, durch die Karte (Start) oder die Trigger-Aktion Research |
