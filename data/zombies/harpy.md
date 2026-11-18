# Harpy

Zombie - Datei `harpy.json`, Schlüssel in der Karte (Modul Data): `zombies.harpy.<wert>`. Die Zeiten stehen in Sekunden, Entfernungen in Kacheln.

| Wert | Standard | Bedeutung |
|---|---|---|
| `name` | `"Harpy"` | Name im Spiel. In Karten- und Datendateien heißt die Art weiter wie ihr Block (`"Armored"`) – ein neuer Name ist also nur diese Zeile |
| `xp` | `3` | Erfahrung, wenn er stirbt – geteilt unter deinen Einheiten in der Nähe (`veterans` in `units.json`). Weglassen = 1 |
| `awareness` | `8.0` | **Wachsamkeit** wie TABs `ActivityAwarenessFactor`: Wie sicher er bei Lärm kommt, wächst damit (Formel bei `sound.json`). Walker 2, Runner, Fat, Riot, Infested 3, Spitter 4, Harpyie 8 |
| `speed` | `5.0` | **Renn**-Geschwindigkeit: wenn er angreift, mit einer Welle läuft oder Lärm folgt (TABs `RunSpeed`; jeder Zombie bekommt zufällig ±20 %) |
| `hp` | `139` | Lebenspunkte – TABs HP ÷ (1 − TABs Rüstungsanteil), so braucht jede Waffe so viele Treffer wie in TAB (Soldat 120 → 198, Riese 10.000 → 14.235) |
| `armor` | `0` | Rüstung in **Punkten** wie StarCraft 2: so viele Punkte gehen von **jedem** Schuss ab, mindestens 1 kommt immer durch (überall 0: TABs Rüstungsanteil steckt in `hp` – nur Upgrades geben Punkte). Upgrades der Zombies (`zombie_armor`) geben mehr |
| `attributes` | `["light", "biological"]` | Was es ist, wie SC2: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure`. Gegen diese Eigenschaften wirkt der `bonus` einer Waffe, und das Panel zeigt sie im Tooltip der Rüstung. Weglassen = keine |
| `bonus` | `{}` | **Zusatzschaden** je Eigenschaft des Ziels, auf `damage` drauf – wie SC2s „+10 gegen Gepanzert“: `{ "armored": 25 }`. Hat das Ziel mehrere der genannten Eigenschaften, zählen alle. Eine Upgrade-Stufe mit `bonus_damage` erhöht jeden dieser Zusätze. Weglassen = keiner |
| `regen` | `0.08` | **Selbstheilung** wie TABs `LifeRegenFactor` (Takt: `regen_every_seconds` / `regen_after_seconds` in `units.json`): Walker, Runner, Riot, Spitter, Harpyie 0,08 (Walker: 3 HP/s), Fat 0,04, Riese 0,005 (50 HP/s); Infested 0,04 ist unserer. Pflicht in jedem Block |
| `fire_factor` | `1.16` | **Feuer** wie TAB: so viel vom Feuer kommt an (`FireDamageFactor`: TAB Spitter 1,5, sonst 1 – hier mal den HP-Aufschlag, Fat 1,18, Riese 1,42) und so lange brennt er, wenn Lucifer ihn anzündet (`BurningTime` 3 s, der Riese fängt kein Feuer: 0) – brennend alle ½ s 5 × `fire_factor` Schaden, ohne Rüstung. Pflicht in jedem Block |
| `burn_seconds` | `3.0` | **Feuer** wie TAB: so viel vom Feuer kommt an (`FireDamageFactor`: TAB Spitter 1,5, sonst 1 – hier mal den HP-Aufschlag, Fat 1,18, Riese 1,42) und so lange brennt er, wenn Lucifer ihn anzündet (`BurningTime` 3 s, der Riese fängt kein Feuer: 0) – brennend alle ½ s 5 × `fire_factor` Schaden, ohne Rüstung. Pflicht in jedem Block |
| `damage` | `30` | Schaden pro Treffer – beim Spitter an jedem im Spritzer |
| `sight` | `9.0` | So weit sieht er Einheiten und **Gebäude mit Leuten** darin (Kommandozentrum, Häuser, Gebäude mit Arbeitern, Türme mit Einheiten – nicht durch Wald). TAB: Walker 5, Runner 6, Fat 5 |
| `watch_seconds` | `0.67` | So oft **schaut er sich um** (TABs `EntityWatchInterval`: Walker und Riese 1,5, Harpyie 0,67, die anderen 1): nimmt das Nächste, das er sieht, als Ziel. Doppelt so lange, solange er schon ein Ziel hat; 3× so lange außerhalb deiner Sicht, 5× wo du nie warst |
| `level` | `4` | **Rang** (TABs `Level`): Stehen auf seiner Kachel mindestens `contagion_count` Wellen-Zombies mit diesem Rang oder höher, schließt er sich der Welle an – gehört die zur Schlusswelle, er auch. Walker 1, Runner 2, Riot 3, Fat, Harpyie und Infested 4, Spitter 5, Riese 6 |
| `stroll` | `[1.0, 2.0]` | `[mindestens, höchstens]` wie TABs `MinRoamingDistance`, `MaxRoamingDistance`: so weit geht er beim **Wandern** pro Ausflug – auf jeder Achse für sich zwischen den beiden **plus 1** (wie TAB), in beide Richtungen (ein Quadrat), von da, wo er gerade steht. TAB: fast alle 0–1 (also unter 2 Kacheln), Harpyie 1–2, Riese 2–3 – eine leergeräumte Gegend füllt sich nur langsam wieder |
| `radius` | `0.3` | Wie groß er ist (Kacheln): so weit halten andere Zombies und Einheiten Abstand, und so viel weiter reicht er. So groß ist auch der Ring unter ihm (Maus, angeklickt) |
| `mass` | `1.0` | Wie schwer er ist (TABs `Mass`: Walker 1, Runner 1,25, Riot 3, Fat 6, Riese 1000): im Gedränge gibt der Leichtere nach – ein Riese läuft durch Walker, sie bewegen ihn kaum |
| `attack_seconds` | `0.67` | Zeit zwischen zwei Angriffen – so lange **steht** er dabei (TAB: `StopMovement`). Weglassen = `hit_every_seconds` |
| `windup_seconds` | `0.27` | **Ausholen** wie TAB (`TimeAction × TimeFactorAction`): der Schlag kommt so lange nach dem Start des Angriffs. Ist die Einheit dann aus seiner Reichweite, trifft er nicht (Schüsse fliegen trotzdem). Walker, Runner, Fat 0,5, Riot 0,4, Spitter 0,42, Harpyie 0,15, Riese 0,16; Infested 0,2 ist unserer. Weglassen = 0: sofort |
| `hit_noise` | `10` | So viel Lärm macht jeder seiner Treffer, wo er steht (holt Zombies, weckt Schlafende; zählt nicht zur Unruhe). Trifft er öfter, als der Lärm sich halbiert (`halve_seconds` in `sound.json`), wird es lauter (Harpyie). Weglassen = `zombie_hit` aus `sound.json` |
| `jump` | `3.0` | Springt über Mauern und Tore bis so viele Kacheln dick, statt sie einzuschlagen – wenn sein Ziel dahinter liegt (Harpyie). Weglassen = 0, springt nie |
| `walk_speed` | `0.3` | **Geh**-Geschwindigkeit beim Wandern (TABs `WalkSpeed`) |
| `stand_seconds` | `2.0` | So lange steht bzw. knurrt er, wenn er nichts zu tun hat (TAB: so lange wie seine Animationen, `TimeAniNormal`, `TimeAniSpecial`). Weglassen = die Werte für alle (unten) |
| `growl_seconds` | `6.0` | So lange steht bzw. knurrt er, wenn er nichts zu tun hat (TAB: so lange wie seine Animationen, `TimeAniNormal`, `TimeAniSpecial`). Weglassen = die Werte für alle (unten) |
| `skins` | `["zombies/harpy"]` | Nur die Optik: seine Aussehen, wie bei den Einheiten – TABs Walker hat drei, der Runner vier. Jeder Zombie trägt eins davon, auch im Schlaf |
| `sprite` | `"test/figure"` | Sein Bild, wenn keins der `skins` da ist. Weglassen = Kästchen |
