# Spielwerte (`data\`)

Alle Zahlen des Spiels an einem Ort. Zahl ändern, speichern, im Spiel **Esc → Restart** (oder „New map“, oder neu
starten) – die neuen Werte gelten ab der nächsten Runde, ohne neu zu bauen. Ist eine Datei kaputt (Komma vergessen,
Wert fehlt), läuft das Spiel mit den alten Werten weiter und zeigt oben links eine **rote Zeile** mit Datei und Stelle.

- Die Dateien werden beim Bauen auch **in die Exe eingebaut**. `dist\LastBastion.exe` läuft also ohne den Ordner, mit
  den Werten vom letzten `build.ps1`.
- **Ausnahme `spawning.json`:** Der konstante Spawn und die Wellen gehören zur **Karte**. Eine neue Karte bekommt eine
  Kopie davon in ihre Kartendatei (eine Runde = eine Datei, siehe Haupt-README), danach zählt, was dort steht – im
  Editor änderbar. Eine Änderung hier gilt also erst für **neue** Spiele („New map“ oder „New game“), nicht für
  „Restart“ einer Karte, die es schon gibt.
- JSON kennt keine Kommentare – deshalb steht hier, was jeder Wert heißt.
- **Zeiten stehen in Sekunden** (`…_seconds`), das Spiel rechnet in Takten (20 pro Sekunde) und rundet.
- **1 Tag = 3 Minuten, 1 Stunde = 7,5 Sekunden** (steht noch im Code, `DAY_TICKS` in `src\sim\mod.rs`).
- Entfernungen in **Kacheln**, Geschwindigkeiten in **Kacheln pro Sekunde**.
- Im Koop müssen später beide PCs dieselben Werte haben, sonst laufen die Spiele auseinander.

## `units.json` – Einheiten

Ein Block pro Typ: `soldier`, `ranger`, `sniper`, `medic`. Die Werte sind (fast) die aus *They Are Billions*.

Ein Schuss geht wie in StarCraft: **drehen** (bis die Einheit zum Ziel schaut) → **nachladen** abwarten (zählt ab dem
letzten Schuss) → **zielen** → Schuss. Ein Schuss dauert also `aim_seconds + cooldown_seconds` – so viel kleiner als
TABs Zeit zwischen zwei Schüssen ist `cooldown_seconds` gewählt.

| Wert | Bedeutung |
|---|---|
| `name`, `letter` | Name im Spiel, 1–2 Buchstaben fürs Symbol |
| `cost` | Preis in Gold. Bezahlt, sobald sie in die Bauschlange kommt; Abbrechen gibt alles zurück (wie StarCraft) |
| `hp` | Lebenspunkte |
| `armor` | Wird von jedem Zombie-Treffer abgezogen (mindestens 1 Schaden kommt immer durch) |
| `damage` | Schaden pro Schuss |
| `range` | Reichweite |
| `sight` | Sichtweite (mindestens so weit wie die Reichweite) |
| `turn_around_seconds` | So lange braucht sie für eine halbe Drehung (180°). Erst wenn sie zum Ziel schaut, zielt sie |
| `aim_seconds` | Ausholen vor jedem Schuss (Bogen spannen, Gewehr anlegen). Neues Ziel: das Zielen fängt von vorne an |
| `cooldown_seconds` | Nachladen nach dem Schuss, bevor sie wieder zielen kann (Medic: 0 = Strahl ohne Pause) |
| `speed` | Laufgeschwindigkeit |
| `build_seconds` | Ausbildungszeit in der Kaserne |
| `noise` | **Lärm** wie in TAB: Jeder Zombie so viele Kacheln um den Schützen hört den Schuss und stürmt los (`noise_seconds` in `zombies.json`). Ranger 1, Soldat 3, Scharfschütze 10. Wald und Mauern halten den Lärm nicht auf |
| `heals` | `true` = schießt nicht, sondern richtet den Strahl auf eigene Einheiten (Medic) |

## `buildings.json` – Gebäude

Ein Block pro Typ: `command_center`, `barracks` (die Kaserne – nur sie bildet Einheiten aus), `tent`, `wall`, `gate`, `tower`.

| Wert | Bedeutung |
|---|---|
| `name`, `letter` | Name im Spiel, Buchstaben fürs Symbol |
| `size` | `[Breite, Tiefe]` in Kacheln (das Tor lässt sich drehen) |
| `hp` | Lebenspunkte |
| `shield_percent` | Schild in **Prozent der HP** (Zelt: 133 % von 150 = 199). Zombies schlagen erst darauf. Bricht er, ist das Gebäude **infiziert** und seine Kolonisten kommen als Zombies heraus. Ohne Kolonisten ist er nur zusätzliches Leben. 0 = kein Schild. Im Spiel ein Balken mit den HP: der Schild graublau dahinter |
| `colonists` | Kolonisten darin: zahlen Gold und werden bei der Infektion zu Zombies (2–4 Runner, Rest Walker) |
| `cost` | Preis in Gold, bezahlt beim Platzieren (im Editor kostenlos) |
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
| `repair_seconds` | So viele Sekunden dauert eine volle Reparatur – eine kleinere entsprechend kürzer |

## `zombies.json` – Zombies

`walker`, `runner` und `fat` (TABs „Chubby“), je mit:

| Wert | Bedeutung |
|---|---|
| `speed` | Laufgeschwindigkeit (jeder Zombie bekommt zufällig ±20 %) |
| `hp` | Lebenspunkte |
| `armor` | Wird von jedem Schuss abgezogen (mindestens 1 Schaden kommt immer durch) |
| `damage` | Schaden pro Treffer |
| `sight` | So weit sieht er Einheiten (nicht durch Wald und Bäume). TAB: Walker 5, Runner 6, Fat 5 |
| `radius` | Wie groß er ist (Kacheln): so weit halten andere Zombies und Einheiten Abstand, und so viel weiter reicht er |

Dazu für alle:

| Wert | Bedeutung |
|---|---|
| `hit_every_seconds` | Zeit zwischen zwei Treffern |
| `hearing` | So nah bemerken sie eine Einheit auch ohne sie zu sehen |
| `notice_buildings` | Ein Gebäude so nah wird angegriffen |
| `alert_seconds` | So lange stürmt ein angeschossener Zombie los, auch ohne den Schützen zu sehen |
| `noise_seconds` | So lange stürmt ein Zombie los, der einen Schuss **gehört** hat (`noise` der Einheit, `units.json`) |
| `charge_steps` | So weit (in Schritten) finden stürmende Zombies den Weg zu einer Einheit |
| `seek_helpers` | Der angeschossene Zombie und so viele der nächsten um ihn herum suchen die **Lücke** in der Mauer … |
| `seek_radius` | … wenn sie höchstens so weit von ihm stehen … |
| `seek_seconds` | … so lange … |
| `seek_steps` | … und höchstens so viele Schritte zu Fuß um Mauern herum |
| `wander_radius` | Wandernde Zombies entfernen sich höchstens so weit von ihrem Platz … |
| `wander_pace` | … mit diesem Anteil ihrer Geschwindigkeit |
| `rise_min_seconds`, `rise_max_seconds` | Eine tote Einheit steht nach so vielen Sekunden als Walker auf (zufällig dazwischen) |
| `roaming` | Die Zombies, die zu Beginn über die Karte streifen (wie viele: Karteneinstellung „Roaming zombies“). Entfernungen als Anteil des Wegs von der Basis zum Rand (0 = Mitte, 1 = Rand): |
| `roaming.keep_out` | So nah am Kommandozentrum (Kacheln) liegt keiner |
| `roaming.edge_weight` | Wie viel mehr weiter draußen liegen: 0 = überall gleich viele, 1 = mit der Entfernung gleichmäßig mehr, 2 = fast alle am Rand |
| `roaming.runners_from`, `roaming.runner_share` | Ab so weit draußen streifen auch Runner herum – so ein Anteil von ihnen |
| `roaming.fat_from`, `roaming.fat_share` | Ab so weit draußen (am Rand) auch Fat – so ein Anteil |

## `colony.json` – Kolonie

| Wert | Bedeutung |
|---|---|
| `start_gold` | Gold zu Beginn |
| `gold_per_colonist_per_hour` | So viel zahlt jeder Kolonist pro Spielstunde (Zelt mit 6 Kolonisten × 1 = 6) |
| `start_soldiers`, `start_snipers`, `start_rangers` | Einheiten zu Beginn |

## `spawning.json` – konstanter Spawn und Wellen einer neuen Runde

Beides bekommt eine **neue Karte** als Kopie in ihre Kartendatei – danach ändert man es im **Editor** (Tab „Waves“)
oder direkt in der Kartendatei. Hier steht also nur, womit neue Karten anfangen.

### Konstanter Spawn (`stages`, `extra_per_day`)

Nachschub aus den Spawnern, die auf „Constant“ stehen (neue Karten: alle). Getötete Zombies kommen **nie** wieder –
was du freiräumst, bleibt frei, bis neue nachkommen.

`stages` ist eine Liste von Abschnitten, **nach Zeit sortiert**. Jeder gilt ab seinem Zeitpunkt bis zum nächsten:

| Wert | Bedeutung |
|---|---|
| `day`, `hour` | Ab wann (Tag ab 1, Stunde 0–23). `"day": 2, "hour": 0` = Ende von Tag 1 |
| `breed` | `"Walker"`, `"Runner"` oder `"Fat"` |
| `every_seconds` | Alle so viele Sekunden kommt eine Gruppe … |
| `group` | … aus `[min, max]` Zombies (zufällig dazwischen), alle aus einem zufälligen Spawner |

- Hat der **nächste** Abschnitt **dieselbe Art**, gehen die Werte **gleitend** dorthin über (so werden die Walker
  von Tag 2 bis Tag 3 nach und nach mehr).
- Wechselt die Art, bleiben die Werte bis zum Wechsel gleich.
- Nach dem **letzten** Abschnitt wird jede Gruppe pro Tag um `extra_per_day` Zombies größer.
- Was gerade kommt, zeigt die Entwickler-Anzeige (**F3**): „Spawn: 4-5 Runners every 60.0 s“.

### Wellen einer neuen Runde (`waves`)

Aus der Anzahl der Tage (Einstellung „Days“) wird die **Zeitleiste** geplant (DESIGN.md 8.2). Die Zahlen gelten **pro
Seite**, aus der die Welle kommt; die Einstellung „Waves“ (25–300 %) nimmt alle mal so viel.

| Wert | Bedeutung |
|---|---|
| `first_day` | An diesem Tag kommt die erste Welle … |
| `every_days` | … dann alle so viele Tage eine, bis vor dem letzten Tag … |
| `hour` | … immer zu dieser Stunde |
| `first`, `last` | Die erste Welle hat so viele Runner, die letzte vor der Schlusswelle so viele – dazwischen gleichmäßig mehr. Die erste Hälfte der Wellen kommt von 1 Seite, die zweite von 2 |
| `final_hour` | Die **Schlusswelle** kommt am letzten Tag zu dieser Stunde, von **allen 4 Seiten** … |
| `final_runners`, `final_fat` | … mit so vielen Runnern und Fat pro Seite |

Mit 10 Tagen (die Mini-Runde): Welle am Tag 4 (200 Runner, 1 Seite) und Tag 7 (600 × 2 Seiten), Schlusswelle am Tag
10 um 6:00 (1.000 Runner + 40 Fat × 4 Seiten = 4.160 Zombies). Mit 40 Tagen: 12 Wellen, dann die Schlusswelle.
**Gewonnen** ist die Runde, wenn alle Zombies der Schlusswelle tot sind; **verloren**, wenn das letzte
Kommandozentrum fällt.
