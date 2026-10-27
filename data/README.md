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
- **Ausnahme `spawning.json`:** Die Events (Wellen, Streuner, Schlusswelle, Sieg) gehören zur **Karte**. Eine neue
  Karte bekommt eine Kopie davon in ihre Kartendatei (eine Runde = eine Datei, siehe Haupt-README), danach zählt, was dort steht – im
  Editor änderbar. Eine Änderung hier gilt also erst für **neue** Spiele („New map“ oder „New game“), nicht für
  „Restart“ einer Karte, die es schon gibt.
- JSON kennt keine Kommentare – deshalb steht hier, was jeder Wert heißt.
- **Zeiten stehen in Sekunden** (`…_seconds`), das Spiel rechnet in Takten (20 pro Sekunde) und rundet.
- **1 Tag = 60 Sekunden, 1 Stunde = 2,5 Sekunden** wie in TAB (steht im Code, `DAY_TICKS` in `src\sim\mod.rs`).
- **„Pro Abrechnung“:** Die Wirtschaft rechnet alle `economy_every_hours` Stunden ab (`colony.json`, 8 wie TAB =
  60 s): Gold, Nahrung, Holz, Unterhalt – alle Werte dazu gelten pro Abrechnung.
- Entfernungen in **Kacheln**, Geschwindigkeiten in **Kacheln pro Sekunde**.
- Im Koop müssen später beide PCs dieselben Werte haben, sonst laufen die Spiele auseinander.

## `units.json` – Einheiten

Ein Block pro Typ: `soldier`, `ranger`, `sniper`, `medic`, `spearman` (der Speerträger) – und die Söldner
`merc_soldier`, `merc_ranger`, `merc_sniper` (aus dem Gasthaus). Die Werte sind (fast) die aus *They Are Billions*.
Dazu der Block `veterans` (unten).

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
| `noise` | **Lärm pro Schuss** wie in TAB (Ranger 1, Soldat 3, Scharfschütze 10). Er sammelt sich dort, wo geschossen wird, und halbiert sich jede Sekunde – ein langer Kampf pendelt sich ein (siehe `sound.json`) |
| `heals` | `true` = schießt nicht, sondern richtet den Strahl auf eigene Einheiten (Medic) … |
| `heal_per_second` | … und heilt so viele HP pro Sekunde … |
| `heal_after_seconds` | … aber nur eine Einheit, die so lange nicht getroffen wurde – kein Heilen mitten im Kampf (nicht wie SC2) |
| `knockback` | Nahkampf (Speerträger): jeder Treffer stößt den Zombie so viele Kacheln zurück – einen großen weniger (Walker-Größe ganz, Fat ≈ halb, Riese ≈ ein Viertel). Weglassen = 0 |
| `sprite` | Nur die Optik: ihr Bild in `assets\` (`"test/figure"` = `assets\test\figure.json`, siehe `assets\README.md`). Weglassen = Kästchen |

**Veteranen** (`veterans`): jeder Zombie, der stirbt, gibt seine Erfahrung (`xp` in `zombies.json`), geteilt unter
deinen Einheiten im Umkreis – ein Veteran nimmt einen halben Anteil. Söldner nehmen nichts (sie sind schon besser).

| Wert | Bedeutung |
|---|---|
| `xp` | So viel Erfahrung, und die Einheit ist **Veteran** – silberne Marke, desertiert bei Schulden dreimal seltener |
| `share_radius` | Einheiten in so vielen Kacheln um den toten Zombie teilen sich seine Erfahrung |
| `hp_percent`, `damage_percent` | So viel Prozent mehr HP (kommen sofort dazu) und Schaden hat ein Veteran |

## `buildings.json` – Gebäude

Ein Block pro Typ: `command_center`, `barracks` (die Kaserne), `tent`, `wall`, `gate`, `tower`,
`sawmill`, `mine`, `hunter_cottage`, `fishing_hut`, `farm`, `windmill`, `charcoal_burner`, `wood_workshop`,
`tesla_tower`, `stone_workshop`, `munitions_factory`, `stone_house`, `stone_wall`, `stone_tower`, `stakes` (die
Pfahlfalle), `market`, `bank`, `inn` (das Gasthaus – bildet die Söldner aus), `barrel` (das Sprengfass), `foundry` (die
Gießerei), `ballista` (die Balliste), `machine_gun_tower` (der MG-Turm).

| Wert | Bedeutung |
|---|---|
| `name`, `letter` | Name im Spiel, Buchstaben fürs Symbol |
| `size` | `[Breite, Tiefe]` in Kacheln (das Tor lässt sich drehen) |
| `hp` | Lebenspunkte |
| `armor` | Wird von jedem Zombie-Treffer abgezogen (mindestens 1 kommt durch). Kommandozentrum 10 wie TAB, weglassen = 0 |
| `shield_percent` | Schild in **Prozent der HP** (Zelt: 133 % von 150 = 199; Kommandozentrum 50 % von 5000 = TABs Barriere 2500). Zombies schlagen erst darauf. Bricht er, ist das Gebäude **infiziert** und seine Leute kommen als Zombies heraus. Ohne Kolonisten – und beim Kommandozentrum – ist er nur zusätzliches Leben. 0 = kein Schild. Im Spiel ein Balken mit den HP: der Schild graublau dahinter |
| `colonists` | Kolonisten darin: zahlen Gold und werden bei der Infektion zu Zombies. Wie viele herauskommen, wie TAB: `colonists` + `worker_cost` − `workers` + `extra_infected`, jeder zufällig aus `infected` (`zombies.json`), alle laufen zum Kommandozentrum |
| `extra_infected` | So viele Zombies kommen bei der Infektion **zusätzlich** heraus (TABs `ExtraUnitsWhenInfected`; Zelt 2 → 4 insgesamt, Steinhaus 8 → 16). Weglassen = 0 |
| `workers` | **Arbeiter**, die es gibt (Zelt, Kommandozentrum) – infiziert keine |
| `worker_cost` | Arbeiter, die es braucht, solange es steht. Nicht genug frei: kann nicht gebaut werden (im Editor egal) |
| `cost` | Preis in Gold, bezahlt beim Platzieren (im Editor kostenlos) |
| `cost_wood`, `cost_stone`, `cost_coal`, `cost_iron` | Dazu Holz, Stein, Kohle und Eisen (aus der Gießerei) aus dem Vorrat (weglassen = 0) |
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
| `noise` | **Lärm pro Arbeitstakt** (`work_every_seconds` in `sound.json`), solange es arbeitet – mal Stromanteil, aus/infiziert/ohne Holz (Köhlerei) oder ohne Erz (Gießerei) still. Jägerhütte 50, Sägewerk und Munitionsfabrik 100, Mine, Köhlerei und Gießerei 150. Weglassen = 0 |
| `smelts`, `smelt_ore`, `smelt_coal` | Gießerei: so viel **Eisen** schmilzt sie bei jeder Abrechnung höchstens (mal Stromanteil), aus je so viel Eisenerz und Kohle – nur so viel, wie im Vorrat ist |
| `weapon` | Ein Turm, der **selbst schießt** (Balliste, MG-Turm): `damage` pro Schuss (minus Rüstung), `range` (Kacheln ab der Mitte), `reload_seconds` (bei wenig Strom länger), `noise` pro Schuss, `toughest`: `true` = auf den Zombie mit den meisten HP in Reichweite, sonst auf den nächsten. Weglassen = schießt nicht |
| `gather_per_tile` | Sammler: so viel bringt jede passende Kachel pro Abrechnung (bei vollem Strom). Was passt, steht im Code: Sägewerk und Jäger Wald, Fischer Wasser, Mine Erz und Gold (auch unter sich), Farm Gras (nicht Erde, nicht Erz – und sie selbst steht nur auf Gras) |
| `gold_per_tile` | Mine: so viel **Gold** bringt jede Goldkachel pro Abrechnung, direkt ins Gold (TAB: das 20-Fache einer Steinkachel – bei uns 2) |
| `build_seconds` | **Bauzeit** wie TAB: so lange baut es sich selbst, nachdem es platziert ist – kurz, länger je größer und je höher im Techtree. Bis dahin tut es nichts (keine Arbeiter, kein Gold, kein Strom, keine Ausbildung), nimmt aber schon seine Arbeiter, und Zombies können es treffen. Weglassen oder 0 = steht sofort. Im Editor steht alles sofort |
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
| `demolish_refund_percent` | **Abreißen** (Entf): so viel Prozent des Preises kommen zurück – Gold, Holz, Stein, Kohle, Eisen, abgerundet (25 = ein kleiner Teil; 0 = wie TAB) |
| `demolish_unfinished_refund_percent` | … eines Gebäudes, das **noch gebaut wird** (falsch gesetzt? fast alles zurück) |
| `repair_seconds` | So viele Sekunden dauert eine volle Reparatur – eine kleinere entsprechend kürzer |
| `build_start_percent` | Ein Gebäude im Bau fängt mit so viel Prozent seiner HP an und wächst bis voll; den Schild hat es erst fertig. Ein Treffer im Bau bleibt (später reparieren) |
| `trap_every_seconds` | So oft treffen Fallen die Zombies auf ihnen |
| `sell_from_percent`, `sell_prices` | Märkte verkaufen nur, was über so viel Prozent des Lagers liegt, zu diesen Preisen (Gold pro Einheit: `wood`, `stone`, `iron_ore`, `coal`, `food` und `iron` – das Eisen der Gießerei) |
| `blast` | Sprengfass: `cost` Gold + `cost_coal` Kohle und `build_seconds` in der Munitionsfabrik. Geht es hoch: `damage` an allen bis `radius` Kacheln (Zombies und Einheiten minus Rüstung, Gebäude wie ein Treffer, Häuser der Dörfer wie ein Schuss), so laut wie `noise`. Wer eins trägt, läuft `carry_slow_percent` langsamer und schießt nicht |
| `scan` | Scan des Kommandozentrums: zeigt `seconds` lang einen Kreis von `radius` Kacheln (auch über Wald). Höchstens `charges` Ladungen, alle `every_seconds` eine neue – solange eine lädt, braucht das Kommandozentrum `power` Strom mehr |

## `zombies.json` – Zombies

`walker`, `runner`, `fat` (TABs „Chubby“), `armored` (TABs gepanzerter Runner, im Spiel „Riot“), `spitter` (TABs
Venom), `infested` (StarCraft 2s Infested Marine), `harpy` (TABs Harpyie: hört weit, schlägt laut zu) und `giant` (TABs Riese,
etwas kleiner – wie viele: `giants` der Karte), je mit:

| Wert | Bedeutung |
|---|---|
| `name` | Name im Spiel. In Karten- und Datendateien heißt die Art weiter wie ihr Block (`"Armored"`) – ein neuer Name ist also nur diese Zeile |
| `awareness` | **Wachsamkeit** wie TABs `ActivityAwarenessFactor`: Wie sicher er bei Lärm kommt, wächst damit (Formel bei `sound.json`). Walker 2, Runner, Fat, Riot, Infested 3, Spitter 4, Harpyie 8 |
| `speed` | Laufgeschwindigkeit (jeder Zombie bekommt zufällig ±20 %) |
| `hp` | Lebenspunkte |
| `armor` | Wird von jedem Schuss abgezogen (mindestens 1 Schaden kommt immer durch) |
| `damage` | Schaden pro Treffer – beim Spitter an jedem im Spritzer |
| `sight` | So weit sieht er Einheiten und **Gebäude mit Leuten** darin (Kommandozentrum, Häuser, Gebäude mit Arbeitern, Türme mit Einheiten – nicht durch Wald). TAB: Walker 5, Runner 6, Fat 5 |
| `watch_seconds` | So oft **schaut er sich um** (TABs `EntityWatchInterval`, geschätzt 0,5): nimmt das Nächste, das er sieht, als Ziel. Doppelt so lange, solange er schon ein Ziel hat; 3× so lange außerhalb deiner Sicht, 5× wo du nie warst |
| `level` | **Rang** (TABs `Level`, geschätzt): Stehen auf seiner Kachel mindestens `contagion_count` Wellen-Zombies mit diesem Rang oder höher, schließt er sich der Welle an. Walker 1, Runner 2, Riot und Harpyie 3, Spitter und Infested 4, Fat 5, Riese 6 |
| `stroll` | `[mindestens, höchstens]`: so weit geht er beim **Wandern** pro Ausflug – auf jeder Achse für sich, in beide Richtungen (ein Quadrat wie TAB), von da, wo er gerade steht, ohne festen Platz: mit der Zeit überall hin. Je seltener die Art, desto weiter: Walker 2–8 … Fat 4–30 |
| `roam_ring` | Beim Wandern nie in diesen Kreis ums Kommandozentrum (Kacheln) – außer er steht schon drin. TABs Riese: 54. Weglassen = 0, kein Ring |
| `roar_every_seconds` | So oft bleibt er stehen und **brüllt** `growl_seconds` lang, egal was er gerade tut (TABs Riese: 12). Wer brüllt, denkt wie TABs Riese: läuft nie dahin, wo er dich aus den Augen verloren hat, und lässt sich nie von einer Welle anstecken. Weglassen = brüllt nie |
| `radius` | Wie groß er ist (Kacheln): so weit halten andere Zombies und Einheiten Abstand, und so viel weiter reicht er. So groß ist auch der Ring unter ihm (Maus, angeklickt) |
| `mass` | Wie schwer er ist (Walker 1): im Gedränge gibt der Leichtere nach – ein Riese läuft durch Walker, sie bewegen ihn kaum. TABs Werte sind unbekannt, geschätzt |
| `range` | **Fernkampf**: so weit schießt er (Kacheln) – über Mauern, nicht durch Wald. Weglassen = 0: er muss dich berühren. Infested 4,5 = knapp unter dem Soldaten (5) |
| `splash` | Sein Schuss trifft jeden so nah am Einschlag (Spitter): Einheiten und Gebäude, keine Zombies. Weglassen = nur das Ziel |
| `attack_seconds` | Zeit zwischen zwei Angriffen. Weglassen = `hit_every_seconds` |
| `projectile_speed` | Sein Schuss fliegt so viele Kacheln pro Sekunde und landet, wo das Ziel stand – wer rechtzeitig weggeht, wird nicht getroffen (Spitter). Weglassen = trifft sofort |
| `xp` | Erfahrung, wenn er stirbt – geteilt unter deinen Einheiten in der Nähe (`veterans` in `units.json`). Weglassen = 1 |
| `jump` | Springt über Mauern und Tore bis so viele Kacheln dick, statt sie einzuschlagen – wenn sein Ziel dahinter liegt (Harpyie). Weglassen = 0, springt nie |
| `hit_noise` | So viel Lärm macht jeder seiner Treffer, wo er steht (holt Zombies, weckt Schlafende; zählt nicht zur Unruhe). Trifft er öfter, als der Lärm sich halbiert (`halve_seconds` in `sound.json`), wird es lauter (Harpyie). Weglassen = `zombie_hit` aus `sound.json` |
| `sprite` | Nur die Optik: sein Bild in `assets\`, wie bei den Einheiten. Weglassen = Kästchen |

Dazu für alle:

| Wert | Bedeutung |
|---|---|
| `hit_every_seconds` | Zeit zwischen zwei Treffern |
| `charge_steps` | So weit (in Schritten) finden stürmende Zombies den Weg zu einer Einheit – um Wasser und Berge herum, durch Mauern hindurch |
| `idle` | Was ein Zombie **ohne Ziel** als Nächstes tut, gewürfelt mit diesen Gewichten (TAB): `stand` 1300, `growl` 150, `roam` 150 – meistens steht er |
| `stand_seconds`, `growl_seconds` | So lange steht er bzw. knurrt er (TAB: so lange wie seine Animation, geschätzt 2). Der Riese brüllt so lange |
| `wander_pace` | Beim Wandern geht er mit diesem Anteil seiner Geschwindigkeit (wie weit pro Ausflug: `stroll` der Art) |
| `contagion_seconds`, `contagion_count` | **Ansteckung** (TAB): so oft schaut jeder Zombie, der nicht zu einer Welle gehört, auf seine Kachel (TAB 238 Takte, knapp 4 s – knapp neben dem Hinhören, damit nicht beides auf denselben Tick fällt). Stehen dort mindestens so viele Wellen-Zombies mit seinem `level` oder höher, läuft er mit zum Kommandozentrum – Schlafende auch |
| `infected` | Was aus einem **infizierten Gebäude** kommt und als was deine Toten aufstehen: je einer zufällig aus dieser Liste – TABs acht (WeakA/B/C → Walker, DressedA → Riot, WorkerA/B und MediumA/B → Runner) |
| `cull_below`, `cull_every_seconds` | **Sicherung** (TAB): Sind weniger als so viele Zombies übrig (wach und schlafend), stirbt alle so viele Sekunden jeder, der keinen Weg zum Kommandozentrum hat – damit die Runde nicht am letzten Zombie auf einer Insel hängt |
| `rise_min_seconds`, `rise_max_seconds` | Eine tote Einheit steht nach so vielen Sekunden als Zombie auf (zufällig dazwischen, Art aus `infected`) und läuft zum Kommandozentrum |

## `sound.json` – Lärm

**Lärm** genau wie in TAB (aus dessen Code): Er sammelt sich dort, wo etwas passiert (je 4 × 4 Kacheln), und jede
Sekunde halbiert sich jede Zelle. Wie laut ein Schuss ist: `noise` in `units.json`.

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
| `work_every_seconds` | So oft macht jedes arbeitende Gebäude seinen Lärm (`noise` in `buildings.json`) – alle im selben Moment, Nachbarn zählen zusammen |
| `unrest_per_zombie` | **Unruhe:** aller Lärm der Kolonie (Schüsse, Maschinen, Explosionen, Bauen) zählt zusammen und sinkt nie – je so viel davon bringt jede Seite eines stillen Events (TABs Streuner) für immer einen Zombie mehr |

## `roaming.json` – Streuner

Die Zombies, die zu Beginn über die Karte streifen (wie viele: `roaming_zombies` der Schwierigkeit). Entfernungen als
Anteil des Wegs von der Basis zum Rand (0 = Mitte, 1 = Rand). Wie weit sie pro Ausflug wandern: `stroll` der Art in
`zombies.json`, wie nah ans Kommandozentrum sie zu Beginn liegen: `keep_out` der Schwierigkeit (beim Wandern dürfen sie
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
| `hp`, `armor` | Lebenspunkte eines Hauses, Rüstung gegen jeden Schuss |
| `max_units` | So viele lässt ein Haus insgesamt heraus, je Größe (`small`, `medium`, `big`), mal `population` (TABs `InfectionNestMaxUnits`, geschätzt) |
| `batch_every_seconds` | Höchstens ein Schub so oft pro Haus |
| `hearing`, `awareness`, `heard_batch` | Hört das Haus **Lärm** so nah – so sicher wie ein Zombie mit dieser `awareness` (Formel bei `sound.json`) –, geht ein Schub × `heard_batch` zur Mitte der gehörten Zelle |
| `guard_every_hours`, `guards_per_size`, `guard_batch` | So oft (jedes Haus zu seiner Stunde): stehen weniger als `guards_per_size` × Hausgröße Zombies ums Haus (es selbst zählt mit), bleibt ein Schub × `guard_batch` dort – die **Wachen** füllen sich auf |
| `raids_from_day`, `raid_every_days`, `raid_batch` | Ab diesem Tag alle so viele Tage (jedes Haus an seinem Tag): ein Schub × `raid_batch` **marschiert zum Kommandozentrum** wie eine Welle |
| `hit_batch`, `hit_reach`, `neighbour_batch` | **Getroffen:** ein Schub × `hit_batch` geht auf die nächste Einheit in so vielen Kacheln (keine da: bleibt am Haus); jedes Nachbarhaus (Fläche × 3 um die Mitte + 4 Kacheln) schickt einen Schub × `neighbour_batch` mit |
| `final_batch` | Ruft die Schlusswelle: ein Schub × so viel, sooft es darf, bis es leer ist – mit der Schlusswelle |
| `breeds` | Welche Arten aus einem Haus je Größe kommen und wie wahrscheinlich (`breed`, `weight` – TABs Gewichte: klein Walker, mittel Runner, groß Runner 400 / Riot 100 / Fat 15) |
| `loot_gold`, `loot_goods` | Ein abgeschossenes Haus lässt Beute fallen: so viel Gold – oder so viel Holz, Stein, Eisenerz oder Kohle. Was es noch nicht herausgelassen hat, kommt nie |

## `loot.json` – Beute

Die **Beute**, die herumliegt – wie viel: Karteneinstellung „Loot“. Wie viele Wächter: `loot_guards` der
Schwierigkeit.

| Wert | Bedeutung |
|---|---|
| `wake_radius`, `hearing`, `awareness` | Kommt eine Einheit so nah – oder hören sie Lärm so nah, so sicher wie ein Zombie mit dieser `awareness` –, kommen die versteckten Wächter auf einmal aus dem Boden (und gehen auf die Einheit bzw. zum Lärm) |
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

## `spawning.json` – TABs Survival-Plan

Die **Events** einer neuen Runde – TABs eigener Plan (aus dessen Code), für **100 Tage und Bevölkerung 1**. Eine neue
Karte bekommt sie als Kopie in ihre Kartendatei, danach ändert man sie im **Editor** (Tab „Events“) oder in der
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
  das letzte Kommandozentrum fällt.

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

Die Karten wie in TAB. Vorerst ist eine Karte einer der Generatoren mit seinen Werten – sie sieht nicht anders aus.
`default`: die Karte beim allerersten Start. In `maps` eine pro Block:

| Wert | Bedeutung |
|---|---|
| `name`, `about`, `score_percent` | Name, eine Zeile Erklärung, Score-Faktor |
| `setup` | Wie sie gemacht wird – alles, was die Seite „Custom setup“ hat: `generator` (`tab`, `wild`, `regions`, `rings`, `canyons`, `blank`), `common` (z. B. `size`, `ways_in`, `loot_piles` und die Dörfer: `villages` pro 10.000 Kacheln, `houses_near` / `houses_edge` Häuser pro Dorf – Open Plains viele kleine, Broken Lands zwei riesige – und `giants`, die Riesen auf einer Karte von TABs Größe 256: auf größeren mehr, auf kleineren weniger, mindestens einer; weit draußen, die Schlusswelle ruft auch sie) und die Werte des Generators (z. B. `"blank": { "trees": 0.02 }`). Was fehlt, bleibt beim Standard. Die Namen der Werte stehen in einer Kartendatei unter `setup` |

**TABs Generator** (`"generator": "tab"`, die Landschaften „TAB BR“ … „TAB VO“): seine Werte unter `"tab"`, TABs Namen
in Klammern – alle **geschätzt**, weil TABs `ZXRules.dat` zu bleibt. Von `common` nimmt er nur `size`,
`spawners_per_side` und `ore_richness`; Anzahlen (Dörfer, Ruinen, Riesen) gelten für TABs 256er-Karte und wachsen
mit der Fläche.

| Wert | Bedeutung |
|---|---|
| `land`, `land_dispersion`, `land_octaves` | Anteil Land, der Rest Seen (`LandWaterFactor`); 0 = große, 1 = kleine Seen (`LandDispersionFactor`); Feinheit der Ufer |
| `mountains` | Berge auf dem höchsten Land, in 2 × 2-Blöcken (`MountainLandFactor`) |
| `grass`, `forest_on_grass`, `forest_on_dirt`, `vegetation_octaves` | Gras auf dem Land, sonst Erde (`GrassEarthFactor`); Wald auf Gras (mal `grass`) und auf Erde (`ForestGrassFactor`, `ForestEarthFactor`) |
| `villages_min`, `villages_max`, `village_size_min`, `village_size_max` | So viele Villages of Doom (`NumDoomVillages`), so groß (Radius, `DoomVillagesSize` – über 8 ein großes Haus in der Mitte, über 12 jedes 8.) |
| `ruins_min`, `ruins_max` | Ruinen mit 2–4 Haufen Beute nahe der Basis (`NumTreasures`) |
| `stone`, `iron`, `coal`, `gold` / `stone_from`, `iron_from`, `coal_from`, `gold_from` | Wie viel davon auf freier Erde (`…EarthFactor`) / erst so weit vom Kommandozentrum (`…FormationMinDistance`). Kohle ist unsere (TAB hat keine): wie Eisen, nie auf Stein oder Eisen, mindestens 10 Kacheln in 25 um die Basis – sonst der beste von 100 Würfen, nie ein neuer Seed |
| `oil_block` | Eine Ölquelle pro Block dieser Kantenlänge (`NCellsPerOilSource`) |
| `infected`, `infected_dispersion` | Wie viel des Landes Zombies hat, mal `population` der Schwierigkeit (`InfectedFactor`); wie fein die Flecken (`InfectedDispersion`) |
| `weak_from`, `medium_from`, `strong_from`, `powerful_from` | Ab so weit draußen: Walker / Runner und Riots / Riots allein / Fat, Harpyie, Spitter (`MinDistanceFor…Infected`) |
| `giants` | Riesen bei Bevölkerung 1 (`NumInfectedGiants`) – mindestens 70 Kacheln draußen, 40 auseinander |
| `nests` | Wie groß die Schübe der Häuser der Villages of Doom sind (`InfectionNestsFactor`, `doom.json`) – geschätzt 1 |
