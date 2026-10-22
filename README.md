# Last Bastion

- **Echtzeit-Strategie ums Überleben:** Einheiten steuern wie in **StarCraft**, Horden und Basisbau wie in **They Are
  Billions (TAB)**, Produktionsketten und Strom wie in **Factorio**.
- **Runden von 2–4 Stunden** – wie in TAB wählt man die Anzahl der Tage. Isometrisch wie TAB.
- **Kern:** die TAB-Seite – die Horde gegen eine Basis im Raster.
- **Stand:** spielbare Mini-Runde + Karten-Editor – was alles drin ist: [Prototyp ausprobieren](#prototyp-ausprobieren).
  Das ganze Design steht in [DESIGN.md](DESIGN.md) (Englisch).

---

## Regeln für Claude (bitte zuerst lesen)

`CLAUDE.md` und Claudes Memory liegen nur lokal – deshalb stehen die Regeln hier:

- **Ich (Maximilian) gebe die Idee, Claude schreibt den Code.** Das gilt auch für `DESIGN.md`, diese README und
  Build-/Einrichtungs-Skripte.
- **Nicht einfach nur coden:** Zu jeder Änderung sagt Claude, **was** geändert wurde und **warum** (welche Dateien,
  welcher Ansatz, welche Alternative verworfen wurde). Ich will Rust und das Spiel dabei verstehen, nicht nur Code
  bekommen – Rust-Besonderheiten, die neu sind (Ownership, Borrowing, Traits …), kurz erklären.
- **Neuen Code Stück für Stück erklären** (so wie beim ersten Fenster): jeden Abschnitt zeigen und
  erklären, was er tut und was die Syntax bedeutet – mit Vergleich zu Python/pygame, wo es passt. Neue Zeichen und
  Begriffe am Ende in einem kleinen Spickzettel (Rust ↔ Python). Was schon erklärt wurde, nur kurz erwähnen statt
  jedes Mal neu – außer es wird anders benutzt. Auch Hilfsdateien (`Cargo.toml`, Skripte) kurz erklären und zum
  passenden Kapitel im Rust-Buch verweisen.
- **Am Ende eine kurze Zusammenfassung** in ein paar Sätzen im Chat. Ändert sich, was der Prototyp kann, wird
  [Prototyp ausprobieren](#prototyp-ausprobieren) an der passenden Stelle angepasst – **kurz, in Stichpunkten**.
  **Keine Verlaufs-/Stand-Einträge** mit Datum: der Verlauf steht in git.
- Passt eine Idee nicht zum Design (`DESIGN.md`) oder hat sie einen Haken, sagt Claude das, bevor es loslegt.

---

## Dateien

| Datei | Was |
|---|---|
| `DESIGN.md` | Design-Dokument: Säulen, Runde, Wirtschaft, Einheiten, Zombies, Karte, Technik, Fahrplan, offene Fragen |
| `Cargo.toml` / `Cargo.lock` | Rust-Projekt und Bibliotheken (`macroquad`, `serde`/`serde_json`, `libm`, `rayon`) / ihre genauen Versionen |
| `build.ps1` | Baut beide Programme nach `dist\LastBastion.exe` und `dist\LastBastionEditor.exe` |
| `data\` | **Alle Spielwerte** als JSON (Einheiten, Gebäude, Zombies, Lärm, Streuner, Villages of Doom, Beute, Kolonie, Spawn, Schwierigkeiten, Spieldauer, Karten) – erklärt in `data\README.md` |
| `assets\` | **Bilder** (Sprite-Sheets) der Zombies und Einheiten – erklärt in `assets\README.md` |
| `examples\test_sprites.rs` | Malt das graue Testbild `assets\test\figure.png` (`cargo run --release --example test_sprites`) |
| `src\main.rs`, `src\bin\editor.rs` | Die zwei Programme (je ein paar Zeilen) |
| `src\lib.rs`, `src\app.rs` | Liste aller Module / Fenster, Bildschirm-Wechsel, Kommandozeile |
| `src\game.rs` | Eine Karte auf dem Bildschirm: Takte, Zeichnen, Endbildschirm, Edit/Test im Editor |
| `src\sim\` | **Die Simulation** (deterministisch, zeichnet nichts): Befehle, Spawn, Zombies, Fernkampf-Zombies (`ranged.rs`), Villages of Doom und Beute (`doom.rs`), Einheiten, Kampf, Gebäude, Wirtschaft (`economy.rs`), Markt (`market.rs`), Schulden und Bank (`debt.rs`), Sprengfässer (`blast.rs`), Scan (`scan.rs`), Runde, Tests |
| `src\placement.rs` | Alle Bauregeln an einem Ort |
| `src\data.rs`, `src\kinds.rs` | `data\` lesen / Einheiten- und Gebäudetypen |
| `src\aggro.rs`, `hunt.rs`, `flow.rs` | Wegfindung der Zombies und Einheiten (Bedrohungs-, Jagdkarte, Flow Field mit Sichtlinie) |
| `src\formation.rs`, `vision.rs`, `grid.rs` | Formationen / Sicht und Nebel / räumliches Raster |
| `src\map.rs` | Die Karte: Gelände, Erz/Öl, Gebäude, Weltgrenze, Sichtlinie |
| `src\mapgen\` | Kartengeneratoren (Wild, Regions, Rings, Canyons, Blank), Wege hinein, Erz und Öl, Dörfer und Beute (`villages.rs`), Werkzeuge (auch Erde) |
| `src\mapfile.rs` | Eine Runde = eine Datei (`%APPDATA%\Last Bastion\maps\`) |
| `src\home.rs`, `menu.rs`, `widgets.rs`, `settings.rs` | Startbildschirm und Karten-Einstellung / Esc-Menü / Knöpfe und Regler / Einstellungen |
| `src\ui.rs`, `cursor.rs` | Maus und Tastatur → Auswahl, Gruppen, Kamera, Befehle / echte Windows-Maus |
| `src\hud.rs` | Die Leiste unten wie in TAB (Minimap, Uhr, Gruppen, Auswahl, Befehlskarte, Vorräte) |
| `src\editor.rs` | Das Panel des Editors |
| `src\draw.rs`, `sprites.rs` | Zeichnen (Bilder, sonst Platzhalter) / Sprite-Sheets laden, Richtung und Animation wählen |
| `src\picture.rs`, `iso.rs` | Kartenbild für Minimap und Vorschau / isometrische Kamera |
| `src\noise.rs`, `rng.rs` | Rauschen für die Generatoren / eigener Zufallsgenerator |

## Anbindung an die Windows-Toolbox

- Das Spiel ist ein eigenes Repo; die [Windows-Toolbox](https://github.com/ItsMaxxx/windows-toolbox) hat nur eine
  Seite im Tab „Games“, die es startet (DESIGN.md 12.7).
- Dafür gilt: das Spiel liegt immer unter **`dist\LastBastion.exe`**, das Design heißt `DESIGN.md`, die Einstellungen
  liegen fest unter `%APPDATA%\Last Bastion` (DESIGN.md 11.4).

## Rust lernen

- **Einrichten:** `winget install Rustlang.Rustup` (Toolchain MSVC) + Visual Studio mit „Desktopentwicklung mit C++“
  (Linker und Windows SDK). Im Editor **rust-analyzer**.
- **Starten:** `cargo run` (Spiel) / `cargo run --bin editor` (Editor) – Debug-Build mit Konsole.
  `cargo build` baut beide (dann findet der Knopf „Map editor“ den Editor).
- **Fertig bauen:** `.\build.ps1` → `dist\` (Release, ohne Konsole).
- **Tests:** `cargo test` – läuft nur, wenn man es aufruft (nicht im Spiel, nicht in `build.ps1`), dauert ≈ 2 s.
- **Lesen:** [Rust-Buch](https://doc.rust-lang.org/book/), [auf Deutsch](https://rust-lang-de.github.io/rustbook-de/)
  – wichtig: Kap. 1–10, später 13 und 15. Zum Üben [Rustlings](https://github.com/rust-lang/rustlings).

## Prototyp ausprobieren

### Die Mini-Runde (`dist\LastBastion.exe`)

- **Start:** 4 Ranger, 1 Soldat, ein Kommandozentrum, 100 Gold, 20 Holz – sonst nichts (Schwierigkeit Normal, jede
  anders: `data\game_difficulties.json`).
- **Länge:** 10 / 20 / 40 / 60 / 80 Tage wählbar (1 Tag = 3 Minuten, 10 Tage ≈ 30 Minuten, `data\game_durations.json`).
- **Ziel:** Wirtschaft aufbauen (Holz, Nahrung, Strom, Stein), Zelte (Gold und Arbeiter), Kaserne (nur sie bildet aus),
  Einheiten, Mauern, Türme.
- **Gewonnen**, wenn die **Schlusswelle** am letzten Tag tot ist; **verloren**, wenn das Kommandozentrum fällt →
  Endbildschirm (Restart, „Look at the map“, Main menu).
- **Eine Runde = eine Datei** (wie TAB) unter `%APPDATA%\Last Bastion\maps\`. **Restart** liest sie neu (Änderungen aus
  dem Editor gelten sofort). Speichern mitten in der Runde kommt später (DESIGN.md 11.5).

### Startbildschirm und Karten-Einstellung

- New game, Map editor, **Custom maps** (Play / Edit / Delete), Settings, Quit.
- **New game** (wie TAB): **Karte**, **Schwierigkeit** und **Dauer** wählen, Seed, Vorschau – dazu der
  **Score-Faktor** (Karte × Schwierigkeit × Dauer; eine Punktzahl gibt es noch nicht).
  - **Karten** (`data\game_maps.json`): Green Valley (Wild), Broken Lands (Regions), Old Fortress (Rings), Red Canyons
    (Canyons), Open Plains (Blank mit einzelnen Bäumen) – sie sehen (noch) nur nach ihrem Generator aus.
  - **Schwierigkeit** (Easy / Normal / Hard / Brutal, `data\game_difficulties.json`): Startgold, -holz und -einheiten,
    Streuner, wie nah jede Art ans Kommandozentrum darf, Zombies pro Dorfhaus, Wächter der Beute, Stärke der Wellen
    und des konstanten Spawns.
  - **Dauer** (`data\game_durations.json`): 10–80 Tage, jede mit eigenem Wellen-Takt.
- **Custom setup…** (im Editor: „New map“): Generator, Regler (Maus drauf = Erklärung), Schwierigkeit und Dauer.
  „Random values“ würfelt, „Defaults“ setzt zurück. Die letzten Werte merkt sich das Spiel.

| Generator | Wie |
|---|---|
| **Wild** | Rauschen wie Factorio: Seen, Bergrücken, Wälder, Flüsse |
| **Regions** | Wie TAB: Regionen mit Wänden und **Pässen** dazwischen |
| **Rings** | Festungsringe mit versetzten Öffnungen |
| **Canyons** | Fels mit gewundenen Schluchten |
| **Blank** | Nur Gras – zum Selberbauen |

- **Für alle Generatoren:** Größe (96–256), freier Platz um die Basis, Spawner pro Seite, Wege hinein, Villages of
  Doom (wie viele, Häuser nahe der Basis / am Rand), Beute, Erde, Erzfelder, Ölgruppen.
- **Erde** (braun): begehbar und bebaubar wie Gras, aber **keine Farm** – in Flecken, 40 % des offenen Bodens (Regler
  „Dirt“, nicht auf Blank).
- **Wege hinein:** gewundene offene Wege von jeder Seite zur Basis, die sich wie Äste vereinigen – die Stellen zum
  Zumauern. Flüsse bekommen dort Furten, Regionswände Pässe. Ein Sicherheitsnetz verbindet alles Abgeschnittene.
- **Weltgrenze:** gerade auf dem Bildschirm wie in TAB – die Spielfläche ist ein Rechteck (doppelt so breit wie
  hoch), die Minimap auch. Außen ein Ring offenen Geländes (≈ 11 Kacheln) mit den **Spawnern** (3 pro Seite, fast die
  ganze Seite breit). Dahinter sieht und läuft man nicht. Größe = so viele Kacheln wie ein Quadrat dieser Kante.
- **Erz:** Eisenerz (rostbraun), Kohle (schwarz), Stein (hellgrau) als Felder; neben der Basis je ein kleines (16–26
  Kacheln weg – oft braucht es einen Tesla-Turm bis dorthin), draußen größer und reicher. Abgebaut von der **Mine** –
  am besten mitten aufs Feld, sie baut auch das Erz unter sich ab.
- **Öl:** Quellen von genau **2 × 2 Kacheln** (lila mit schwarzer Lache), berühren sich nie. Neben der Basis eine
  einzelne, draußen Gruppen von 2 bis 6.

### Zombies und Wellen

- **Wellen** (`data\spawning.json`, der Takt aus der Dauer): bei 10 Tagen ab Tag 4 alle 3 Tage (auf Normal 200 → 600
  Runner pro Seite). Die ersten nur Runner, danach
  immer mehr Riots, Infested, Spitter und Fat – aus denselben Spawnern wie die Runner. Am letzten Tag 6:00 die
  **Schlusswelle** von allen 4 Seiten mit allen Arten: Sie **ruft alle Zombies der Karte** (auch die in Dörfern und um
  Beute – sie gehören dann dazu) und **deckt die Karte auf**. Getötete Zombies kommen nie wieder.
- **Konstanter Spawn** (`data\spawning.json`): ab Tag 2 kleine Gruppen Walker, immer öfter; ab Tag 10 Runner statt
  Walker; ab Tag 15 / 20 / 25 / 30 kommen Riots / Infested / Spitter / Fat mit (15 / 10 / 8 / 5 % jeder Gruppe). Jede
  Gruppe kommt **dicht beieinander an einem Punkt** eines Spawners, nicht über die ganze Seite verstreut.
- **Wie viele (wie TAB):** fast überall, in **Grüppchen von 1–3 auf einer Kachel – nie mehr als 3** (`pack` in
  `data\roaming.json`). `roaming_zombies` der Schwierigkeit: pro 1.000 Kacheln, auf denen sie stehen können (Easy 900,
  Normal 1.500, Brutal 2.000 = eine Gruppe auf fast jeder Kachel). Auf 256 und Normal ≈ 65.000, dazu die in Dörfern
  und um Beute.
- **Die Balance machen die Arten, nicht die Menge:** nahe der Basis ≈ 1 pro Kachel, nur Walker; am Rand ≈ 1,6, davon
  über die Hälfte Runner, Riots, Infested, Spitter, Fat.
- **Wie nah jede Art ans Kommandozentrum darf** (`keep_out` der Schwierigkeit, Normal: Walker und Runner 10, Riot und
  Infested 25, Spitter 30, Fat 35 Kacheln; Brutal: alle 10) – beim Verteilen und beim Bummeln, auch nicht auf dem
  Weg zum Ziel. Angelockte und Wellen dürfen näher – Angelockte bummeln von dort zufällig weiter, pro Ausflug
  höchstens 3 Kacheln tiefer (`inside_step`), im Ganzen wieder hinaus.
- **Arten:** **Walker** (langsam), **Runner** (schnell), **Fat** (groß, 600 HP), **Riot** (TABs gepanzerter Runner:
  schnell, Rüstung 3 – ein Pfeil macht nur 7), **Spitter** (TABs Venom: spuckt Säure 3,5 Kacheln weit, über Mauern;
  sie fliegt kurz und trifft alle im Umkreis von 1 Kachel – wer rechtzeitig weggeht, entkommt), **Infested** (SC2s
  Infested Marine: schießt 4,5 Kacheln, knapp unter dem Soldaten). Weiter draußen mehr von den harten Arten.
- **Schlafen weit weg** (DESIGN.md 8.5): Wer weit weg von deinen Augen liegt, schläft – kostet fast nichts. Wach werden
  sie, wenn eine Einheit oder ein Gebäude auf Sichtweite + 8 Kacheln herankommt (bevor du sie siehst), durch Lärm
  (genau wie die wachen – und gehen hin), durch Scan, Sprengfass und die Schlusswelle (alle, 2.000 pro Takt, die
  nächsten zuerst). Wer 20 s ruhig und weit weg von allen Augen ist, schläft wieder ein.
- **Auch Schlafende bummeln** – Ausflug für Ausflug wie die wachen (nur kommen sie sofort an und warten dann, so lange
  der Weg gedauert hätte). Ein Ausflug, der nahe deiner Augen endet, weckt sie unterwegs: Sie laufen wach von außen
  herein. **Ein leergeräumter Ort füllt sich so wieder auf** – die Zombies gehen dir nicht aus.
- **Einblenden:** Zombies kommen sanft ins Bild (0,6 s) und verschwinden sanft (0,8 s) – wie TAB, kein Aufploppen am
  Sichtrand. Im Editor („All“, „Zombies“) sieht man auch die Schlafenden, dunkler.
- **Streuner bummeln ohne festen Platz:** jeder Ausflug bis zu Walker 8 Kacheln, Runner 14, Riot 18, Infested 22,
  Spitter 24, Fat 30 von da, wo er gerade steht – mit der Zeit also überall hin, auch nahe der Basis (nie
  näher, als seine Art darf).
- **Villages of Doom** (wie TAB, weit weg von der Basis): dunkle Häuser mit rotem Dach. **Wie viele Dörfer und wie
  groß, sagt die Karte** (Green Valley ≈ 10 mit 3–6 Häusern, Open Plains ≈ 26 kleine mit 2–3, Broken Lands **2
  riesige** mit 15–25), die Schwierigkeit macht sie mehr oder weniger. In **jedem Haus gleich viele** schlafende
  Zombies (`doom_zombies`: Easy 150, Normal 250, Hard 350, Brutal 500). Kommt eine Einheit auf 7 Kacheln heran,
  kommen nur **ein paar** heraus (3, alle 10 s). Was sie wirklich weckt, ist **Lärm** in 24 Kacheln: dann strömen sie
  heraus (15 pro Sekunde, das Dach leuchtet rot) und laufen zum Lärm. Jeder Treffer lässt einen **zusätzlich** heraus.
  Haus abschießen (1.500 HP): der Rest kommt auf einmal, es lässt Beute fallen (1.000 Gold oder 20
  Holz/Stein/Eisenerz/Kohle).
- **Beute** (≈ 40 Haufen): Gold (500) oder 10 Holz/Stein/Eisenerz/Kohle – drauflaufen hebt sie auf. Um jeden Haufen
  liegen Zombies versteckt (Normal 8–50, mehr weiter draußen), die auf 5 Kacheln oder bei Lärm aus dem Boden kommen.
- Häuser und Beute bleiben auf der Karte sichtbar, sobald man sie einmal gesehen hat, auch auf der Minimap (rot).
- **Sehen** nur 5–6 Kacheln (nicht durch Wald), **hören** Einheiten 3 Kacheln weit, angeschossen stürmen sie 5 s los.
- **Lärm wie in TAB** (`data\sound.json`): Er sammelt sich dort, wo etwas passiert – je Schuss Ranger 1,
  Soldat 3, Scharfschütze 10; jeder Zombie-Treffer 1, ein platziertes Gebäude 40, eine sterbende Einheit 50, ein
  zerstörtes Gebäude 100, ein **infiziertes** 100 + 50 je Zombie, der herauskommt. Ein langer Kampf wird also immer
  lauter, eine Gruppe dicht beieinander lauter als verteilt. Nach 1 s Ruhe halbiert er sich jede Sekunde.
- **Maschinen sind laut** (wie Factorios Verschmutzung): alle 10 s macht jedes arbeitende Gebäude seinen Lärm, alle
  im selben Moment – Jägerhütte 50, Sägewerk und Munitionsfabrik 100, Mine und Köhlerei 150 (mal Stromanteil; aus,
  infiziert, Köhlerei ohne Holz: still). **Eins allein lockt niemanden**; Mine + Köhlerei im selben 4 × 4-Feld (300)
  weckt ein Dorf in 24 Kacheln, ein Industriegebiet lockt Runner von 20 Kacheln weit.
- **Unruhe:** aller Lärm der Kolonie (Schüsse, Maschinen, Explosionen, Bauen – nicht der der Zombies) zählt zusammen
  und **sinkt nie**: je 50.000 bringt jede Gruppe des konstanten Spawns **für immer** einen Zombie mehr. Die Fabrik
  vor der Schlusswelle abzureißen, nimmt das nicht zurück. F3 zeigt sie.
- **Wer kommt:** Zombies hören 4 × ihre Sichtweite weit (Walker 20 Kacheln) und kommen ab 1000 / Wachsamkeit (Walker
  500, Runner 333, Spitter 250) – per Zufall, je lauter, desto sicherer. Mit F3 sieht man den lautesten Ort.
- **Gedächtnis:** Ein Zombie geht dorthin, wo der Lärm war oder wo er zuletzt eine Einheit gesehen hat, und bleibt
  dort, wenn er nichts findet. Nur Wellen-Zombies laufen danach weiter zum Kommandozentrum. Angeklickt zeigt ein Zombie
  mit einer roten Linie, wohin er will. Auf dem Weg laufen sie um Wasser und Berge, aber **durch Mauern** (einreißen);
  ein paar Angeschossene suchen die Lücke.
- Tote Einheiten stehen als Walker auf. Ein Zelt mit gebrochenem Schild ist **infiziert** und gehört den Zombies –
  Zombies und Einheiten laufen hindurch. **Reparieren** heilt es. Fällt ein Gebäude mit Arbeitern (Jägerhütte,
  Sägewerk …), kommen die Arbeiter als Zombies heraus – laut! Weit draußen gebaute Hütten also gut schützen.

### Einheiten und Kampf

- **Soldat, Ranger, Scharfschütze, Medic** – alle Werte in `data\` (siehe `data\README.md`).
- **Söldner** aus dem Gasthaus (Söldner-Soldat, -Ranger, -Scharfschütze): stärker als Veteranen (×1,5 HP, ×1,4
  Schaden, schneller), 2,5 × so teuer, doppelter Unterhalt, **keine Arbeiter** – das Limit ist nur das Gold. Goldene
  Marke über dem Kopf. Bei Schulden gehen sie zuerst, schon ab Stufe 2.
- **Sprengfass tragen:** Rechtsklick auf die Munitionsfabrik – jede Einheit nimmt eins (30 % langsamer, schießt
  nicht), **D** stellt es ab. Stirbt der Träger, geht es hoch.
- **Größe:** eine Einheit braucht 0,35 Kacheln (Walker/Runner 0,25, Fat 0,45) – so groß wie die Figuren auf den
  Türmen. Formationen stehen 0,5 Kacheln auseinander.
- **Schießen wie StarCraft:** drehen → zielen → schießen → nachladen. Eine Einheit bleibt bei ihrem Ziel.
  Schüsse bis zum Tod: Walker Ranger 4 / Soldat 3 / Scharfschütze 1, Runner 6 / 4 / 1.
- **Feuer verteilen:** neue Ziele sind Zombies, die die anderen nicht schon sicher töten.
- **Medic:** greift nie an, richtet seinen Strahl auf Verletzte (Heilen kommt noch).
- **Formationen** (wie *Diplomacy is Not an Option*): nur beim **Ziehen** – Linie, Block, Kreis.

### Die Leiste unten (wie TAB)

- **Minimap** (klicken/ziehen = Kamera, Rechtsklick = hinlaufen).
- **Uhr:** Tag, Stunde, **Pause-Knopf**, Tempo, nächste Welle (Schlusswelle rot).
- **Kontrollgruppen** 1–0 untereinander.
- **Auswahl:** eine Sache = Bild, Name, Lebensbalken (HP + Schild in einem), Text, Werte in Kästchen; mehrere = Symbole.
- **Befehlskarte** 5 × 3 mit Tooltips, Preis rot, wenn das Gold nicht reicht.
- **Gold im Minus:** gelb („-250 of 500“), solange es im Dispo der Banken liegt, rot darüber.
- **Vorräte:** Kolonisten, **Arbeiter** (frei/alle), **Nahrung**, **Energie** (gebraucht/erzeugt), Gold, Holz, Stein,
  Eisenerz, Kohle – jeweils mit „+/− pro Abrechnung“ (alle 8 Stunden); rot, wenn etwas knapp wird.

### Steuerung (wie StarCraft 2)

| Taste | Was |
|---|---|
| Linke Maus (klicken / ziehen) | Auswählen (Rahmen nimmt Gebäude nur ohne Einheiten). Auf nichts = Auswahl weg; Shift: dazu/weg. Zombie, Haus eines Dorfs, Beute: nur ansehen |
| Strg + Klick, Doppelklick | Alle dieses Typs auf dem Bildschirm |
| Klick auf ein Symbol unten | Nur dieses (Shift: weg, Strg: nur dieser Typ) |
| Tab / Shift+Tab | Nächster / voriger Typ in der Auswahl (auch Gebäude) – die Befehlskarte zeigt ihn |
| F2 | Alle Einheiten |
| Rechte Maus | Laufen / Zombie angreifen / Einheit folgen / in den Turm / Fass aus der Munitionsfabrik holen / Haus eines Dorfs angreifen / auf Beute laufen. Gebäude: Sammelpunkt |
| Rechte Maus **ziehen** | Formation (X Linie, C Block, V Kreis) |
| T / P / M + Linksklick | Angriff (Attack-Move) / Patrouille / Laufen – ziehen = in Formation |
| S / H | Stopp / Halten (läuft nicht von selbst, lässt sich aber schieben) |
| F / X | Ausgewählte Gebäude reparieren / alle reparieren (X nur ohne ausgewählte Einheiten) |
| **Shift** + Befehl | Hinten an die Befehlskette hängen. Shift loslassen = Befehl weg von der Maus, wieder drücken = zurück |
| **Shift** + bauen / ausbilden | Fehlt nur Gold: ins Minus, in den Dispo der Banken (ohne Shift ist 0 die Grenze) |
| **Strg** + ausbilden | 5 auf einmal (auch Fässer) – anders als SC2, dort ist es Shift |
| D | Einheiten mit Fass: abstellen |
| Strg / Shift / Strg+Shift + 1–0 | Gruppe setzen / dazu / „stehlen“ – auch mit Gebäuden. 1–0: auswählen, zweimal: Kamera hin |
| WASD, Pfeile, Rand, Mausrad | Kamera, Zoom. **WASD nur ohne Auswahl** (sonst gehören die Tasten der Befehlskarte) |
| Mittlere Maus ziehen | Karte verschieben |
| Strg + F5–F8 / F5–F8 | Kamera merken / hinspringen. Rücktaste: zur Basis |
| Leertaste | Pause (Befehle gehen weiter) · `+` / `-`: Tempo ×1–×8 |
| Esc | Bricht ab, was wartet → hebt die Auswahl auf → Spielmenü (Settings, Restart, New map, Main menu) |
| F3 / F12 | Messwerte, Schalter, Tastenhilfe (Abschnitte per Klick ein-/ausklappen) / Maus im Fenster halten an/aus |

### Bauen

- **Kommandozentrum:** ein **Baumenü pro Taste** (wie TABs Kategorien). Fett = gibt es schon, der Rest ist grau mit
  „soon“ (Tooltip sagt, was es tun soll). **B** oder **Esc** zurück – oder das Kommandozentrum neu auswählen (Klick, Gruppe).
  Dazu **C Scan** (unten).

| Menü | Q | W | E | R | T | A | S |
|---|---|---|---|---|---|---|---|
| **Q** Kolonisten | **Zelt** | **Steinhaus** | | | | | |
| **W** Ressourcen | **Jägerhütte** | **Fischerhütte** | **Sägewerk** | **Mine** | Ölpumpe | **Farm** | |
| **E** Energie | **Tesla-Turm** | **Windmühle** | **Köhlerei** | Ölgenerator | | | |
| **A** Industrie und Handel | **Holzwerkstatt** | **Steinwerkstatt** | Gießerei | **Gasthaus** | **Markt** | **Bank** | |
| **S** Militär | **Kaserne** | **Munitionsfabrik** | Belagerungsfabrik | | | | |
| **D** Verteidigung | **Mauer** | **Tor** | **Turm** | **Pfahlfalle** (R) | **Sprengfass** (T) | **Steinmauer** (A) | **Steinturm** (D) |
| **Y** Wunder | – | | | | | | |

Balliste (F) und MG-Turm (G) sind noch grau.

- **Kaserne** (ohne Voraussetzung baubar): Q Soldat (100 Gold), W Ranger (60), R Scharfschütze (200), T Medic (120),
  G Sammelpunkt. **Soldat und Scharfschütze** (Schusswaffen) erst mit einer **Munitionsfabrik**. Strg = 5 auf
  einmal, **Taste halten = immer wieder** (wie SC2), höchstens 5 in der Schlange, Klick auf die Schlange bricht ab
  (Gold und Arbeiter zurück).
- **Gasthaus** (Steinwerkstatt, 800 Gold): Q Söldner-Soldat (250), W Söldner-Ranger (150), R Söldner-Scharfschütze
  (500) – dieselben Tasten wie in der Kaserne, Schusswaffen auch hier erst mit Munitionsfabrik.
- **Sprengfass:** die Munitionsfabrik macht es (Q, 40 Gold + 2 Kohle, 8 s), ihr Lager fasst 6. Im Verteidigungsmenü
  (T) setzen – nimmt eins aus dem Lager, ziehen = Reihe – oder von Einheiten wegtragen lassen. **Jeder Schaden** und es
  geht hoch: 200 Schaden in 2,5 Kacheln für alle (Zombies, eigene Einheiten, Gebäude, Häuser der Dörfer), Fässer
  daneben gehen mit, Lärm 400.
- **Scan** (C am Kommandozentrum, wie SC2): Klick auf die Karte → 10 s ein Kreis von 10 Kacheln sichtbar, auch über
  Wald, ohne Lärm. 2 Ladungen, alle 120 s eine neue – solange eine lädt, nimmt das Kommandozentrum seinem Netz
  **30 Strom** (früh fast alles).
- **Preise** (Gold + Holz/Stein, alles in `data\buildings.json`): Zelt 30, Kaserne 450 + 20 Holz + 20 Stein,
  Holzwerkstatt 1000 + 40 Holz, Mine 300 + 30 Holz, Sägewerk 300, Mauer 10 + 3 Holz, Tor 50 + 10 Holz, Turm 120 + 10 Holz
  (fast alles wie TAB). Gebäude stehen sofort (noch
  keine Bauzeit). Der Tooltip zeigt den ganzen Preis.
- **Arbeiter** (wie TAB): Zelt gibt 2, Steinhaus 8, Kommandozentrum 4. Kaserne braucht 8, Sägewerk und Mine 4, jede Einheit 1, Mauer, Tor, Turm 0
  – solange sie steht oder lebt. Keine freien Arbeiter = nichts Neues. Werte: `worker_cost` / `workers` in `data\`.
- **Ziehen** baut eine Reihe (Mauern schräg 1 breit wie TAB, Türme mit Lücke). Das Gebäude bleibt an der Maus –
  Rechtsklick/Esc legt es weg.
- **Regeln:** nicht hinter der Grenze, kein Zombie auf dem Platz + 4 Kacheln drumherum. **Auf eigenen Einheiten
  erlaubt** – sie werden zur Seite geschoben.
- **Tor** (3 × 1, R dreht): Einheiten gehen durch, Zombies nicht. Darf auf Mauern (wird billiger).
- **Turm:** bis zu 4 Einheiten per Rechtsklick hinein – schießen und sehen 2 Kacheln weiter, unverwundbar. Q = raus,
  G = Sammelpunkt. 1 Kachel Abstand zum nächsten Turm.
- **Zelt:** 4 Kolonisten, 2 Arbeiter. Isst 1 Nahrung pro Abrechnung – satt zahlt es 8 Gold (× Stromanteil), hungrig
  nichts. Schild weg = infiziert (dann keine Arbeiter).
- **Reparieren (F)** bei jedem Gebäude, für alle ausgewählten: kostet den fehlenden Anteil von `repair_cost`
  (`data\buildings.json`, z. B. Mauer 5, Zelt 50, Kommandozentrum 300 für eine volle Reparatur), sofort bezahlt; die
  HP kommen über bis zu 10 s zurück. Ein infiziertes Gebäude ist danach geheilt (zu, zahlt wieder Gold).
- **Alles reparieren (X** oder Knopf „Repair all“ unten rechts unter „Energy“): alle Gebäude. X nur, wenn keine
  Einheiten ausgewählt sind (sonst ist X die Linien-Formation). Reicht das Gold nicht: die billigsten zuerst.
- **Nicht reparierbar**, solange ein Zombie in der Bauzone steht (4 Kacheln, wie beim Bauen) – später auch nicht auf
  Creep.

### Wirtschaft

- **Sammeln** (wie TAB): Sägewerk (Wald + Bäume → Holz), Jägerhütte (Wald → Nahrung), Fischerhütte (Wasser → Nahrung),
  **Mine** (Erzfelder → Eisenerz, Kohle, Stein – was eben dort liegt), **Farm** (3 × 3, Felder auf Gras → viel Nahrung;
  steht nur auf Gras, Erde und Erz tragen nichts). Jedes zählt die passenden Kacheln in einem **Quadrat**: seine
  Fläche + `gather_range` Kacheln rundum (wie TAB: Mine 2 → 6 × 6, Sägewerk 3, Jäger 4, Fischer 3, Farm 2). Die
  Kacheln darunter nur mit `gather_below` – **die Mine** steht auf dem Erz und baut es mit ab (wie Factorios Bohrer).
  Zwei gleiche teilen sich überlappende Kacheln. Beim Platzieren: Rahmen, gelbe Kacheln, „+x pro Abrechnung“; in der
  Auswahl „Range“ und „Below“.
- **Strom** (DESIGN.md 5.5): Kommandozentrum erzeugt 20 und reicht 8 Kacheln; **Tesla-Turm** im Netz trägt es 7
  Kacheln weiter. Alles, was Strom erzeugt, braucht oder trägt, muss im Netz stehen (blaue Kreise beim Platzieren).
  Zu wenig Strom → alles im Netz arbeitet anteilig langsamer (Sammeln, Gold der Zelte, Ausbilden). Tesla-Turm weg →
  das Netz zerfällt.
- **Windmühle** +50 (350 Gold + 25 Holz), **Köhlerei** +20, verbrennt aber 1 Holz pro Abrechnung (ohne Holz: nichts).
- **Bauen braucht freien Strom** (wie TAB): was Strom nimmt, wird nur gebaut, wenn sein Netz so viel übrig hat.
- **Freischalten** (wie TAB, die Werkstätten stellen nichts her):
  - **Holzwerkstatt** → Mine, Farm, Windmühle, Tesla-Turm, Pfahlfalle, Steinwerkstatt.
  - **Steinwerkstatt** → Munitionsfabrik, Steinhaus, Steinmauer, Steinturm.
  - **Munitionsfabrik** (kostet auch 10 Kohle) → Soldat und Scharfschütze. Munition selbst bleibt gratis.
- **Steinhaus** (16 Kolonisten, 8 Arbeiter, isst 2, zahlt 40), **Steinmauer** (1000 HP), **Steinturm** (2000 HP) lassen sich direkt
  auf Zelt, Mauer, Turm bauen – ein Upgrade an Ort und Stelle (ein Teil des alten Goldes kommt zurück).
- **Pfahlfalle** (wie TABs Stakes): Zombies laufen darüber, greifen sie nicht an und bekommen alle 0,5 s Schaden; jeder
  Treffer nutzt sie ab. Eigene Einheiten laufen unverletzt darüber.
- **Abrechnung alle 8 Stunden** (wie TABs 8-Stunden-Takt, 60 s, `economy_every_hours` in `data\colony.json`):
  Zinsen des Dispo → Köhlereien nehmen Holz → Sammler und Kommandozentrum liefern → Häuser essen und zahlen → Märkte
  verkaufen → **Unterhalt** wird bezahlt. Alle Werte „pro Abrechnung“.
- **Markt** (3 × 3, ab Start baubar, 400 Gold + 30 Holz): Häuser, deren Mitte auf ihm oder bis 2 Kacheln drumherum
  liegt (7 × 7), zahlen **+50 % Gold** – jedes Haus nur für einen Markt (den ältesten). Er **verkauft**, was über
  **80 % des Lagers** liegt, reihum je 1 pro Ware, höchstens **1 Ware je 4 Kolonisten** seiner Häuser pro Abrechnung
  (Holz 2, Stein/Eisenerz/Kohle 3, Nahrung 1 Gold). **Q**: Verkauf aus/an. Beim Platzieren: Rahmen, seine Häuser gelb,
  „+x Gold, bis zu y Waren“.
- **Bank** (2 × 2, Steinwerkstatt, 800 Gold + 30 Stein): Goldlager **+2.000** und **Dispo 500**. Fehlt beim Bauen oder
  Ausbilden nur Gold, geht es mit **Shift** ins Minus – ohne Schuldenstrafen, aber **10 % Zinsen** pro Abrechnung.
  Fällt die Bank, fällt ihr Teil der Dispo-Schulden weg – und das Gold über dem kleineren Lager. Nicht abschaltbar.
- **Kommandozentrum** (wie TAB): 10.000 HP, Rüstung 10 (jeder Zombie-Treffer −10), kein Schild, wird nie infiziert.
  Zahlt 40 Gold und bringt 5 Nahrung pro Abrechnung, gratis.
- Ausgewähltes Gebäude zeigt, was fehlt: KEIN STROM, WENIG STROM, HUNGRIG, KEIN HOLZ, AUS.
- **Unterhalt** (wie TAB, `upkeep` in `data\`): jede Einheit (Ranger 1, Soldat 3, Medic 3, Scharfschütze 5 pro Abrechnung)
  und jedes laufende Gebäude (Sägewerk 4, Mine 6, Farm 20, Windmühle 8, Köhlerei 4, Kaserne 14 …). Gold/h im Vorrat = Einnahmen
  minus Unterhalt.
- **Schulden:** Reicht das Gold nicht, wird der Rest Schulden (Gold rot mit Minus); neues Gold zahlt sie zuerst ab.
  Verschuldet: Einheiten −15 % Schaden und Schussrate; über 1 Abrechnung Unterhalt −30 % und langsamer; über 2
  **desertiert** alle 10 s eine (weg, keine Leiche). Werte in `data\colony.json`. Es zählt nur, was über dem Dispo
  der Banken liegt. **Söldner** desertieren schon ab Stufe 2, und solange es welche gibt, gehen nur sie.
- **Aus/An (Y)** bei jedem Gebäude, das etwas kostet oder macht (nicht Häuser, Mauern, Türme, Kommandozentrum, Bank): aus =
  kein Unterhalt, keine Arbeiter, kein Strom – und es tut nichts (eine Werkstatt schaltet dann auch nichts frei). Wird
  dunkel gezeichnet. Wieder an nur mit genug freien Arbeitern, freiem Strom im Netz und genug Goldeinnahmen
  für seinen Unterhalt (Gold der Häuser ≥ Unterhalt mit ihm).

### Karten-Editor (`dist\LastBastionEditor.exe`)

- **Starten:** direkt, über „Map editor“ im Spiel, über „Edit“ bei einer Karte, oder Kartendatei auf die Exe ziehen.
- **Edit / Test** (F10): Edit = keine Zeit vergeht, alles wird so gespeichert. Test = die Runde läuft; zurück in Edit
  ist die Karte wieder wie vorher.
- **Speichern:** Save / Strg+S. Ungespeichertes → Frage Save / Don't save / Cancel. „Revert to saved“ im Esc-Menü.
- **Gebäude** gehen im Editor überall hin und kosten nichts.

| Tab | Was |
|---|---|
| **Map** | Name (= Dateiname), Save, Close, New map, Tage, Startgold, Startholz |
| **Terrain** | Gelände malen (Berg, Wald, Baum, Wasser, Erde, Erase), Erz malen, **Öl stempeln** (2 × 2; eine Kachel löschen nimmt die ganze Quelle), Pinselgröße |
| **Entities** | Einheiten, alle Gebäude, **Radierer**, Zombies (Pinsel / Drop, alle 6 Arten, stürmen / wandern), **Doom house** und **Loot** (Klick), Spawner (setzen, ziehen, umbenennen, Constant / Periodic / Off), Sofort-Wellen |
| **Waves** | Zeitleiste der Wellen (Zeit, **Final** = ruft alle Zombies der Karte, **Reveal** = deckt die Karte auf, Anzahl, Art, woher) und der konstante Spawn |
| **View** (immer) | All / Player / Zombies – durch wessen Augen man schaut |

### Für Entwickler

- **Gleiche Karte:** „Custom maps“ → Play, oder `LastBastion.exe --seed 12345` (ohne eigene Datei).
- **Determinismus:** mit festem Seed ist „Checksum tick 600“ (F3) bei jedem Start gleich; `cargo test` prüft das.
- **Balancing:** `cargo test --release balance_mini_round -- --ignored --nocapture` spielt die Mini-Runde zweimal
  automatisch durch (untätig / einfacher Spieler: Sägewerk, Jäger, Köhlerei, Zelte, Holzwerkstatt, Tesla + Mine,
  Kaserne, Soldaten). `$env:ROAMERS = "0"` vorher: ohne herumstreifende Zombies – nur die Wirtschaft.
- **Kartenbilder:** `cargo test dump_previews -- --ignored` mit `DUMP_DIR` schreibt jeden Generator als PNG.
- **Tempo der Horde:** `cargo test --release horde_tick_time -- --ignored --nocapture` misst auf einer 256er-Karte,
  wie lange ein Takt mit 22.000 bis 87.000 Zombies dauert (er hat 50 ms) – zu Beginn (fast alle schlafen) und wenn
  die Schlusswelle alle ruft. Standard (65.000, ≈ 2.200 wach): ≈ 0,4 ms / ≈ 10 ms.

## Fahrplan

- Die Schritte stehen in [DESIGN.md, Abschnitt 13](DESIGN.md#13-prototype-roadmap).
- Reihenfolge: Steuerung → Horden-Stresstest → **Mini-Runde** (macht es Spaß?) → dieselbe Runde im **Koop**.
