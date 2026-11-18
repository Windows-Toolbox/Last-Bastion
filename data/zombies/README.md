# Zombies (`data\zombies\`)

Jedes Ding hat eine eigene Datei: `<name>.json` (die Werte) und `<name>.md` (die Werte mit Bedeutung, zum Lesen). In `_general.json` stehen die Werte, die für alle zugleich gelten. Das Spiel setzt alle Dateien des Ordners beim Laden zu einem Block zusammen (`units.soldier.hp` ist also dasselbe wie früher in `units\` – so heißen die Werte auch im Editor-Modul Data). **Ein neues Ding** braucht seine `.json` (mit `name`) hier, eine Zeile in `src\data\files.rs` und – wie bisher – seinen Eintrag im Code (`kinds.rs`); ein Test merkt, wenn eins davon fehlt.


`walker`, `runner`, `fat` (TABs „Chubby“), `armored` (TABs gepanzerter Runner, im Spiel „Riot“), `spitter` (TABs
Venom), `infested` (StarCraft 2s Infested Marine), `harpy` (TABs Harpyie: hört weit, schlägt laut zu) und `giant` (TABs Riese,
etwas kleiner – wie viele: `giants` der Karte), je mit:

| Wert | Bedeutung |
|---|---|
| `name` | Name im Spiel. In Karten- und Datendateien heißt die Art weiter wie ihr Block (`"Armored"`) – ein neuer Name ist also nur diese Zeile |
| `awareness` | **Wachsamkeit** wie TABs `ActivityAwarenessFactor`: Wie sicher er bei Lärm kommt, wächst damit (Formel bei `sound.json`). Walker 2, Runner, Fat, Riot, Infested 3, Spitter 4, Harpyie 8 |
| `speed` | **Renn**-Geschwindigkeit: wenn er angreift, mit einer Welle läuft oder Lärm folgt (TABs `RunSpeed`; jeder Zombie bekommt zufällig ±20 %) |
| `walk_speed` | **Geh**-Geschwindigkeit beim Wandern (TABs `WalkSpeed`) |
| `hp` | Lebenspunkte – TABs HP ÷ (1 − TABs Rüstungsanteil), so braucht jede Waffe so viele Treffer wie in TAB (Soldat 120 → 198, Riese 10.000 → 14.235) |
| `armor` | Rüstung in **Punkten** wie StarCraft 2: so viele Punkte gehen von **jedem** Schuss ab, mindestens 1 kommt immer durch (überall 0: TABs Rüstungsanteil steckt in `hp` – nur Upgrades geben Punkte). Upgrades der Zombies (`zombie_armor`) geben mehr |
| `attributes` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = keine |
| `bonus` | **Zusatzschaden** je Eigenschaft des Ziels, auf `damage` drauf – wie SC2s „+10 gegen Gepanzert“: `{ "armored": 25 }`. Hat das Ziel mehrere der genannten Eigenschaften, zählen alle. Eine Upgrade-Stufe mit `bonus_damage` erhöht jeden dieser Zusätze. Weglassen = keiner |
| `regen` | **Selbstheilung** wie TABs `LifeRegenFactor` (Takt: `regen_every_seconds` / `regen_after_seconds` in `units\`): Walker, Runner, Riot, Spitter, Harpyie 0,08 (Walker: 3 HP/s), Fat 0,04, Riese 0,005 (50 HP/s); Infested 0,04 ist unserer. Pflicht in jedem Block |
| `fire_factor`, `burn_seconds` | **Feuer** wie TAB: so viel vom Feuer kommt an (`FireDamageFactor`: TAB Spitter 1,5, sonst 1 – hier mal den HP-Aufschlag, Fat 1,18, Riese 1,42) und so lange brennt er, wenn Lucifer ihn anzündet (`BurningTime` 3 s, der Riese fängt kein Feuer: 0) – brennend alle ½ s 5 × `fire_factor` Schaden, ohne Rüstung. Pflicht in jedem Block |
| `damage` | Schaden pro Treffer – beim Spitter an jedem im Spritzer |
| `sweep_radius`, `sweep_degrees`, `sweep_hits_zombies` | **Flächenschlag** (TABs Riese: Bogen 120°, Radius 3,5): jeder Treffer trifft sein Ziel und alle vor ihm in diesem Bogen – Einheiten am Boden, Gebäude, mit `sweep_hits_zombies` auch andere Zombies (TABs `FriendlyDamage`). Weglassen = nur das Ziel |
| `push` | **Rückstoß** (TABs `FactorPush`, Riese 150): wen ein Schlag von ihm tötet, der fliegt weg – `push / mass / 12` Kacheln pro Sekunde (Einheiten: Masse 1), durch alles hindurch wie in TAB, bremst über `push_seconds`. Lebende schiebt er nicht. Weglassen = 0 |
| `sight` | So weit sieht er Einheiten und **Gebäude mit Leuten** darin (Kommandozentrum, Häuser, Gebäude mit Arbeitern, Türme mit Einheiten – nicht durch Wald). TAB: Walker 5, Runner 6, Fat 5 |
| `watch_seconds` | So oft **schaut er sich um** (TABs `EntityWatchInterval`: Walker und Riese 1,5, Harpyie 0,67, die anderen 1): nimmt das Nächste, das er sieht, als Ziel. Doppelt so lange, solange er schon ein Ziel hat; 3× so lange außerhalb deiner Sicht, 5× wo du nie warst |
| `level` | **Rang** (TABs `Level`): Stehen auf seiner Kachel mindestens `contagion_count` Wellen-Zombies mit diesem Rang oder höher, schließt er sich der Welle an – gehört die zur Schlusswelle, er auch. Walker 1, Runner 2, Riot 3, Fat, Harpyie und Infested 4, Spitter 5, Riese 6 |
| `stroll` | `[mindestens, höchstens]` wie TABs `MinRoamingDistance`, `MaxRoamingDistance`: so weit geht er beim **Wandern** pro Ausflug – auf jeder Achse für sich zwischen den beiden **plus 1** (wie TAB), in beide Richtungen (ein Quadrat), von da, wo er gerade steht. TAB: fast alle 0–1 (also unter 2 Kacheln), Harpyie 1–2, Riese 2–3 – eine leergeräumte Gegend füllt sich nur langsam wieder |
| `stand_seconds`, `growl_seconds` | So lange steht bzw. knurrt er, wenn er nichts zu tun hat (TAB: so lange wie seine Animationen, `TimeAniNormal`, `TimeAniSpecial`). Weglassen = die Werte für alle (unten) |
| `roam_ring` | Beim Wandern nie in diesen Kreis ums Kommandozentrum (Kacheln) – außer er steht schon drin. TABs Riese: 54. Weglassen = 0, kein Ring |
| `roar_every_seconds` | So oft bleibt er stehen und **brüllt** `growl_seconds` lang, egal was er gerade tut (TABs Riese: 12). Wer brüllt, denkt wie TABs Riese: läuft nie dahin, wo er dich aus den Augen verloren hat, und lässt sich nie von einer Welle anstecken. Weglassen = brüllt nie |
| `radius` | Wie groß er ist (Kacheln): so weit halten andere Zombies und Einheiten Abstand, und so viel weiter reicht er. So groß ist auch der Ring unter ihm (Maus, angeklickt) |
| `mass` | Wie schwer er ist (TABs `Mass`: Walker 1, Runner 1,25, Riot 3, Fat 6, Riese 1000): im Gedränge gibt der Leichtere nach – ein Riese läuft durch Walker, sie bewegen ihn kaum |
| `venom` | `true` = seine Treffer sind **Gift** (Spitter, wie TABs Venom): keine Rüstung hilft, eine Einheit bekommt ihren `venom_factor` davon. Weglassen = `false` |
| `range` | **Fernkampf**: so weit schießt er (Kacheln) – über Mauern, nicht durch Wald. Weglassen = 0: er muss dich berühren. Infested 4,5 = knapp unter dem Soldaten (5) |
| `splash` | Sein Schuss trifft jeden so nah am Einschlag (Spitter): Einheiten und Gebäude, keine Zombies. Weglassen = nur das Ziel |
| `attack_seconds` | Zeit zwischen zwei Angriffen – so lange **steht** er dabei (TAB: `StopMovement`). Weglassen = `hit_every_seconds` |
| `windup_seconds` | **Ausholen** wie TAB (`TimeAction × TimeFactorAction`): der Schlag kommt so lange nach dem Start des Angriffs. Ist die Einheit dann aus seiner Reichweite, trifft er nicht (Schüsse fliegen trotzdem). Walker, Runner, Fat 0,5, Riot 0,4, Spitter 0,42, Harpyie 0,15, Riese 0,16; Infested 0,2 ist unserer. Weglassen = 0: sofort |
| `projectile_speed` | Sein Schuss fliegt so viele Kacheln pro Sekunde und landet, wo das Ziel stand – wer rechtzeitig weggeht, wird nicht getroffen (Spitter). Weglassen = trifft sofort |
| `xp` | Erfahrung, wenn er stirbt – geteilt unter deinen Einheiten in der Nähe (`veterans` in `units\`). Weglassen = 1 |
| `jump` | Springt über Mauern und Tore bis so viele Kacheln dick, statt sie einzuschlagen – wenn sein Ziel dahinter liegt (Harpyie). Weglassen = 0, springt nie |
| `hit_noise` | So viel Lärm macht jeder seiner Treffer, wo er steht (holt Zombies, weckt Schlafende; zählt nicht zur Unruhe). Trifft er öfter, als der Lärm sich halbiert (`halve_seconds` in `sound.json`), wird es lauter (Harpyie). Weglassen = `zombie_hit` aus `sound.json` |
| `skins` | Nur die Optik: seine Aussehen, wie bei den Einheiten – TABs Walker hat drei, der Runner vier. Jeder Zombie trägt eins davon, auch im Schlaf |
| `sprite` | Sein Bild, wenn keins der `skins` da ist. Weglassen = Kästchen |

Dazu für alle:

| Wert | Bedeutung |
|---|---|
| `hit_every_seconds` | Zeit zwischen zwei Treffern |
| `charge_steps` | So weit (in Schritten) finden stürmende Zombies den Weg zu einer Einheit – um Wasser und Berge herum, durch Mauern hindurch |
| `idle` | Was ein Zombie **ohne Ziel** als Nächstes tut, gewürfelt mit diesen Gewichten (TAB): `stand` 1300, `growl` 150, `roam` 150 – meistens steht er |
| `stand_seconds`, `growl_seconds` | So lange steht bzw. knurrt er – für eine Art ohne eigene Werte |
| `contagion_seconds`, `contagion_count` | **Ansteckung** (TAB): so oft schaut jeder Zombie, der nicht zu einer Welle gehört, auf seine Kachel (TAB 238 Takte, knapp 4 s – knapp neben dem Hinhören, damit nicht beides auf denselben Tick fällt). Stehen dort mindestens so viele Wellen-Zombies mit seinem `level` oder höher, läuft er mit zum Kommandozentrum – Schlafende auch |
| `infected` | Was aus einem **infizierten Gebäude** kommt und als was deine Toten aufstehen: je einer zufällig aus dieser Liste – TABs acht (WeakA/B/C → Walker, DressedA → Riot, WorkerA/B und MediumA/B → Runner) |
| `cull_below`, `cull_every_seconds` | **Sicherung** (TAB): Sind weniger als so viele Zombies übrig (wach und schlafend), stirbt alle so viele Sekunden jeder, der keinen Weg zum Kommandozentrum hat – damit die Runde nicht am letzten Zombie auf einer Insel hängt |
| `rise_min_seconds`, `rise_max_seconds` | Eine tote Einheit steht nach so vielen Sekunden als Zombie auf (zufällig dazwischen, Art aus `infected`) und läuft zum Kommandozentrum |
| `push_seconds` | So lange rutscht ein weggeschleuderter Körper, bis er liegt (geschätzt – TABs Dämpfung steckt in seiner Physik-DLL) |
| `die_seconds` | So lange spielt ein getöteter Zombie seinen Tod, dann ist er weg (TABs `TimeAniDie`: 1 s) |
