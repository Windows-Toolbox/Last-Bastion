# Spielwerte (`data\`)

Alle Zahlen des Spiels an einem Ort. Zahl ändern, speichern, im Spiel **Esc → Restart** (oder „New map“, oder neu
starten) – die neuen Werte gelten ab der nächsten Runde, ohne neu zu bauen. Ist eine Datei kaputt (Komma vergessen,
Wert fehlt), läuft das Spiel mit den alten Werten weiter und zeigt oben links eine **rote Zeile** mit Datei und Stelle.

- Die Dateien werden beim Bauen auch **in die Exe eingebaut**. `dist\LastBastion.exe` läuft also ohne den Ordner, mit
  den Werten vom letzten `build.ps1`.
- **Vor der Runde** wählt man wie in TAB **Karte, Schwierigkeit und Dauer** (`game_maps.json`,
  `game_difficulties.json`, `game_durations.json`). Was sie sagen, landet beim Start in der Kartendatei (Zombies,
  Wellen, Startgold …) – „Restart“ ändert daran nichts mehr. Nur `keep_out` der Schwierigkeit wird beim Spielen
  nachgeschaut, über ihren Namen in der Kartendatei.
- **Ausnahme `spawning.json`:** Der konstante Spawn und die Wellen gehören zur **Karte**. Eine neue Karte bekommt eine
  Kopie davon in ihre Kartendatei (eine Runde = eine Datei, siehe Haupt-README), danach zählt, was dort steht – im
  Editor änderbar. Eine Änderung hier gilt also erst für **neue** Spiele („New map“ oder „New game“), nicht für
  „Restart“ einer Karte, die es schon gibt.
- JSON kennt keine Kommentare – deshalb steht hier, was jeder Wert heißt.
- **Zeiten stehen in Sekunden** (`…_seconds`), das Spiel rechnet in Takten (20 pro Sekunde) und rundet.
- **1 Tag = 3 Minuten, 1 Stunde = 7,5 Sekunden** (steht noch im Code, `DAY_TICKS` in `src\sim\mod.rs`).
- **„Pro Abrechnung“:** Die Wirtschaft rechnet alle `economy_every_hours` Stunden ab (`colony.json`, 8 wie TAB =
  60 s): Gold, Nahrung, Holz, Unterhalt – alle Werte dazu gelten pro Abrechnung.
- Entfernungen in **Kacheln**, Geschwindigkeiten in **Kacheln pro Sekunde**.
- Im Koop müssen später beide PCs dieselben Werte haben, sonst laufen die Spiele auseinander.

## `units.json` – Einheiten

Ein Block pro Typ: `soldier`, `ranger`, `sniper`, `medic` – und die Söldner `merc_soldier`, `merc_ranger`,
`merc_sniper` (aus dem Gasthaus). Die Werte sind (fast) die aus *They Are Billions*.

Ein Schuss geht wie in StarCraft: **drehen** (bis die Einheit zum Ziel schaut) → **nachladen** abwarten (zählt ab dem
letzten Schuss) → **zielen** → Schuss. Ein Schuss dauert also `aim_seconds + cooldown_seconds` – so viel kleiner als
TABs Zeit zwischen zwei Schüssen ist `cooldown_seconds` gewählt.

| Wert | Bedeutung |
|---|---|
| `name`, `letter` | Name im Spiel, 1–2 Buchstaben fürs Symbol |
| `cost` | Preis in Gold. Bezahlt, sobald sie in die Bauschlange kommt; Abbrechen gibt alles zurück (wie StarCraft) |
| `worker_cost` | **Arbeiter**, die sie braucht – ab der Bauschlange, solange sie lebt (stirbt sie, sind sie wieder frei) |
| `upkeep` | **Unterhalt:** Gold pro Abrechnung, solange sie lebt (noch nicht in der Bauschlange). Reicht das Gold nicht: Schulden (`colony.json`) |
| `food` | **Nahrung** pro Abrechnung, solange sie lebt – wie TAB (Soldat 1). Die Einheiten essen vor den Häusern; fehlt Nahrung, passiert ihnen nichts, aber den Häusern fehlt sie. Weglassen = 0 |
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
| `requires` | Muss erst stehen, damit sie ausgebildet werden kann – Soldat und Scharfschütze: `"MunitionsFactory"`. Weglassen = nichts |
| `trained_in` | Wo sie ausgebildet wird: `"Barracks"` (weglassen) oder `"Inn"` (die Söldner) |
| `mercenary` | `true` = Söldner: bei Schulden geht er zuerst, schon ab Stufe 2 (`colony.json`), und trägt eine goldene Marke. Weglassen = `false` |
| `noise` | **Lärm pro Schuss** wie in TAB (Ranger 1, Soldat 3, Scharfschütze 10). Er sammelt sich dort, wo geschossen wird – ein langer Kampf wird immer lauter (siehe `sound.json`) |
| `heals` | `true` = schießt nicht, sondern richtet den Strahl auf eigene Einheiten (Medic) |
| `sprite` | Nur die Optik: ihr Bild in `assets\` (`"test/figure"` = `assets\test\figure.json`, siehe `assets\README.md`). Weglassen = Kästchen |

## `buildings.json` – Gebäude

Ein Block pro Typ: `command_center`, `barracks` (die Kaserne), `tent`, `wall`, `gate`, `tower`,
`sawmill`, `mine`, `hunter_cottage`, `fishing_hut`, `farm`, `windmill`, `charcoal_burner`, `wood_workshop`,
`tesla_tower`, `stone_workshop`, `munitions_factory`, `stone_house`, `stone_wall`, `stone_tower`, `stakes` (die
Pfahlfalle), `market`, `bank`, `inn` (das Gasthaus – bildet die Söldner aus), `barrel` (das Sprengfass).

| Wert | Bedeutung |
|---|---|
| `name`, `letter` | Name im Spiel, Buchstaben fürs Symbol |
| `size` | `[Breite, Tiefe]` in Kacheln (das Tor lässt sich drehen) |
| `hp` | Lebenspunkte |
| `armor` | Wird von jedem Zombie-Treffer abgezogen (mindestens 1 kommt durch). Kommandozentrum 10 wie TAB, weglassen = 0 |
| `shield_percent` | Schild in **Prozent der HP** (Zelt: 133 % von 150 = 199; Kommandozentrum 50 % von 5000 = TABs Barriere 2500). Zombies schlagen erst darauf. Bricht er, ist das Gebäude **infiziert** und seine Kolonisten kommen als Zombies heraus. Ohne Kolonisten – und beim Kommandozentrum – ist er nur zusätzliches Leben. 0 = kein Schild. Im Spiel ein Balken mit den HP: der Schild graublau dahinter |
| `colonists` | Kolonisten darin: zahlen Gold und werden bei der Infektion zu Zombies (2–4 Runner, Rest Walker) |
| `workers` | **Arbeiter**, die es gibt (Zelt, Kommandozentrum) – infiziert keine |
| `worker_cost` | Arbeiter, die es braucht, solange es steht. Nicht genug frei: kann nicht gebaut werden (im Editor egal) |
| `cost` | Preis in Gold, bezahlt beim Platzieren (im Editor kostenlos) |
| `cost_wood`, `cost_stone`, `cost_coal` | Dazu Holz, Stein und Kohle aus dem Vorrat (weglassen = 0) |
| `requires` | Muss erst stehen, z. B. `"WoodWorkshop"` (Name des Typs wie in den Kartendateien; weglassen = nichts) |
| `energy` | Strom, den es ins Netz gibt (Kommandozentrum, Windmühle, Köhlerei) |
| `energy_use` | Strom, den es braucht. Gebaut (oder mit Y wieder an) nur, wenn sein Netz so viel übrig hat; fällt danach Strom weg, arbeitet es anteilig langsamer |
| `power_radius` | Kommandozentrum, Tesla-Turm: so weit (Kacheln ab der Mitte) reicht ihr Netz. Wer Strom erzeugt, braucht oder trägt, muss darin stehen |
| `burns_wood` | Holz pro Abrechnung – ohne Holz kein Strom (Köhlerei) |
| `food` | Nahrung pro Abrechnung – ohne sie zahlen die Kolonisten kein Gold (Zelt) |
| `food_made` | Nahrung pro Abrechnung aus dem Nichts, ohne Kacheln (Kommandozentrum 20 wie TAB; ein Zelt isst 4) |
| `gold` | Gold pro Abrechnung, mal Stromanteil – Häuser nur, wenn ihre Kolonisten **satt** sind (wie TAB: Zelt 8, Steinhaus 40, Kommandozentrum 200 ohne zu essen) |
| `upkeep` | **Unterhalt:** Gold pro Abrechnung, solange es läuft – aus (Y) oder infiziert: nichts. Weglassen = 0 |
| `gather_range` | Sammler: so viele Kacheln um seine Fläche herum arbeitet es, auf jeder Seite – ein Quadrat (Mine 2 × 2 mit 2 → 6 × 6 wie TAB) |
| `gather_below` | Sammler: `true` = die Kacheln unter ihm zählen mit (Mine: das Erz darunter). Weglassen = `false`. Bei Sägewerk, Jäger und Fischer ohne Wirkung – sie stehen nie auf Wald oder Wasser |
| `trap_damage`, `trap_wear` | Falle: Schaden an jedem Zombie darauf (minus Rüstung) alle `trap_every_seconds`, und so viele eigene HP kostet sie jeder Treffer |
| `trade_range`, `house_bonus_percent`, `colonists_per_good` | Markt: seine Häuser sind die, deren Mitte auf ihm oder bis so viele Kacheln drumherum liegt (2 → 7 × 7) – jedes Haus nur für einen Markt, den ältesten. Sie zahlen so viel Prozent mehr Gold, und für je so viele Kolonisten darin verkauft er 1 Ware pro Abrechnung |
| `gold_storage`, `credit` | Bank: so viel mehr Gold passt ins Lager, und so viel **Dispo** gibt sie – mit Shift darf man so weit ins Minus kaufen, ohne Schuldenstrafen (Zinsen: `colony.json`) |
| `barrel_storage` | Munitionsfabrik: so viele Sprengfässer fasst ihr Lager |
| `noise` | **Lärm pro Arbeitstakt** (`work_every_seconds` in `sound.json`), solange es arbeitet – mal Stromanteil, aus/infiziert/ohne Holz still. Jägerhütte 50, Sägewerk und Munitionsfabrik 100, Mine und Köhlerei 150. Weglassen = 0 |
| `gather_per_tile` | Sammler: so viel bringt jede passende Kachel pro Abrechnung (bei vollem Strom). Was passt, steht im Code: Sägewerk und Jäger Wald + Bäume, Fischer Wasser, Mine Erz (auch unter sich), Farm Gras (nicht Erde, nicht Erz – und sie selbst steht nur auf Gras) |
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
| `trap_every_seconds` | So oft treffen Fallen die Zombies auf ihnen |
| `sell_from_percent`, `sell_prices` | Märkte verkaufen nur, was über so viel Prozent des Lagers liegt, zu diesen Preisen (Gold pro Einheit: `wood`, `stone`, `iron`, `coal`, `food`) |
| `blast` | Sprengfass: `cost` Gold + `cost_coal` Kohle und `build_seconds` in der Munitionsfabrik. Geht es hoch: `damage` an allen bis `radius` Kacheln (Zombies und Einheiten minus Rüstung, Gebäude wie ein Treffer, Häuser der Dörfer wie ein Schuss), so laut wie `noise`. Wer eins trägt, läuft `carry_slow_percent` langsamer und schießt nicht |
| `scan` | Scan des Kommandozentrums: zeigt `seconds` lang einen Kreis von `radius` Kacheln (auch über Wald). Höchstens `charges` Ladungen, alle `every_seconds` eine neue – solange eine lädt, braucht das Kommandozentrum `power` Strom mehr |

## `zombies.json` – Zombies

`walker`, `runner`, `fat` (TABs „Chubby“), `armored` (TABs gepanzerter Runner, im Spiel „Riot“), `spitter` (TABs
Venom) und `infested` (StarCraft 2s Infested Marine), je mit:

| Wert | Bedeutung |
|---|---|
| `name` | Name im Spiel. In Karten- und Datendateien heißt die Art weiter wie ihr Block (`"Armored"`) – ein neuer Name ist also nur diese Zeile |
| `alertness` | **Wachsamkeit** wie in TAB: kommt bei Lärm ab `threshold` (in `sound.json`) / `alertness` (Walker 2 → 500, Runner, Fat, Riot, Infested 3 → 333, Spitter 4 → 250) |
| `speed` | Laufgeschwindigkeit (jeder Zombie bekommt zufällig ±20 %) |
| `hp` | Lebenspunkte |
| `armor` | Wird von jedem Schuss abgezogen (mindestens 1 Schaden kommt immer durch) |
| `damage` | Schaden pro Treffer – beim Spitter an jedem im Spritzer |
| `sight` | So weit sieht er Einheiten (nicht durch Wald und Bäume). TAB: Walker 5, Runner 6, Fat 5 |
| `stroll` | Als **Wanderer** geht er pro Ausflug höchstens so weit (Kacheln, in x und y) – von da, wo er gerade steht, ohne festen Platz: mit der Zeit überall hin. Je seltener die Art, desto weiter: Walker 8, Runner 14, Riot 18, Infested 22, Spitter 24, Fat 30. Nie näher ans Kommandozentrum als `keep_out` seiner Art in der Schwierigkeit (`game_difficulties.json`) |
| `radius` | Wie groß er ist (Kacheln): so weit halten andere Zombies und Einheiten Abstand, und so viel weiter reicht er |
| `range` | **Fernkampf**: so weit schießt er (Kacheln) – über Mauern, nicht durch Wald. Weglassen = 0: er muss dich berühren. Infested 4,5 = knapp unter dem Soldaten (5) |
| `splash` | Sein Schuss trifft jeden so nah am Einschlag (Spitter): Einheiten und Gebäude, keine Zombies. Weglassen = nur das Ziel |
| `attack_seconds` | Zeit zwischen zwei Angriffen. Weglassen = `hit_every_seconds` |
| `projectile_speed` | Sein Schuss fliegt so viele Kacheln pro Sekunde und landet, wo das Ziel stand – wer rechtzeitig weggeht, wird nicht getroffen (Spitter). Weglassen = trifft sofort |
| `sprite` | Nur die Optik: sein Bild in `assets\`, wie bei den Einheiten. Weglassen = Kästchen |

Dazu für alle:

| Wert | Bedeutung |
|---|---|
| `hit_every_seconds` | Zeit zwischen zwei Treffern |
| `hearing` | So nah bemerken sie eine Einheit auch ohne sie zu sehen |
| `notice_buildings` | Ein Gebäude so nah wird angegriffen |
| `alert_seconds` | So lange stürmt ein angeschossener Zombie los, auch ohne den Schützen zu sehen |
| `charge_steps` | So weit (in Schritten) finden stürmende Zombies den Weg zu einer Einheit |
| `seek_helpers` | Der angeschossene Zombie und so viele der nächsten um ihn herum suchen die **Lücke** in der Mauer … |
| `seek_radius` | … wenn sie höchstens so weit von ihm stehen … |
| `seek_seconds` | … so lange … |
| `seek_steps` | … und höchstens so viele Schritte zu Fuß um Mauern herum |
| `wander_pace` | Wanderer streifen mit diesem Anteil ihrer Geschwindigkeit herum (wie weit pro Ausflug: `stroll` der Art) |
| `rise_min_seconds`, `rise_max_seconds` | Eine tote Einheit steht nach so vielen Sekunden als Walker auf (zufällig dazwischen) |

## `sound.json` – Lärm

**Lärm** wie in TAB: Er sammelt sich dort, wo etwas passiert (je 4 × 4 Kacheln), solange es weitergeht. Wie laut ein
Schuss ist: `noise` in `units.json`.

| Wert | Bedeutung |
|---|---|
| `threshold`, `hearing` | Ein Zombie kommt bei so viel Lärm geteilt durch seine `alertness` und hört so viel mal seine Sichtweite weit (TAB: 1000 und 4). Weiter weg ist es leiser (am Rand halb so laut); er kommt per Zufall – gerade laut genug jedes zweite Mal, doppelt so laut immer. Er geht **dorthin, wo der Lärm war**, und bleibt da |
| `quiet_seconds`, `halve_seconds` | Ist es dort so lange still, halbiert sich der Lärm – danach immer wieder nach so vielen Sekunden (TAB: 1 und 1) |
| `zombie_hit`, `unit_dies`, `building_placed`, `building_destroyed` | So laut: jeder Treffer eines Zombies, eine sterbende Einheit, ein platziertes Gebäude, ein zerstörtes Gebäude |
| `building_infected`, `per_infected` | Ein infiziertes Gebäude – plus so viel für jeden, der herauskommt. Ein Gebäude mit Arbeitern, das fällt, ist infiziert: die Arbeiter kommen als Zombies heraus (TAB: eine überrannte Jägerhütte weit draußen lockt die Horde) |
| `work_every_seconds` | So oft macht jedes arbeitende Gebäude seinen Lärm (`noise` in `buildings.json`) – alle im selben Moment, Nachbarn zählen zusammen |
| `unrest_per_zombie` | **Unruhe:** aller Lärm der Kolonie (Schüsse, Maschinen, Explosionen, Bauen) zählt zusammen und sinkt nie – je so viel davon bringt jede Gruppe des konstanten Spawns für immer einen Zombie mehr |

## `roaming.json` – Streuner

Die Zombies, die zu Beginn über die Karte streifen (wie viele: `roaming_zombies` der Schwierigkeit). Entfernungen als
Anteil des Wegs von der Basis zum Rand (0 = Mitte, 1 = Rand). Wie weit sie pro Ausflug bummeln: `stroll` der Art in
`zombies.json`, wie nah ans Kommandozentrum: `keep_out` der Schwierigkeit. Weit weg von deinen Augen schlafen sie, bis
etwas sie weckt (DESIGN.md 8.5).

| Wert | Bedeutung |
|---|---|
| `edge_weight` | Wie viel mehr weiter draußen liegen: 0 = überall gleich viele, 1 = mit der Entfernung gleichmäßig mehr, 2 = fast alle am Rand (jetzt 0,5: fast überall, außen etwas mehr – wie TAB) |
| `inside_step` | Wer **im Ring** seiner Art steht (`keep_out` der Schwierigkeit – angelockt, Pech gehabt), bummelt von dort zufällig weiter: pro Ausflug höchstens so viele Kacheln näher ans Kommandozentrum. Ausflüge weiter hinein fallen weg – im Ganzen bummelt er also wieder hinaus. Pro Zombie wird dafür nichts gespeichert |
| `pack` | `[wenigste, meiste]`: sie liegen in Grüppchen auf je einer Kachel, eine Gruppe pro Kachel – also nie mehr als `meiste` auf einer Kachel (wie TAB: bis 3). `[3, 3]` = überall genau 3 (dann ≈ doppelt so viele möglich, Regler bis 2.000 reicht dann nicht mehr ganz) |
| `mix` | Welche Arten außer Walkern: je `breed` (Name wie in den Dateien), `from` (ab so weit draußen) und `share` (so ein Anteil der Zombies dort). Der Rest sind Walker. Gilt auch für die Villages of Doom und die Wächter der Beute. Näher am Kommandozentrum als `keep_out` ihrer Art (Schwierigkeit) wird eine Art zum Walker |

## `doom.json` – Villages of Doom

Die Häuser der **Villages of Doom** (wie TAB) – wie viele Dörfer und wie viele Häuser darin: die Karte („Villages of
doom“, „Houses near base“, „Houses at edge“), mal `villages_percent` / `village_houses_percent` der Schwierigkeit. Wie
viele Zombies in einem Haus schlafen: `doom_zombies` der Schwierigkeit – in jedem Haus genau gleich viele.

| Wert | Bedeutung |
|---|---|
| `hp`, `armor` | Lebenspunkte eines Hauses, Rüstung gegen jeden Schuss |
| `wake_radius`, `near_zombies`, `near_every_seconds` | Steht eine Einheit so nah (Kacheln ab der Wand), kommen nur **ein paar** heraus (so viele) – wieder nach so vielen Sekunden, solange sie bleibt. Sie gehen auf die Einheit los |
| `hearing`, `hear_threshold` | Hört das Haus **Lärm** so nah, mindestens so laut, … |
| `release_per_second` | … strömen so viele pro Sekunde heraus, solange der Lärm anhält – und gehen dorthin, wo er war. Abgeschossen: der Rest kommt auf einmal und geht auf den Schützen los |
| `hit_zombies` | Jeder Treffer lässt so viele **zusätzlich** heraus (nicht von den drinnen abgezogen), solange es nicht leer ist |
| `loot_gold`, `loot_goods` | Ein abgeschossenes Haus lässt Beute fallen: so viel Gold – oder so viel Holz, Stein, Eisenerz oder Kohle |

## `loot.json` – Beute

Die **Beute**, die herumliegt – wie viel: Karteneinstellung „Loot“. Wie viele Wächter: `loot_guards` der
Schwierigkeit.

| Wert | Bedeutung |
|---|---|
| `wake_radius`, `hearing`, `hear_threshold` | Kommt eine Einheit so nah – oder ist Lärm so nah mindestens so laut –, kommen die versteckten Wächter auf einmal aus dem Boden (und gehen auf die Einheit bzw. zum Lärm) |
| `gold`, `goods`, `gold_share` | Ein Haufen: so viel Gold oder Holz/Stein/Eisenerz/Kohle – Gold in diesem Anteil der Haufen |

## `colony.json` – Kolonie

Womit die Kolonie startet (Gold, Holz, Einheiten), sagt die Schwierigkeit (`game_difficulties.json`).

| Wert | Bedeutung |
|---|---|
| `economy_every_hours` | So viele Stunden von einer **Abrechnung** der Wirtschaft zur nächsten (TAB: 8) |
| `storage_gold`, `storage_goods`, `storage_food` | **Lager:** so viel Gold, so viel von jeder Ware (Holz, Stein, Eisenerz, Kohle) und so viel Nahrung passt höchstens hinein – wie TABs Kommandozentrum (2000 Gold, 50 Waren). Was bei einer Abrechnung oder als Beute darüber hinaus käme, ist verloren. Nahrung lagert TAB nicht, bei uns sammelt sie sich zwischen den Abrechnungen – daher eine eigene Grenze. In der Leiste: der Balken füllt sich, voll wird er rot |
| `debt_penalty_percent` | **Schulden:** Unterhalt, den das Gold nicht deckt, wird Schulden – neues Gold zahlt sie zuerst ab. Verschuldet machen die Einheiten so viel Prozent weniger Schaden und schießen so viel langsamer |
| `deep_debt_payouts`, `deep_debt_penalty_percent`, `deep_debt_slow_percent` | Mehr Schulden als so viele Abrechnungen Unterhalt (gerade jetzt): so viel Prozent weniger Schaden, und sie laufen so viel langsamer |
| `desert_debt_payouts`, `desert_every_seconds` | Mehr als so viele Abrechnungen: alle so viele Sekunden **desertiert** eine Einheit – weg für immer, je mehr Unterhalt sie kostet, desto eher. Söldner schon ab `deep_debt_payouts`, und solange es welche gibt, nur sie |
| `credit_interest_percent` | Schulden im Dispo der Banken bringen keine Strafen (gezählt wird nur, was darüber liegt) – sie kosten so viel Prozent davon an Zinsen pro Abrechnung |

## `spawning.json` – konstanter Spawn und Wellen einer neuen Runde

Beides bekommt eine **neue Karte** als Kopie in ihre Kartendatei – danach ändert man es im **Editor** (Tab „Waves“)
oder direkt in der Kartendatei. Hier steht also nur, womit neue Karten anfangen.

### Konstanter Spawn (`stages`, `extra_per_day`, `specials`)

Nachschub aus den Spawnern, die auf „Constant“ stehen (neue Karten: alle). Getötete Zombies kommen **nie** wieder –
was du freiräumst, bleibt frei, bis neue nachkommen.

`stages` ist eine Liste von Abschnitten, **nach Zeit sortiert**. Jeder gilt ab seinem Zeitpunkt bis zum nächsten:

| Wert | Bedeutung |
|---|---|
| `day`, `hour` | Ab wann (Tag ab 1, Stunde 0–23). `"day": 2, "hour": 0` = Ende von Tag 1 |
| `breed` | `"Walker"`, `"Runner"`, `"Fat"`, `"Armored"`, `"Spitter"` oder `"Infested"` |
| `every_seconds` | Alle so viele Sekunden kommt eine Gruppe … |
| `group` | … aus `[min, max]` Zombies (zufällig dazwischen), alle aus einem zufälligen Spawner – **dicht beieinander an einem Punkt** darin |

- Hat der **nächste** Abschnitt **dieselbe Art**, gehen die Werte **gleitend** dorthin über (so werden die Walker
  von Tag 2 bis Tag 3 nach und nach mehr).
- Wechselt die Art, bleiben die Werte bis zum Wechsel gleich.
- Nach dem **letzten** Abschnitt wird jede Gruppe pro Tag um `extra_per_day` Zombies größer.
- `specials`: je `day` (ab diesem Tag), `breed` und `share` – so ein Anteil jeder Gruppe ist ab dann diese Art statt der
  des Abschnitts (per Zufall, nach der Reihe der Liste). Im Editor noch nicht änderbar, nur in der Datei.
- Die Schwierigkeit macht jede Gruppe `constant_spawn_percent` so groß (auch `extra_per_day`) – beim Anlegen der Karte.
- Dazu kommt die **Unruhe** (`unrest_per_zombie` in `sound.json`): jede Gruppe für immer so viele Zombies mehr.
- Was gerade kommt, zeigt die Entwickler-Anzeige (**F3**): „Spawn: 4-5 Runners every 60.0 s“.

### Wellen einer neuen Runde (`waves`)

Aus der Dauer (`game_durations.json`: wie viele Tage, ab wann und wie oft) wird die **Zeitleiste** geplant (DESIGN.md
8.2). Die Zahlen gelten **pro Seite**, aus der die Welle kommt; die Schwierigkeit nimmt alle mal `waves_percent`.

| Wert | Bedeutung |
|---|---|
| `hour` | Jede Welle kommt zu dieser Stunde |
| `first`, `last` | Die erste Welle hat so viele Runner, die letzte vor der Schlusswelle so viele – dazwischen gleichmäßig mehr. Die erste Hälfte der Wellen kommt von 1 Seite, die zweite von 2 |
| `specials` | Arten, die mitkommen: je `breed`, `from` (ab diesem Anteil der Wellen: 0 = erste, 1 = letzte vor der Schlusswelle) und `share` (so viele pro Runner bei der letzten Welle – beim ersten Auftauchen ein Viertel davon). Die ersten Wellen sind also nur Runner. Sie kommen gleichzeitig aus denselben Spawnern wie die Runner |
| `final_hour` | Die **Schlusswelle** kommt am letzten Tag zu dieser Stunde, von **allen 4 Seiten** … |
| `final_runners` | … mit so vielen Runnern pro Seite und allen `specials` in vollem Anteil. Sie **ruft alle Zombies der Karte** (auch die in Häusern und um Beute) – die gehören dann zur Schlusswelle – und **deckt die Karte auf** |

Mit 10 Tagen (die Mini-Runde, Schwierigkeit Normal): Welle am Tag 4 (200 Runner, 1 Seite) und Tag 7 (600 Runner + 72
Riots, 48 Infested, 36 Spitter, 24 Fat × 2 Seiten), Schlusswelle am Tag 10 um 6:00 (1.000 Runner + 120 Riots, 80
Infested, 60 Spitter, 40 Fat × 4 Seiten = 5.200 Zombies – dazu alle, die noch auf der Karte sind). Mit 40 Tagen (ab
Tag 5 alle 4): 9 Wellen, dann die Schlusswelle.
**Gewonnen** ist die Runde, wenn alle Zombies der Schlusswelle tot sind; **verloren**, wenn das letzte
Kommandozentrum fällt.

## `game_difficulties.json` – Schwierigkeiten

Vor der Runde gewählt (Startbildschirm „New game“, im Editor „New map“). `default`: die Schwierigkeit, wenn nichts
gewählt ist – und für eine Karte, deren Schwierigkeit es nicht (mehr) gibt. In `difficulties` eine pro Block:

| Wert | Bedeutung |
|---|---|
| `name`, `about` | Name auf dem Knopf, eine Zeile Erklärung darunter. Die Kartendatei merkt sich den Namen – umbenannt heißt für alte Karten: die `default` |
| `score_percent` | So viel Prozent zählt die Punktzahl (TABs Score-Faktor). Startbildschirm: Karte × Schwierigkeit × Dauer. Eine Punktzahl gibt es noch nicht |
| `start_gold`, `start_wood` | Gold und Holz zu Beginn (der übrige Vorrat startet leer). Im Editor pro Karte änderbar |
| `start_soldiers`, `start_rangers`, `start_snipers` | Einheiten zu Beginn |
| `roaming_zombies` | Streuner zu Beginn, pro 1.000 Kacheln, auf denen sie stehen können – in Grüppchen von `pack` (`roaming.json`), also füllen ≈ 2.000 fast jede Kachel |
| `keep_out` | Pro Art (`walker`, `runner`, `fat`, `armored`, `spitter`, `infested`): so nah ans Kommandozentrum (Kacheln) kommt keiner – nicht zu Beginn, nicht beim Bummeln (auch nicht auf dem Weg zum Ziel). Wer angelockt oder mit einer Welle kommt, kümmert sich nicht darum – und bummelt danach wieder hinaus |
| `villages_percent`, `village_houses_percent` | So viel Prozent so viele Villages of Doom, wie die Karte hat (`villages`), und so viel Prozent ihrer Häuser (`houses_near`, `houses_edge`) |
| `doom_zombies` | So viele Zombies schlafen in **jedem** Haus der Villages of Doom – fest, egal wie weit draußen |
| `loot_guards` | `[nahe der Basis, am Rand]`: so viele Zombies liegen um einen Haufen Beute versteckt |
| `waves_percent` | Jede Welle (auch die Schlusswelle) so viel Prozent der Zahlen in `spawning.json` |
| `constant_spawn_percent` | Jede Gruppe des konstanten Spawns so viel Prozent so groß |

## `game_durations.json` – Spieldauer

Wie lang eine Runde dauert (wie TABs 80 / 100 / 120 / 150 Tage) – 1 Tag = 3 Minuten. `default_days`: die Dauer beim
allerersten Start. In `durations` eine pro Block:

| Wert | Bedeutung |
|---|---|
| `days` | So viele Tage – die Schlusswelle kommt am letzten |
| `score_percent` | Wie bei der Schwierigkeit: weniger Tage sind schwerer und zählen mehr |
| `first_wave_day`, `wave_every_days` | Die erste Welle kommt an diesem Tag, dann alle so viele Tage eine, bis vor dem letzten Tag. Eine Karte mit anderen Tagen (im Editor geändert) nimmt die Dauer mit den nächstliegenden Tagen |

## `game_maps.json` – Karten

Die Karten wie in TAB. Vorerst ist eine Karte einer der Generatoren mit seinen Werten – sie sieht nicht anders aus.
`default`: die Karte beim allerersten Start. In `maps` eine pro Block:

| Wert | Bedeutung |
|---|---|
| `name`, `about`, `score_percent` | Name, eine Zeile Erklärung, Score-Faktor |
| `setup` | Wie sie gemacht wird – alles, was die Seite „Custom setup“ hat: `generator` (`wild`, `regions`, `rings`, `canyons`, `blank`), `common` (z. B. `size`, `ways_in`, `loot_piles` und die Dörfer: `villages` pro 10.000 Kacheln, `houses_near` / `houses_edge` Häuser pro Dorf – Open Plains viele kleine, Broken Lands zwei riesige) und die Werte des Generators (z. B. `"blank": { "trees": 0.02 }`). Was fehlt, bleibt beim Standard. Die Namen der Werte stehen in einer Kartendatei unter `setup` |
