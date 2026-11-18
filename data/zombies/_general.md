# Allgemeine Werte: zombies

Gelten für alle in `data\zombies\` zugleich, nicht für ein einzelnes Ding. Schlüssel in der Karte: `zombies.<wert>`.

| Wert | Standard | Bedeutung |
|---|---|---|
| `hit_every_seconds` | `1.0` | Zeit zwischen zwei Treffern |
| `charge_steps` | `36` | So weit (in Schritten) finden stürmende Zombies den Weg zu einer Einheit – um Wasser und Berge herum, durch Mauern hindurch |
| `idle` | `{ "stand": 1300, "growl": 150, "roam": 150 }` | Was ein Zombie **ohne Ziel** als Nächstes tut, gewürfelt mit diesen Gewichten (TAB): `stand` 1300, `growl` 150, `roam` 150 – meistens steht er |
| `stand_seconds` | `2.0` | So lange steht bzw. knurrt er, wenn er nichts zu tun hat (TAB: so lange wie seine Animationen, `TimeAniNormal`, `TimeAniSpecial`). Weglassen = die Werte für alle (unten) |
| `growl_seconds` | `2.0` | So lange steht bzw. knurrt er, wenn er nichts zu tun hat (TAB: so lange wie seine Animationen, `TimeAniNormal`, `TimeAniSpecial`). Weglassen = die Werte für alle (unten) |
| `contagion_seconds` | `3.95` | **Ansteckung** (TAB): so oft schaut jeder Zombie, der nicht zu einer Welle gehört, auf seine Kachel (TAB 238 Takte, knapp 4 s – knapp neben dem Hinhören, damit nicht beides auf denselben Tick fällt). Stehen dort mindestens so viele Wellen-Zombies mit seinem `level` oder höher, läuft er mit zum Kommandozentrum – Schlafende auch |
| `contagion_count` | `3` | **Ansteckung** (TAB): so oft schaut jeder Zombie, der nicht zu einer Welle gehört, auf seine Kachel (TAB 238 Takte, knapp 4 s – knapp neben dem Hinhören, damit nicht beides auf denselben Tick fällt). Stehen dort mindestens so viele Wellen-Zombies mit seinem `level` oder höher, läuft er mit zum Kommandozentrum – Schlafende auch |
| `infected` | `["Walker", "Walker", "Walker", "Armored", "Runner", "Runner", "Runner", "Runner"]` | Was aus einem **infizierten Gebäude** kommt und als was deine Toten aufstehen: je einer zufällig aus dieser Liste – TABs acht (WeakA/B/C → Walker, DressedA → Riot, WorkerA/B und MediumA/B → Runner) |
| `cull_below` | `100` | **Sicherung** (TAB): Sind weniger als so viele Zombies übrig (wach und schlafend), stirbt alle so viele Sekunden jeder, der keinen Weg zum Kommandozentrum hat – damit die Runde nicht am letzten Zombie auf einer Insel hängt |
| `cull_every_seconds` | `120.0` | **Sicherung** (TAB): Sind weniger als so viele Zombies übrig (wach und schlafend), stirbt alle so viele Sekunden jeder, der keinen Weg zum Kommandozentrum hat – damit die Runde nicht am letzten Zombie auf einer Insel hängt |
| `rise_min_seconds` | `2.0` | Eine tote Einheit steht nach so vielen Sekunden als Zombie auf (zufällig dazwischen, Art aus `infected`) und läuft zum Kommandozentrum |
| `rise_max_seconds` | `6.0` | Eine tote Einheit steht nach so vielen Sekunden als Zombie auf (zufällig dazwischen, Art aus `infected`) und läuft zum Kommandozentrum |
| `push_seconds` | `0.5` | So lange rutscht ein weggeschleuderter Körper, bis er liegt (geschätzt – TABs Dämpfung steckt in seiner Physik-DLL) |
| `die_seconds` | `1.0` | So lange spielt ein getöteter Zombie seinen Tod, dann ist er weg (TABs `TimeAniDie`: 1 s) |
