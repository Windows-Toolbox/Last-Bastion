# Spielwerte (`data\`)

Alle Zahlen des Spiels an einem Ort. Zahl ändern, speichern, im Spiel **Esc → Restart** (oder „New map“, oder neu
starten) – die neuen Werte gelten ab der nächsten Runde, ohne neu zu bauen. Ist eine Datei kaputt (Komma vergessen,
Wert fehlt), läuft das Spiel mit den alten Werten weiter und zeigt oben links eine **rote Zeile** mit Datei und Stelle.

- Die Dateien werden beim Bauen auch **in die Exe eingebaut**. `dist\LastBastion.exe` läuft also ohne den Ordner, mit
  den Werten vom letzten `build.ps1`.
- **Vor der Runde** wählt man wie in TAB **Karte, Schwierigkeit und Dauer** (`game_maps.json`,
  `game_difficulties.json`, `game_durations.json`). Was sie sagen, landet beim Start in der Kartendatei (Zombies,
  Wellen, Startgold …) – „Restart“ ändert daran nichts mehr. Nur `keep_out` der Schwierigkeit wird über ihren Namen
  in der Kartendatei nachgeschaut.
- **Je Karte änderbar:** Im Editor (Modul **Data**) lässt sich jeder Zahlen- und Ja/Nein-Wert aus `units`, `buildings`,
  `zombies`, `sound`, `roaming`, `doom`, `loot`, `colony`, `research`, `audio` und `trains` für **eine Karte** überschreiben –
  sie speichert nur die Abweichungen (`data` in der Kartendatei, z. B. `"units.soldier.hp": 150`), alles andere gilt aus
  diesen Dateien. Nicht dabei: `spawning` (gehört der Karte), `game_difficulties`, `game_durations`, `game_maps` (Auswahl vor der Runde).
  Namen in diesen Dateien dürfen keinen Punkt haben (ein Test prüft das).
- **Ausnahme `spawning.json`:** Die Events (Wellen, Streuner, Schlusswelle, Sieg) gehören zur **Karte**. Eine neue
  Karte bekommt eine Kopie davon in ihre Kartendatei (eine Runde = eine Datei, siehe Haupt-README), danach zählt, was dort steht – im
  Editor änderbar. Eine Änderung hier gilt also erst für **neue** Spiele („New map“ oder „New game“), nicht für
  „Restart“ einer Karte, die es schon gibt.
- JSON kennt keine Kommentare – deshalb steht hier, was jeder Wert heißt.
- **Zeiten stehen in Sekunden** (`…_seconds`), das Spiel rechnet in Takten (20 pro Sekunde) und rundet.
- **1 Tag = 60 Sekunden, 1 Stunde = 2,5 Sekunden** wie in TAB (steht im Code, `DAY_TICKS` in `src\sim\mod.rs`).
- **„Pro Abrechnung“:** Die Wirtschaft rechnet alle `economy_every_seconds` Sekunden ab (`colony.json`, 28 wie TABs
  `CResourceGenerator`): Gold, Nahrung, Holz, Unterhalt – alle Werte dazu gelten pro Abrechnung.
- **Rüstung in Punkten** (SC2), Grundwert überall 0: TABs Anteil steckt in `hp` (TAB-HP ÷ (1 − Anteil)), Gift- und
  Feuerfaktor sind mit demselben Faktor mal genommen – so braucht jede Waffe so viele Treffer wie in TAB.
- Entfernungen in **Kacheln**, Geschwindigkeiten in **Kacheln pro Sekunde**.
- Im Koop müssen später beide PCs dieselben Werte haben, sonst laufen die Spiele auseinander.

## `units\`, `buildings\`, `zombies\` – Einheiten, Gebäude, Zombies

Je Typ ein **Ordner**, je Ding darin eine Datei (`soldier.json`) mit ihrer Beschreibung (`soldier.md`); `_general.json` hat die Werte für alle. Was jeder Wert heißt, steht in der `README.md` des Ordners und – zum Nachschlagen für genau dieses Ding – in dessen `.md`:

- [`units\`](units/README.md) – Einheiten: Kampf, Preis, Zeiten, Veteranen
- [`buildings\`](buildings/README.md) – Gebäude: Preis, Wirtschaft, Strom, Türme, Fallen, Wunder
- [`zombies\`](zombies/README.md) – Zombies: die acht Arten, ihr Verhalten, Ansteckung

## `sound.json` – Lärm

**Lärm** genau wie in TAB (aus dessen Code): Er sammelt sich dort, wo etwas passiert (je 4 × 4 Kacheln), und jede
Sekunde halbiert sich jede Zelle. Wie laut ein Schuss ist: `noise` in `units\`.

**Wer kommt:** Jeder Zombie hört alle `listen_seconds` (jeder zu einem anderen Tick) und nimmt von den lauten Zellen in
Hörweite die, die er am besten hört: **Lärm × `per_awareness` × `awareness` / Entfernung²** (Kacheln). So viel Prozent
Chance hat er zu kommen – ab 100 sicher; doppelt so weit weg ein Viertel so sicher. Er geht zur Mitte der Zelle und
bleibt dort, solange es laut ist. Ist es dort still, wenn er das nächste Mal hört, vergisst er es. Wer gerade ein Ziel
hat, dahin läuft, wo er es aus den Augen verloren hat, oder mit einer Welle kommt, hört nicht hin.

| Wert | Bedeutung |
|---|---|
| `hearing` | Ein Zombie hört so viel mal seine Sichtweite weit, in x und y (TAB: 4) |
| `per_awareness` | Der Faktor in der Formel oben (TAB: 20) |
| `halve_seconds` | So oft halbiert sich jede Zelle, ob still oder nicht (TAB: 1) |
| `listen_seconds` | So oft hört jeder Zombie hin, auch die Schlafenden (TAB: 4) |
| `places_listen_seconds` | So oft hören die Häuser des Untergangs und die versteckten Wächter der Beute hin (TAB: 1) |
| `zombie_hit`, `unit_dies`, `building_placed`, `building_destroyed` | So laut: jeder Treffer eines Zombies, eine sterbende Einheit, ein platziertes Gebäude, ein zerstörtes Gebäude |
| `building_infected`, `per_infected` | Ein infiziertes Gebäude – plus so viel für jeden, der herauskommt. Ein Gebäude mit Arbeitern, das fällt, ist infiziert: die Arbeiter kommen als Zombies heraus (TAB: eine überrannte Jägerhütte weit draußen lockt die Horde) |
| `work_every_seconds` | So oft macht jedes arbeitende Gebäude seinen Lärm (`noise` in `buildings\`) – alle im selben Moment, Nachbarn zählen zusammen |
| `unrest_per_zombie` | **Unruhe:** aller Lärm der Kolonie (Schüsse, Maschinen, Explosionen, Bauen) zählt zusammen und sinkt nie – je so viel davon bringt jede Seite eines stillen Events (TABs Streuner) für immer einen Zombie mehr |

## `roaming.json` – Streuner

Die Zombies, die zu Beginn über die Karte streifen (wie viele: `roaming_zombies` der Schwierigkeit). Entfernungen als
Anteil des Wegs von der Basis zum Rand (0 = Mitte, 1 = Rand). Wie weit sie pro Ausflug wandern: `stroll` der Art in
`zombies\`, wie nah ans Kommandozentrum sie zu Beginn liegen: `keep_out` der Schwierigkeit (beim Wandern dürfen sie
näher, wie in TAB – nur der Riese hält `roam_ring`). Weit weg von deinen Augen schlafen sie, bis etwas sie weckt
(DESIGN.md 8.5).

| Wert | Bedeutung |
|---|---|
| `edge_weight` | Wie viel mehr weiter draußen liegen: 0 = überall gleich viele, 1 = mit der Entfernung gleichmäßig mehr, 2 = fast alle am Rand (jetzt 0,5: fast überall, außen etwas mehr – wie TAB) |
| `pack` | `[wenigste, meiste]`: sie liegen in Grüppchen auf je einer Kachel, eine Gruppe pro Kachel – also nie mehr als `meiste` auf einer Kachel (wie TAB: bis 3). `[3, 3]` = überall genau 3 (dann ≈ doppelt so viele möglich, Regler bis 2.000 reicht dann nicht mehr ganz) |
| `ring_ramp` | Über so viele Kacheln hinter dem Ring ums Kommandozentrum (das kleinste `keep_out` der Schwierigkeit) dünnen sie nach innen aus: direkt am Ring kaum welche und nur einzeln, so weit draußen so viele wie überall und in vollen Grüppchen (`pack`). 0 = kein sanfter Rand (jetzt 6). Insgesamt bleiben es gleich viele – die fehlenden liegen weiter draußen |
| `mix` | Welche Arten außer Walkern: je `breed` (Name wie in den Dateien), `from` (ab so weit draußen) und `share` (so ein Anteil der Zombies dort). Der Rest sind Walker. Gilt auch für die Villages of Doom und die Wächter der Beute. Näher am Kommandozentrum als `keep_out` ihrer Art (Schwierigkeit) wird eine Art zum Walker |
| `giants_from` | Die Riesen (`giants` der Karte) liegen mindestens so weit draußen (Anteil des Wegs wie `from`) – schlafend, sie bummeln und wachen wie alle |

## `doom.json` – Villages of Doom

Die Häuser der **Villages of Doom** wie TABs (`CInfectionNest`): Ihre Zombies liegen nicht drin, sie **entstehen in
Schüben**, wenn etwas passiert. Ein Schub = aufgerundet (zufällig 0,5 … 1,5) × **Hausgröße** (klein 1, mittel 2, groß
5 – nach den Kacheln: 2 × 2, 3 × 3, größer) × `factor` des Hauses (`population` der Schwierigkeit × `nests` der
Landschaft) × der Faktor des Anlasses. Wie viele Dörfer und Häuser: auf TABs Karten der Generator, auf unseren die
Karte („Villages of doom“, „Houses near base“, „Houses at edge“) mal `villages_percent` / `village_houses_percent`.

| Wert | Bedeutung |
|---|---|
| `hp`, `armor` | Lebenspunkte eines Hauses je Größe (`small`, `medium`, `big` – TAB 400 / 1500 / 4000), Rüstung in Punkten gegen jeden Schuss (TAB: keine) |
| `max_units` | So viele lässt ein Haus insgesamt heraus, je Größe, mal `population` (TABs `InfectionNestMaxUnits`: 40 / 150 / 500) |
| `batch_every_seconds` | Höchstens ein Schub so oft pro Haus |
| `hearing`, `awareness`, `heard_batch` | Hört das Haus **Lärm** so nah (je Größe, TAB: 4 × seine Sichtweite – 20 / 24 / 32) – so sicher wie ein Zombie mit dieser `awareness` (Formel bei `sound.json`) –, geht ein Schub × `heard_batch` zur Mitte der gehörten Zelle |
| `guard_every_hours`, `guards_per_size`, `guard_batch` | So oft (jedes Haus zu seiner Stunde): stehen weniger als `guards_per_size` × Hausgröße Zombies ums Haus (es selbst zählt mit), bleibt ein Schub × `guard_batch` dort – die **Wachen** füllen sich auf |
| `raids_from_day`, `raid_every_days`, `raid_batch` | Ab diesem Tag alle so viele Tage (jedes Haus an seinem Tag): ein Schub × `raid_batch` **marschiert zum Kommandozentrum** wie eine Welle |
| `hit_batch`, `hit_reach`, `neighbour_batch` | **Getroffen:** ein Schub × `hit_batch` geht auf die nächste Einheit in so vielen Kacheln (keine da: bleibt am Haus); jedes Nachbarhaus (Fläche × 3 um die Mitte + 4 Kacheln) schickt einen Schub × `neighbour_batch` mit |
| `final_batch` | Ruft die Schlusswelle: ein Schub × so viel, sooft es darf, bis es leer ist – mit der Schlusswelle |
| `breeds` | Welche Arten aus einem Haus je Größe kommen und wie wahrscheinlich (`breed`, `weight` – TABs Gewichte: klein Walker, mittel Runner, groß Runner 400 / Riot 100 / Fat 15) |
| `loot_gold`, `loot_goods` | Ein abgeschossenes Haus lässt Beute fallen: so viel Gold – oder so viel Holz, Stein, Eisenerz oder Kohle. Was es noch nicht herausgelassen hat, kommt nie |
| `skins` | Nur die Optik: die Aussehen je Größe (`small`, `medium`, `big` – TABs je vier Häuser, aus `tools\tab_buildings.py`). Jedes Haus trägt eins davon, ein größeres wird größer gezeichnet; rötlich, während es einen Schub herauslässt. Weglassen oder keins da = Klötze |

## `loot.json` – Beute

Die **Beute**, die herumliegt – wie viel: Karteneinstellung „Loot“. Wie viele Wächter: `loot_guards` der
Schwierigkeit.

| Wert | Bedeutung |
|---|---|
| `wake_radius`, `hearing`, `awareness` | Kommt eine Einheit so nah – oder hören sie Lärm so nah, so sicher wie ein Zombie mit dieser `awareness` –, kommen die versteckten Wächter auf einmal aus dem Boden (und gehen auf die Einheit bzw. zum Lärm) |
| `gold`, `goods`, `gold_share` | Ein Haufen: so viel Gold oder Holz/Stein/Eisenerz/Kohle – Gold in diesem Anteil der Haufen |
| `food`, `energy` | TABs **Vorräte** (`PickableFood`, `PickableEnergy`, je 20): aufgehoben macht das Kommandozentrum für den Rest der Runde so viel mehr Nahrung je Abrechnung bzw. Strom. Sie liegen an den Beute-Stellen beim brennenden Lastwagen bzw. am Strommast (`food_sites_*`, `energy_sites_*` der Landschaft) |

## `colony.json` – Kolonie

Womit die Kolonie startet (Gold, Holz, Einheiten), sagt die Schwierigkeit (`game_difficulties.json`).

| Wert | Bedeutung |
|---|---|
| `economy_every_seconds` | So viele Sekunden von einer **Abrechnung** der Wirtschaft zur nächsten (TAB: 28) |
| `sight_into_forest` | So viele Waldkacheln tief sehen deine Einheiten und Gebäude in einen Wald (TAB: 3), dahinter nichts. Schießen geht weiter nur bis zum Waldrand |
| `storage_gold`, `storage_goods`, `storage_food` | **Lager:** so viel Gold, so viel von jeder Ware (Holz, Stein, Eisenerz, Kohle) und so viel Nahrung passt höchstens hinein – wie TABs Kommandozentrum (2000 Gold, 50 Waren). Was bei einer Abrechnung oder als Beute darüber hinaus käme, ist verloren. Nahrung lagert TAB nicht, bei uns sammelt sie sich zwischen den Abrechnungen – daher eine eigene Grenze. In der Leiste: der Balken füllt sich, voll wird er rot |
| `debt_penalty_percent` | **Schulden:** Unterhalt, den das Gold nicht deckt, wird Schulden – neues Gold zahlt sie zuerst ab. Verschuldet machen die Einheiten so viel Prozent weniger Schaden und schießen so viel langsamer |
| `deep_debt_payouts`, `deep_debt_penalty_percent`, `deep_debt_slow_percent` | Mehr Schulden als so viele Abrechnungen Unterhalt (gerade jetzt): so viel Prozent weniger Schaden, und sie laufen so viel langsamer |
| `desert_debt_payouts`, `desert_every_seconds` | Mehr als so viele Abrechnungen: alle so viele Sekunden **desertiert** eine Einheit – weg für immer, je mehr Unterhalt sie kostet, desto eher. Söldner schon ab `deep_debt_payouts`, und solange es welche gibt, nur sie |
| `credit_interest_percent` | Schulden im Dispo der Banken bringen keine Strafen (gezählt wird nur, was darüber liegt) – sie kosten so viel Prozent davon an Zinsen pro Abrechnung |

## `spawning.json` – TABs Survival-Plan

Die **Events** einer neuen Runde – TABs eigener Plan (aus dessen Code), für **100 Tage und Bevölkerung 1**. Eine neue
Karte bekommt sie als Kopie in ihre Kartendatei, danach ändert man sie im **Editor** (Modul „Events“) oder in der
Datei. Beim Anlegen: **Stunden × Tage / 100** (aufgerundet – nicht der Zufallsversatz, nicht die Vorwarnung),
**Anzahlen × `population`** der Schwierigkeit (gerundet, mindestens 1; Riesen nicht).

| Wert | Bedeutung |
|---|---|
| `stream_every_seconds`, `stream_speed` | Wie TABs Spawner: alle so viele Sekunden lässt jede Seite einen Schub heraus – je Art ein Anteil an `stream_speed` (die erste Art füllt auf), mindestens 1 jede. Ein großer Schwarm kommt so als lange Kolonne |
| `events` | Die Events, jedes wie in der Kartendatei (Felder unten), dazu `low_population` |

Ein **Event** (TABs `ZXLevelEvent`, die TAB-Namen in Klammern):

| Feld | Bedeutung |
|---|---|
| `name` | Name (im Editor, in der F3-Anzeige) |
| `hour`, `random_hours` | Kommt zu dieser Stunde ab Rundenbeginn (Tag 1, 0:00 = 0; `GameTime`) plus zufällig bis zu so vielen Stunden (`GameTimeRandomOffset`, beim Start gewürfelt) |
| `every_hours`, `times` | Wieder alle so viele Stunden (0: einmal; `RepeatTime`), höchstens so oft (0: unbegrenzt; `MaxRepetitions`) |
| `growth`, `growth_max` | Das n-te Mal bringt 1 + `growth` × (n − 1) so viele, das Plus höchstens `growth_max` (0: unbegrenzt) |
| `notify_hours` | So viele Stunden vorher **angekündigt** (oben auf dem Bildschirm, Countdown an der Uhr, Schwarm auf der Minimap); fehlt es, kommt es **still** |
| `dispersion` | Jede Seite lässt ihre Zombies aus einem Quadrat dieser Größe (Kacheln) an einer zufälligen Stelle heraus (`MaxCellDispersionGeneration`); 0 = über die ganze Seite |
| `final_wave`, `reveal` | Schlusswelle: ruft alle Zombies der Karte (auch Häuser und Beute-Wächter) – sie gehören dann dazu; deckt die Karte auf |
| `won`, `won_when_dead` | Die Runde ist gewonnen, wenn es kommt (TABs Sieg am Ende des letzten Tags) – bzw. wenn es jedes Mal kam und von der Schlusswelle keiner mehr lebt (alte Karten) |
| `groups` | Seine Zombies: je Gruppe `from` (`{"Sides": 1}` eine zufällige Seite = TABs `N \| S \| E \| W`, `{"Sides": 4}` jede Seite die volle Zahl = `N & S & E & W`, `"OneSpawner"`, `{"Spawners": [3, 7]}`) und `breeds` – je `count` (bis `up_to`, zufällig dazwischen – TABs „4-6“) und `breed` |
| `low_population` | Nur hier: unter Bevölkerung 0,5 gelten diese `hour`, `every_hours`, `times` (TABs Streuner kommen dann später und seltener) |

Der Plan (bei 100 Tagen, Normal):

| Event | ab | wie oft | von wo | Zombies je Seite | Wachstum | Streuung |
|---|---|---|---|---|---|---|
| Stragglers weak | Tag 3 (Stunde 48) | alle 2 Tage, 10× | alle 4 Seiten, still | 3–6 Walker | +10 % | ganze Seite |
| Swarm easy | Tag 14 (+ bis 1 Tag) | alle 10 Tage, 4× | 1 zufällige Seite, 8 h vorher | 16–24 Runner | +300 % | 8 Kacheln |
| Stragglers medium | Tag 26 (+ bis 1 Tag) | alle 6 Tage, 10× | alle 4 Seiten, still | 4–8 Runner, 1–2 Riots | +50 % | ganze Seite |
| Swarm hard | Tag 51 (+ bis 1 Tag) | alle 7 Tage, 5× | 1 zufällige Seite, 8 h vorher | 100–160 Runner, 25–40 Riots, 3–5 Fat | +350 % | 20 Kacheln |
| Final swarm | Tag 93 | 1× | alle 4 Seiten, 24 h vorher | 2.200 Runner, 550 Riots, je 50 Fat, Harpyien, Spitter | – | ganze Seite |
| Victory | Ende Tag 100 | | | | | |

- Arten wie in „Für jeden Schritt“ von `docs\tab-nachbau.md` (WeakA/B/C → Walker, Medium/Worker → Runner, DressedA →
  Riot, StrongA → Fat, Venom → Spitter); TABs Bereiche gleicher Arten zusammengezählt.
- Getötete Zombies kommen **nie** wieder – was du freiräumst, bleibt frei, bis neue nachkommen.
- **Gewonnen**, wenn „Victory“ kommt (Ende des letzten Tags) und das Kommandozentrum noch steht; **verloren**, wenn
  das letzte Kommandozentrum fällt oder infiziert ist (sein Schild bricht).

## `game_difficulties.json` – Schwierigkeiten

Vor der Runde gewählt (Startbildschirm „New game“, im Editor „New map“). `default`: die Schwierigkeit, wenn nichts
gewählt ist – und für eine Karte, deren Schwierigkeit es nicht (mehr) gibt. In `difficulties` eine pro Block:

| Wert | Bedeutung |
|---|---|
| `name`, `about` | Name auf dem Knopf, eine Zeile Erklärung darunter. Die Kartendatei merkt sich den Namen – umbenannt heißt für alte Karten: die `default` |
| `score_percent` | So viel Prozent zählt die Punktzahl (TABs Score-Faktor). Startbildschirm: Karte × Schwierigkeit × Dauer. Eine Punktzahl gibt es noch nicht |
| `start_gold`, `start_wood` | Gold und Holz zu Beginn (der übrige Vorrat startet leer). Im Editor pro Karte änderbar |
| `start_soldiers`, `start_rangers`, `start_snipers` | Einheiten zu Beginn |
| `roaming_zombies` | Streuner zu Beginn, pro 1.000 Kacheln, auf denen sie stehen können – in Grüppchen von `pack` (`roaming.json`), also füllen ≈ 2.000 fast jede Kachel. Nur auf unseren Karten – auf TABs zählt `population` |
| `population` | TABs Zombie-Bevölkerung (`FactorZombiePopulation`): so viel mal die Anzahlen der Events (`spawning.json`). Für Karten von TABs Generator: so viel mal das `infected` der Karte liegen Zombies; unter 0,5 nur halb so viele Dörfer und keine Riesen, ab 1 / 1,2 / 1,4 einmal / zweimal / dreimal ihre `giants` (darunter die Hälfte). Auf allen Karten: so viel mal `max_units` lässt ein Haus der Villages of Doom heraus, und so groß sind seine Schübe (`doom.json`) |
| `keep_out` | Pro Art (`walker`, `runner`, `fat`, `armored`, `spitter`, `infested`, `harpy`, `giant`): so nah ans Kommandozentrum (Kacheln) liegt zu Beginn keiner. Beim Wandern dürfen sie näher, wie in TAB (nur der Riese hält seinen `roam_ring`). Nur auf unseren Karten – auf TABs sagt die Karte, ab wo welche Art liegt (`weak_from` …) |
| `villages_percent`, `village_houses_percent` | So viel Prozent so viele Villages of Doom, wie die Karte hat (`villages`), und so viel Prozent ihrer Häuser (`houses_near`, `houses_edge`) – nur auf unseren Karten |
| `giants_percent` | So viel Prozent so viele **Riesen**, wie die Karte hat (`giants`) – nur auf unseren Karten |
| `loot_guards` | `[nahe der Basis, am Rand]`: so viele Zombies liegen um einen Haufen Beute versteckt |

## `game_durations.json` – Spieldauer

Wie lang eine Runde dauert – wie TABs 80 / 100 / 120 / 150 Tage, dazu eine kurze (20) zum Ausprobieren. 1 Tag = 60
Sekunden. `default_days`: die Dauer beim allerersten Start. In `durations` eine pro Block:

| Wert | Bedeutung |
|---|---|
| `days` | So viele Tage – TABs Plan (`spawning.json`) wird darauf gestreckt: Sieg am Ende des letzten, Schlusswelle bei 92 % |
| `score_percent` | Wie bei der Schwierigkeit: weniger Tage sind schwerer und zählen mehr (geschätzt) |

## `game_maps.json` – Karten

Die Karten wie in TAB. Vorerst ist eine Karte einer der Generatoren mit seinen Werten – und ihre Landschaft (`look`).
`default`: die Karte beim allerersten Start. In `maps` eine pro Block:

| Wert | Bedeutung |
|---|---|
| `name`, `about`, `score_percent` | Name, eine Zeile Erklärung, Score-Faktor |
| `setup` | Wie sie gemacht wird – alles, was die Seite „Custom setup“ hat: `generator` (`tab` – der einzige; alte Namen wie `wild` lesen sich als `tab`), `common` (`size`, `spawners_per_side`) und die Werte von TABs Generator unter `"tab"` (unten). Was fehlt, bleibt beim Standard. Die Namen der Werte stehen in einer Kartendatei unter `setup` |
| `setup.look` | Als welche von **TABs Landschaften** sie gezeichnet wird: `BR`, `TM`, `AL`, `DS`, `FA`, `VO` (Bilder aus `tools\tab_terrain.py`, nur auf deinem PC). Nur das Bild – das Spiel merkt nichts davon. Weglassen = BR. Die Kartendatei behält es (`look`) |
| `modifiers` | Was die **Landschaft** mit der ganzen Runde macht, in Prozent (100 = normal): `zombie_hearing` (ihre `awareness`), `zombie_sight` (damit auch, wie weit sie hören), `zombie_speed`, `unit_speed`, `unit_sight` (deine Einheiten und Gebäude), `building_energy` (Strombedarf, gerundet wie TAB: 1 bleibt 1, 4 → 5), `hunting_forest` / `_grass` / `_dirt` (Jagd pro Kachel, in Prozent von `gather_per_tile` des Jägers). TAB: AL Gehör 50, Tempo 80 für alle, Strom 130; DS Gehör 130, Sicht 110 für alle. Weglassen = 100, Jagd 60 / 30 / 15 (TABs BR). Vorerst für die ganze Karte. Der Startbildschirm zeigt sie unter der Vorschau; die Kartendatei behält sie (`modifiers`) |

**TABs Generator** (`"generator": "tab"`, die Landschaften „TAB FA“ … „TAB VO“ in TABs Reihenfolge, mit TABs Namen
und Score-Faktoren 80–175 %): seine Werte unter `"tab"`, TABs Namen in Klammern – **TABs eigene** aus seiner Tabelle
`MapThemes` (nur Kohle ist unsere, wie Eisen). Kälte in AL und Hitze in DS sind `modifiers` (oben). Von `common` nimmt er nur `size`,
`spawners_per_side` und `ore_richness`; Anzahlen (Dörfer, Ruinen, Riesen) gelten für TABs 256er-Karte und wachsen
mit der Fläche.

| Wert | Bedeutung |
|---|---|
| `land`, `land_dispersion`, `land_octaves` | Anteil Land, der Rest Seen (`LandWaterFactor`); 0 = große, 1 = kleine Seen (`LandDispersionFactor`); Feinheit der Ufer |
| `mountains` | Berge auf dem höchsten Land, in 2 × 2-Blöcken (`MountainLandFactor`) |
| `grass`, `forest_on_grass`, `forest_on_dirt`, `vegetation_octaves` | Gras auf dem Land, sonst Erde (`GrassEarthFactor`); Wald auf Gras (mal `grass`) und auf Erde (`ForestGrassFactor`, `ForestEarthFactor`) |
| `villages_min`, `villages_max`, `village_size_min`, `village_size_max` | So viele Villages of Doom (`NumDoomVillages`), so groß (Radius, `DoomVillagesSize` – über 8 ein großes Haus in der Mitte, über 12 jedes 8.) |
| `ruins_min`, `ruins_max` | Ruinen mit 2–4 Haufen Beute nahe der Basis (`NumTreasures`) |
| `energy_sites_min`, `energy_sites_max` / `food_sites_min`, `food_sites_max` | Weitere Beute-Stellen wie die Ruinen: ein Strommast mit 2 × Strom (`NumSuppyEnergy`) / ein brennender Lastwagen mit 2 × Nahrung (`NumSuppyFood`) – Vorräte, `food`/`energy` in `loot.json` |
| `stone`, `iron`, `coal`, `gold` / `stone_from`, `iron_from`, `coal_from`, `gold_from` | Wie viel davon auf freier Erde (`…EarthFactor`) / erst so weit vom Kommandozentrum (`…FormationMinDistance`). Kohle ist unsere (TAB hat keine): wie Eisen, nie auf Stein oder Eisen, mindestens 10 Kacheln in 25 um die Basis – sonst der beste von 100 Würfen, nie ein neuer Seed |
| `oil_block` | Eine Ölquelle pro Block dieser Kantenlänge (`NCellsPerOilSource`) |
| `infected`, `infected_dispersion` | Wie viel des Landes Zombies hat, mal `population` der Schwierigkeit (`InfectedFactor`); wie fein die Flecken (`InfectedDispersion`) |
| `weak_from`, `medium_from`, `strong_from`, `powerful_from` | Ab so weit draußen: Walker / Runner und Riots / Riots allein / Fat, Harpyie, Spitter (`MinDistanceFor…Infected`) |
| `giants` | Riesen bei Bevölkerung 1 (`NumInfectedGiants`) – mindestens 70 Kacheln draußen, 40 auseinander |
| `nests` | Wie groß die Schübe der Häuser der Villages of Doom sind (`InfectionNestsFactor`, `doom.json`) – geschätzt 1 |

## `research.json` – Forschung (wie TAB)

TABs Forschung (`Tech…` in seinen `Commands`): jede gehört zu einer Werkstatt, ist einmal zu bezahlen und gilt dann
die ganze Runde. Was sie freischaltet, braucht außerdem, was es `requires` (`buildings\`, `units\`).

| Wert | Bedeutung |
|---|---|
| `seconds_per_factor` | So lange dauert eine Forschung mal ihrem `time_factor` (TAB: 1,4 × Faktor × 50 = 70 s) – solange die Werkstatt Strom hat, langsamer bei zu wenig, gar nicht ausgeschaltet |
| `id` | TABs Name (`TechWoodHouse`) – für die Tastenbelegung und später die Skripte |
| `name` | Auf dem Knopf und in der Meldung „Research complete“ |
| `researched_in` | Die Werkstatt: `WoodWorkshop`, `StoneWorkshop`, `Foundry` |
| `slot` | Platz auf ihrer Karte, 0–14 Reihe für Reihe (TABs `PanelPosition` − 1; 8 Reparieren, 10 An/Aus, 14 Abreißen sind belegt) |
| `time_factor` | Mal `seconds_per_factor` – Wunder 2, Gasthaus 1,5. Weglassen = 1 |
| `blocked` | `true` = diese Forschung gibt es nicht: sie steht nicht auf der Karte der Werkstatt und lässt sich nicht starten – die Aktion „Research“ eines Triggers schenkt sie trotzdem. Im Standard immer `false`; gedacht für **eine Karte** (Editor → Data → „All researches“, Spalte Blocked), z. B. in einer Kampagne. Was von ihr abhängt (Gebäude, Einheiten, `needs_tech`), bleibt dann gesperrt |
| `upgrade` | Hebt eine Upgrade-Reihe (`upgrades.json`): `{ "track": "infantry_weapons", "level": 2 }` – die Forschung erscheint, wenn die Reihe auf Stufe 1 steht, und setzt sie auf 2. Was die Stufe den Einheiten gibt, steht in deren Dateien (`upgrades`). Weglassen = keine Reihe |
| `gold`, `wood`, `stone`, `iron`, `oil` | Was sie kostet – beim Start bezahlt, beim Abbrechen alles zurück |
| `buildings`, `units` | Was sie freischaltet |
| `needs_tech`, `needs_building` | Erst sichtbar, wenn diese Forschung fertig ist und dieses Gebäude steht (Thanatos, Titan: Ingenieurszentrum) |

## `trains.json` – der Zug (wie TAB)

TABs `Locomotive` und `TrainCarriage` (`src\sim\trains.rs`); TAB rechnet 30-mal pro Sekunde, hier umgerechnet.

| Wert | Bedeutung |
|---|---|
| `max_speed` | Höchstens so schnell (Kacheln/s – TAB: `MaxSpeed` 8 × 30 / 1000 je Takt = 7,2) |
| `acceleration` | So schnell wird er schneller (Kacheln/s²; TAB 0,001 je Takt = 0,9) |
| `brake_tiles` | Bremst vor einem Halt: höchstens `max_speed` × Rest-Weg / dies |
| `least_speed` | Aber nie langsamer als das, bis er da ist (Kacheln/s) |
| `stop_seconds` | So lange hält er (TABs `TimeInStopPoint` 3 s) |
| `gap` | Abstand zwischen zwei Wagen (Kacheln) |
| `damage`, `hits_from_speed` | Schneller als das (Kacheln/s): so viel Schaden je Takt für jeden auf dem Gleis vor, unter und hinter einem Wagen (vor Rüstung) |
| `cars` | Wie lang Lok und Wagen sind (Kacheln) |

## `audio.json` – welcher Ton zu wem

TABs `SoundOn…`-Spalten (`Entities`, `Commands`) auf unsere Arten. Jeder Wert ist ein **Listenname**: alle Dateien
seines Ordners in `assets\sounds\`, deren Name ihn enthält (`ZombieRoarMedium` → 15 Dateien), mehrere durch Leerzeichen
getrennt. Weglassen = still.

| Wert | Bedeutung |
|---|---|
| `units` | je Einheit: `voice` (Anfang ihrer Stimmen in `UnitVoices\`: `_What`, `_Yes`, `_Attack`, `_Trained`, `_Die`, `_Bye`), `shot`, `melee` (Thanatos' Schlag), `hit` (wo seine Rakete einschlägt) |
| `zombies` | je Art: `roar` (knurrt, rennt), `attack` (schlägt zu, spuckt), `died` (stirbt); der Riese dazu `death`, `fall` (0,7 s später), `steps` |
| `buildings` | je Gebäude: `placed`, `selected` (gewählt), `destroyed`, `infected`, `shot`, `hit`; `structure: true` = TABs Mauern, Tore, Türme, Fallen – kein „Gebäude fertig“ |
| `doom_small`, `doom_big` | Ein Dorfhaus fällt (das kleine, die anderen) |
| `ambient` | Der Klang der Landschaft je `look` (TABs `ZXMapTheme.AmbientSound`). Solange es regnet (Skript-Aktion „Weather“, auf VO von Anfang an), `RainLoopA` dazu |

## `upgrades.json` – Upgrade-Reihen (wie SC2)

Eine **Reihe** (`tracks`) hat eine Stufe von 0 bis `levels` (SC2: 3): *Infantry weapons/armor*, *Heavy weapons/armor* (Lucifer, Thanatos, Titan), *Tower weapons*, *Structure armor* (jedes Gebäude) und die der Zombies, *Zombie weapons/armor*. Die Stufe steigt durch

- eine **Forschung** mit `"upgrade": { "track": "infantry_weapons", "level": 1 }` (in `research.json`; sie erscheint, wenn die Reihe eine Stufe darunter ist – Stufe 2 erst nach Stufe 1),
- den **Start der Karte** (Editor → Data → Upgrades; `upgrades` in der Kartendatei, z. B. eine Kampagnenkarte, die auf Stufe 2 beginnt),
- die Trigger-Aktion *Research* (schenkt eine Forschung, auch eine blockierte),
- den Ausdruck `SetUpgrade("zombie_weapons", 2)` in einem Trigger (setzt die Stufe genau; lesen: `UpgradeLevel("…")`) – der Weg für die Reihen der Zombies, für die es keine Forschung gibt.

**Was eine Stufe gibt, steht beim Ding** (`upgrades` in dessen Datei, flach: Soldat +2 Schaden, Ranger +1, Scharfschütze +10) – nicht in der Forschung. Darum gilt ein Upgrade für alles, was schon steht, und für alles, was später gebaut wird; mehr Leben kommt sofort dazu. Karten können die Boni wie jeden Wert ändern (Data). Mitgeliefert sind die Reihen und Boni als Beispiel – **keine Forschung** hebt sie an, solange keine in `research.json` steht.

**Rüstung und Eigenschaften** (Einheit, Zombie, Gebäude): `armor` in Punkten – jeder Treffer verliert sie, mindestens 1 kommt durch (Gift und Feuer ignorieren sie); Gebäude dazu `shield_armor` für Treffer auf den Schild. `attributes`: `light`, `armored`, `massive`, `biological`, `mechanical`, `structure` (Gebäude ohne Angabe: gepanzert + Gebäude). `bonus` (Einheit, Zombie, `weapon` eines Turms): Zusatzschaden je Eigenschaft des Ziels, z. B. `{ "armored": 25 }`; trifft das Ziel mehrere, zählen alle. Eine Stufe kann `bonus_damage` geben – das geht auf jeden Zusatz der Waffe. Bei Zombies zählen aus `upgrades` nur `damage`, `bonus_damage`, `armor`, `attack_speed`.
