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
  erklären, was er tut und was die Syntax bedeutet – mit Vergleich zu Python/pygame, wo es passt. **Kein
  Spickzettel** am Ende. Was schon erklärt wurde, nur kurz erwähnen statt
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
| `docs\tab-nachbau.md` | **TAB-Nachbau:** die nächsten Schritte, je einer pro Unterhaltung – wie TAB es macht (aus dem dekompilierten Code), was entschieden ist, welche Dateien betroffen sind |
| `Cargo.toml` / `Cargo.lock` | Rust-Projekt und Bibliotheken (`macroquad`, `egui-macroquad`, `serde`/`serde_json`, `libm`, `rayon`) / ihre genauen Versionen |
| `build.ps1` | Baut beide Programme nach `dist\LastBastion.exe` und `dist\LastBastionEditor.exe` |
| `data\` | **Alle Spielwerte** als JSON (Einheiten, Gebäude, Zombies, Lärm, Streuner, Villages of Doom, Beute, Kolonie, Spawn, Schwierigkeiten, Spieldauer, Karten) – erklärt in `data\README.md` |
| `assets\` | **Bilder** (Sprite-Sheets) der Zombies und Einheiten – erklärt in `assets\README.md` |
| `examples\test_sprites.rs` | Malt das graue Testbild `assets\test\figure.png` (`cargo run --release --example test_sprites`) |
| `src\main.rs`, `src\bin\editor.rs` | Die zwei Programme (je ein paar Zeilen) |
| `src\lib.rs`, `src\app.rs` | Liste aller Module / Fenster, Bildschirm-Wechsel, Kommandozeile |
| `src\game.rs` | Eine Karte auf dem Bildschirm: Takte, Zeichnen, Endbildschirm, Edit/Test im Editor |
| `src\sim\` | **Die Simulation** (deterministisch, zeichnet nichts): Befehle, Spawn, Zombies, Gedränge (`crowd.rs`), Fernkampf-Zombies (`ranged.rs`), Villages of Doom und Beute (`doom.rs`), Einheiten, Kampf, Gebäude, Wirtschaft (`economy.rs`), Markt (`market.rs`), Schulden und Bank (`debt.rs`), Sprengfässer (`blast.rs`), Scan (`scan.rs`), Runde, Tests |
| `src\placement.rs` | Alle Bauregeln an einem Ort |
| `src\data.rs`, `src\kinds.rs` | `data\` lesen / Einheiten- und Gebäudetypen |
| `src\aggro.rs`, `hunt.rs`, `flow.rs` | Wegfindung der Zombies und Einheiten (Bedrohungs-, Jagdkarte, Flow Field mit Sichtlinie) |
| `src\formation.rs`, `vision.rs`, `grid.rs` | Formationen / Sicht und Nebel / räumliches Raster |
| `src\map.rs` | Die Karte: Gelände, Erz/Öl, Gebäude, Weltgrenze, Sichtlinie |
| `src\mapgen\` | Kartengeneratoren (**TAB** – `tab.rs`, seine Rausch-Ebenen `layer.rs` –, Wild, Regions, Rings, Canyons, Blank), Wege hinein, Erz und Öl, Dörfer und Beute (`villages.rs`), Werkzeuge (auch Erde) |
| `src\mapfile.rs` | Eine Runde = eine Datei (`%APPDATA%\Last Bastion\maps\`) |
| `src\home.rs`, `menu.rs`, `widgets.rs`, `settings.rs` | Startbildschirm und Karten-Einstellung / Esc-Menü / Knöpfe und Regler / Einstellungen |
| `src\keys.rs`, `keymap.rs` | Welche Taste was tut (Standard, gespeichert in `settings.json`) / die Seite „Key mapping“ |
| `src\ui.rs`, `cursor.rs` | Maus und Tastatur → Auswahl, Gruppen, Kamera, Befehle / echte Windows-Maus |
| `src\hud.rs` | Die Leiste unten wie in TAB (Minimap, Uhr, Gruppen, Auswahl, Befehlskarte, Vorräte) |
| `src\editor.rs`, `src\undo.rs` | Das Panel des Editors (egui) / Rückgängig und Wiederholen |
| `src\draw.rs`, `sprites.rs` | Zeichnen (Bilder, sonst Platzhalter) / Sprite-Sheets laden, Richtung und Animation wählen |
| `src\picture.rs`, `iso.rs` | Kartenbild für Minimap und Vorschau / isometrische Kamera |
| `src\noise.rs`, `rng.rs` | Rauschen für die Generatoren (auch TABs Simplex-Rauschen) / eigener Zufallsgenerator |

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

- New game, Map editor, **Custom maps** (alle Dateien: Play / Edit / Delete), Settings, **Key mapping**, Quit.
- **New game** (wie TAB): **Karte**, **Schwierigkeit** und **Dauer** wählen, Seed, Vorschau – dazu der
  **Score-Faktor** (Karte × Schwierigkeit × Dauer; eine Punktzahl gibt es noch nicht).
  - **Custom levels** (zweiter Reiter, wie TABs „Custom Levels“): jede **im Editor gespeicherte** Karte – nicht die
    Dateien, die jede neue Runde anlegt. Ihre Runde steht in der Datei (Tage, Schwierigkeit, Wellen, Start), also nur
    Vorschau, „Start game“ und „Edit in the editor“.
  - **Karten** (`data\game_maps.json`, je zwei nebeneinander): zuerst **TABs sechs Landschaften** mit TABs Generator
    (Standard: **BR – Moorland**; TM Lowlands, AL Highlands, DS Wasteland, FA Caustic, VO Volcanic – Score 90–180 %).
    Dann unsere: **Peaceful Lowlands** (Wild wie TABs Karte: trockene Ebenen, grüne Wiesen mit Hainen, große glatte
    Seen), Green Valley (Wild), Broken Lands (Regions), Old Fortress (Rings), Red Canyons (Canyons), Open Plains
    (Blank mit einzelnen Waldkacheln).
  - **Schwierigkeit** (Easy / Normal / Hard / Brutal, `data\game_difficulties.json`): Startgold, -holz und -einheiten,
    Streuner, wie nah jede Art ans Kommandozentrum darf, Zombies pro Dorfhaus, Wächter der Beute, Stärke der Wellen
    und des konstanten Spawns. Auf TABs Karten statt Streuner und Abständen TABs **Bevölkerung** (0,6 / 1 / 1,2 / 1,5).
  - **Dauer** (`data\game_durations.json`): 10–80 Tage, jede mit eigenem Wellen-Takt.
- **Custom setup…** (im Editor: „New map“): Generator, Regler (Maus drauf = Erklärung), Schwierigkeit und Dauer.
  „Random values“ würfelt, „Defaults“ setzt zurück. Die letzten Werte merkt sich das Spiel.

| Generator | Wie |
|---|---|
| **TAB** | TABs eigener Generator, aus dessen Code (`src\mapgen\tab.rs`) – macht alles selbst, siehe unten. Seine Landschaften sind nur andere Zahlen |
| **Wild** | Rauschen wie Factorio: Seen (Regler „Shore detail“: glatt bis zerfranst), Bergrücken, Wälder („Forest size“: große Wälder oder viele Haine), Flüsse. Eine Feuchte entscheidet: nass = Wald, daneben grüne Wiese, trocken = Erde |
| **Regions** | Wie TAB: Regionen mit Wänden und **Pässen** dazwischen |
| **Rings** | Festungsringe mit versetzten Öffnungen |
| **Canyons** | Fels mit gewundenen Schluchten |
| **Blank** | Nur Gras – zum Selberbauen |

- **TABs Generator** (Rausch-Ebenen wie TAB, mit TABs Simplex-Rauschen): Land/Wasser, Berge in 2 × 2-Blöcken, Gras,
  Wald auf Gras und auf Erde. Das **Kommandozentrum** steht auf dem freien Platz nächst der Mitte; weniger als 18 Wald
  in 11 Kacheln, kein Stein (4) oder Eisen (10) in der Nähe oder eingesperrt (unter 30 % der Fläche erreichbar) →
  neuer Seed. Was es nicht erreicht, bleibt leer. **Dörfer** mit einem Haus in der Mitte und Häusern in einer Spirale
  (klein, mittel, groß); **Ruinen** nahe der Basis mit 2–4 Haufen Beute (Gold oder eine Ware, weiter draußen
  wertvoller); **Stein, Eisen, Gold** (gelb – statt Kohle, noch keine Mine dafür) als Flecken auf freier Erde; **Öl**
  eine Quelle pro Block. **Zombies** aus einem Rauschen: je höher, desto mehr auf einer Kachel (1–3), weiter draußen
  stärkere Arten; **Riesen** ab 70 Kacheln. Die Zahlen jeder Landschaft sind geschätzt (TABs `ZXRules.dat` bleibt zu).
  Nicht übernommen: alte Türme, Raben, Vorräte, Mutanten.
- **Für unsere Generatoren:** Größe (96–256), freier Platz um die Basis, Spawner pro Seite, Wege hinein, Villages of
  Doom (wie viele, Häuser nahe der Basis / am Rand), Beute, Erde, Erzfelder, Ölgruppen. TABs nimmt davon nur Größe
  und Spawner, das Startgebiet und die Wege hinein gibt es dort nicht.
- **Erde** (braun): begehbar und bebaubar wie Gras, aber **keine Farm** – in Flecken, 40 % des offenen Bodens (Regler
  „Dirt“, nicht auf Blank). Bei Wild dort, wo es am trockensten ist.
- **Startgebiet** (wie Factorio, `src\mapgen\start.rs`): Auf jeder Karte findet ein Sägewerk im Netz des
  Kommandozentrums ≥ 20 Waldkacheln (ist die Lichtung breiter als das Netz: einen Tesla-Turm weiter) – reicht der
  Wald dort nicht, wächst ein Hain dazu. Ein Steinfeld liegt einen Tesla-Turm weit weg (die Kaserne braucht Stein).
  Ein Test prüft das für jede Karte mit 10 Seeds.
- **Wege hinein:** gewundene offene Wege von jeder Seite zur Basis, die sich wie Äste vereinigen – die Stellen zum
  Zumauern. Flüsse bekommen dort Furten, Regionswände Pässe. Ein Sicherheitsnetz verbindet alles Abgeschnittene.
- **Weltgrenze:** gerade auf dem Bildschirm wie in TAB – die Spielfläche ist ein Rechteck (doppelt so breit wie
  hoch), die Minimap auch. Außen ein Ring offenen Geländes (≈ 11 Kacheln) mit den **Spawnern** (3 pro Seite, fast die
  ganze Seite breit). Dahinter sieht und läuft man nicht. Größe = so viele Kacheln wie ein Quadrat dieser Kante.
  Im Editor lässt sich die Spielfläche überallhin ziehen und beliebig groß machen – Kamera und Minimap folgen ihr.
- **Erz:** Eisenerz (rostbraun), Kohle (schwarz), Stein (hellgrau) als Felder; neben der Basis je ein kleines (Stein
  12–16 Kacheln weg, Eisen und Kohle 14–24 – ein, zwei Tesla-Türme), draußen größer und reicher. Abgebaut von der **Mine** –
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
- **Auf TABs Karten** legt sie der Generator (oben): auf 256 und Normal ≈ 17.000–46.000 je nach Landschaft, nahe der
  Basis keine (10 Kacheln), Runner und Riots ab ≈ 25, Fat, Harpyie und Spitter ab ≈ 65 (BR). In einem Dorfhaus schlafen klein
  1/5, mittel 2/5, groß alle `doom_zombies` (TABs Dörfer haben viel mehr Häuser). Der Rest dieses Abschnitts gilt
  für unsere Karten.
- **Wie viele (wie TAB):** fast überall, in **Grüppchen von 1–3 auf einer Kachel – nie mehr als 3** (`pack` in
  `data\roaming.json`). `roaming_zombies` der Schwierigkeit: pro 1.000 Kacheln, auf denen sie stehen können (Easy 900,
  Normal 1.500, Brutal 2.000 = eine Gruppe auf fast jeder Kachel). Auf 256 und Normal ≈ 65.000, dazu die in Dörfern
  und um Beute.
- **Die Balance machen die Arten, nicht die Menge:** nahe der Basis ≈ 1 pro Kachel, nur Walker; am Rand ≈ 1,6, davon
  über die Hälfte Runner, Riots, Infested, Spitter, Fat.
- **Wie nah jede Art zu Beginn ans Kommandozentrum liegt** (`keep_out` der Schwierigkeit, Normal: Walker und Runner
  14, Riot und Infested 25, Spitter 30, Fat 35 Kacheln; Brutal: alle 14). Beim Wandern dürfen sie näher, wie in TAB –
  nur der Riese bleibt 54 Kacheln weg. **Sanfter Rand:** direkt am Ring liegen sie dünn und einzeln, erst 6 Kacheln
  weiter (`ring_ramp`) so viele wie überall, bis 3 pro Kachel.
- **Arten:** **Walker** (langsam), **Runner** (schnell), **Fat** (groß, 600 HP), **Riot** (TABs gepanzerter Runner:
  schnell, Rüstung 3 – ein Pfeil macht nur 7), **Spitter** (TABs Venom: spuckt Säure 3,5 Kacheln weit, über Mauern;
  sie fliegt kurz und trifft alle im Umkreis von 1 Kachel – wer rechtzeitig weggeht, entkommt), **Infested** (SC2s
  Infested Marine: schießt 4,5 Kacheln, knapp unter dem Soldaten), **Harpyie** (TABs Harpy, gelb: schnell, ihr
  hört 2–4× sicherer als die anderen und 32 Kacheln weit, kommt zu jedem Kampf als Erste; jeder ihrer Treffer macht
  Lärm 200, alle 0,75 s – schneller, als er sich halbiert, also wird es laut und alles in der Nähe kommt – zuerst
  erschießen! **Springt über Mauern und
  Tore** bis 3 Kacheln dick (0,6 s in der Luft), wenn ihr Ziel dahinter liegt; eine dickere Mauer schlägt sie ein),
  **Riese** (wie TAB, etwas kleiner: 3.000 HP, Rüstung 6, 150 Schaden – eine Mauer in 3 Schlägen; nur ein paar pro
  Karte weit draußen – **wie viele, sagt die Karte** (Einstellung „Giants“, Standard 4 auf 256er-Karten, Peaceful
  Lowlands 2, Broken Lands 5, Open Plains 6), die Schwierigkeit macht daraus 50 % bis 225 % – die Schlusswelle ruft
  auch sie). Weiter draußen mehr von den harten Arten.
- **Schlafen weit weg** (DESIGN.md 8.5): Wer weit weg von deinen Augen liegt, schläft – kostet fast nichts. Wach werden
  sie, wenn eine Einheit oder ein Gebäude auf Sichtweite + 8 Kacheln herankommt (bevor du sie siehst), durch Lärm
  (genau wie die wachen – und gehen hin), durch Scan, Sprengfass und die Schlusswelle (alle, 2.000 pro Takt, die
  nächsten zuerst). Wer 20 s ruhig und weit weg von allen Augen ist, schläft wieder ein.
- **Auch Schlafende wandern** – nach denselben Würfeln wie die wachen (nur kommen sie sofort an und warten dann, so
  lange der Weg gedauert hätte). Ein Ausflug, der nahe deiner Augen endet, weckt sie unterwegs: Sie laufen wach von
  außen herein. **Ein leergeräumter Ort füllt sich so wieder auf** – die Zombies gehen dir nicht aus. Eine Welle, die
  über Schlafende zieht, steckt sie an wie wache.
- **Gedränge wie TABs Physik – die Masse fließt:** Zombies sind glatte Kreise mit Gewicht, die sich jeden Tick in 8
  Runden auseinanderschieben (`src\sim\crowd.rs`). Vor einer Mauer drückt die Masse die vorderen seitlich weg und
  verteilt sich über die ganze Wand, durch eine Lücke fließt sie wie Wasser, um Ecken herum. Sie steht dabei ein Stück
  ineinander und ruhig, ohne zu zittern, und kein Körper steckt in einer Wand. **Gewicht:** Riese und Fat schieben Kleine
  weg, ein Walker bewegt einen Riesen kaum. **Felsen wie TAB:** wer gerade angehalten hat, steht 5 s fest (500-fach
  schwer) – die anderen fließen um ihn herum. Deine Einheiten auch, solange sie halten oder zielen; sonst schiebt die
  Horde sie.
- **Einblenden:** Zombies kommen sanft ins Bild (0,6 s) und verschwinden sanft (0,8 s) – wie TAB, kein Aufploppen am
  Sichtrand. Im Editor („All“, „Zombies“) sieht man auch die Schlafenden, dunkler.
- **Leerlauf wie TAB:** ohne Ziel wird gewürfelt – meistens **stehen** (1300), selten **knurren** (150) oder
  **wandern** (150). Ein Ausflug geht auf jeder Achse für sich Walker 2–8 Kacheln … Fat 4–30 weit (ein Quadrat, kein
  Kreis), ohne festen Platz – mit der Zeit also überall hin, auch nahe an die Basis. Der **Riese brüllt** alle 12 s
  und bleibt dafür stehen, egal was er tut.
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
- **Der Entscheidungsbaum wie TAB** (aus dessen Code, DESIGN.md 8.1): Ziel in Sicht → hin und zuschlagen; aus den
  Augen verloren → zu dem Platz, stehen bleiben; Welle → zum Kommandozentrum; Lärm → hin; sonst Leerlauf.
- **Ziele:** alle 0,5 s schaut sich jeder um (mit Ziel doppelt so selten, außerhalb deiner Sicht 3×, im Unerkundeten
  5× so selten) – **Sehen** 5–8 Kacheln, nicht durch Wald. Ziele sind nur **deine Einheiten und Gebäude mit Leuten**
  (Kommandozentrum, Häuser, Gebäude mit Arbeitern, Türme mit Einheiten) – **keine Mauern, leeren Türme, Strommasten**.
- **Wand-Regel:** Steht eine Mauer, ein Tor oder Turm auf der geraden Linie zum Ziel, reißt er **die** ein. Niemand
  sucht eine Lücke. Die Harpyie springt stattdessen.
- **Angeschossen:** Er und alle ohne Ziel auf seiner Kachel und den vier daneben gehen auf den Schützen los (sitzt der
  im Turm: auf den Turm) – auch außer Sicht, dann zu seinem Platz.
- **Ansteckung:** Stehen auf der Kachel eines Zombies mindestens 3 Wellen-Zombies mit gleichem oder höherem Rang
  (Walker 1, Runner 2, Riot/Harpyie 3, Spitter/Infested 4, Fat 5), läuft er mit – eine Welle wächst auf ihrem Weg.
- **Sicherung:** Sind weniger als 100 Zombies übrig, stirbt alle 2 Minuten jeder, der keinen Weg zum Kommandozentrum
  hat.
- **Lärm genau wie in TAB** (aus dessen Code, `data\sound.json`): Er sammelt sich dort, wo etwas passiert (je 4 × 4
  Kacheln) – je Schuss Ranger 1, Soldat 3, Scharfschütze 10; jeder Zombie-Treffer 1, ein platziertes Gebäude 40, eine
  sterbende Einheit 50, ein zerstörtes Gebäude 100, ein **infiziertes** 100 + 50 je Zombie, der herauskommt. **Jede
  Sekunde halbiert sich jede Zelle** – ein langer Kampf pendelt sich bei etwa dem Doppelten ein, was er pro Sekunde
  macht; eine Gruppe dicht beieinander ist lauter als verteilt.
- **Maschinen sind laut** (wie Factorios Verschmutzung): alle 10 s macht jedes arbeitende Gebäude seinen Lärm, alle
  im selben Moment – Jägerhütte 50, Sägewerk und Munitionsfabrik 100, Mine und Köhlerei 150 (mal Stromanteil; aus,
  infiziert, Köhlerei ohne Holz: still). Ein kurzer Stoß, der sich in Sekunden weghalbiert: weit weg lockt er selten
  jemanden, nah dran schon – Nachbarn im selben 4 × 4-Feld zählen zusammen.
- **Unruhe:** aller Lärm der Kolonie (Schüsse, Maschinen, Explosionen, Bauen – nicht der der Zombies) zählt zusammen
  und **sinkt nie**: je 50.000 bringt jede Gruppe des konstanten Spawns **für immer** einen Zombie mehr. Die Fabrik
  vor der Schlusswelle abzureißen, nimmt das nicht zurück. F3 zeigt sie.
- **Wer kommt:** Jeder Zombie hört alle 4 s hin (jeder zu einem anderen Zeitpunkt, Schlafende auch) – 4 × seine
  Sichtweite weit (Walker 20 Kacheln). Von den lauten Stellen nimmt er die, die er am besten hört: **Lärm × 20 ×
  Wachsamkeit / Entfernung²** ist seine Chance in Prozent (Wachsamkeit Walker 2, Runner 3, Spitter 4, Harpyie 8) –
  doppelt so weit weg nur ein Viertel so sicher. Häuser und Beute-Wächter hören jede Sekunde hin. Mit F3 sieht man den
  lautesten Ort und wie sicher ein Walker in 10 Kacheln käme.
- **Gedächtnis:** Ein Zombie geht zum Lärm und bleibt dort, **solange es laut ist** – ist es still, wenn er das
  nächste Mal hinhört, vergisst er es. Wo er eine Einheit aus den Augen verloren hat, geht er hin und bleibt dort, wenn
  er nichts findet. Nur Wellen-Zombies laufen danach weiter zum Kommandozentrum. Angeklickt zeigt ein Zombie mit
  einer roten Linie, wohin er will. Auf dem Weg laufen sie um Wasser und Berge, aber **durch Mauern** (einreißen).
- Tote Einheiten stehen als TABs Infizierte auf (3/8 Walker, 4/8 Runner, 1/8 Riot) und laufen zum Kommandozentrum.
  Ein Zelt mit gebrochenem Schild ist **infiziert** und gehört den Zombies – Zombies und Einheiten laufen hindurch;
  heraus kommen 4 (Steinhaus 16) solcher Infizierten wie eine Welle. **Reparieren** heilt es. Fällt ein Gebäude mit
  Arbeitern (Jägerhütte, Sägewerk …), kommen die Arbeiter als Zombies heraus – laut! Weit draußen gebaute Hütten also
  gut schützen.

### Einheiten und Kampf

- **Soldat, Ranger, Scharfschütze, Medic, Speerträger** – alle Werte in `data\` (siehe `data\README.md`).
- **Speerträger** (Kaserne, 50 Gold, ohne Munitionsfabrik): Nahkampf mit 1,2 Kacheln Reichweite, **jeder Treffer stößt
  den Zombie zurück** (0,6 Kacheln, große weniger: Fat ≈ die Hälfte, Riese ≈ ein Viertel), **fast lautlos** (Lärm 2 pro
  Stich – etwas mehr als ein Zombie-Treffer, weniger als ein Gewehrschuss). Hält früh eine
  Linie oder ein Tor – gegen Spitter und Infested stirbt er.
- **Veteranen:** jeder Zombie, der stirbt, gibt Erfahrung (Walker 1, Riot/Spitter/Infested/Harpyie 3, Fat 10, Riese
  100), geteilt unter allen Einheiten in 8 Kacheln – Veteranen nehmen einen halben Anteil. Bei 20 wird eine Einheit
  **Veteran**: +25 % HP (sofort), +20 % Schaden, silberne Marke, desertiert dreimal seltener. Söldner lernen nicht.
- **Entlassen (Entf):** die ausgewählten Einheiten verlassen die Kolonie – kein Unterhalt mehr, Arbeiter frei, kein
  Gold zurück.
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
- **Medic:** greift nie an, heilt mit seinem Strahl 4 HP/s – aber **nur, wer 3 s nicht getroffen wurde**: kein
  SC2-Heilen mitten im Kampf, Verletzte muss man zurückziehen.
- **Formationen** (wie *Diplomacy is Not an Option*): nur beim **Ziehen** – Linie, Block, Kreis.

### Die Leiste unten (wie TAB)

- **Minimap** (klicken/ziehen = Kamera, Rechtsklick = hinlaufen). Immer sichtbar, auch im Nebel: jeder **Riese**
  (oranger Punkt, wach oder schlafend) und jeder **Schwarm einer Welle** (rotes Warndreieck in seiner Mitte, größer
  je mehr er hat – eine Welle von zwei Seiten sind zwei Schwärme; noch im Spawn-Ring: am Rand der Minimap).
- **Uhr:** Tag, Stunde, **Pause-Knopf**, Tempo, nächste Welle (Schlusswelle rot).
- **Kontrollgruppen** 1–0 untereinander.
- **Auswahl:** eine Sache = Bild, Name, Lebensbalken (HP + Schild in einem), Text, Werte in Kästchen; mehrere = Symbole.
- **Befehlskarte** 5 × 3 mit Tooltips, Preis rot, wenn das Gold nicht reicht.
- **Gold im Minus:** gelb („-250 of 500“), solange es im Dispo der Banken liegt, rot darüber.
- **Vorräte:** Kolonisten, **Arbeiter** (frei/alle), **Nahrung**, **Energie** (gebraucht/erzeugt), Gold, Holz, Stein,
  Eisenerz, Kohle, **Eisen** – jeweils mit „+/− pro Abrechnung“ (alle 8 Stunden); rot, wenn etwas knapp wird.

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
| S / H | Stopp / Halten (läuft nicht von selbst; eigene Einheiten schieben sie, Zombies nicht) |
| F / X | Ausgewählte Gebäude reparieren / alle reparieren (X nur ohne ausgewählte Einheiten) |
| **Shift** + Befehl | Hinten an die Befehlskette hängen. Shift loslassen = Befehl weg von der Maus, wieder drücken = zurück |
| **Shift** + bauen / ausbilden | Fehlt nur Gold: ins Minus, in den Dispo der Banken (ohne Shift ist 0 die Grenze) |
| **Strg** + ausbilden | 5 auf einmal (auch Fässer) – anders als SC2, dort ist es Shift |
| D | Einheiten mit Fass: abstellen |
| Entf | Ausgewählte Einheiten entlassen / Gebäude abreißen |
| Strg / Shift / Strg+Shift + 1–0 | Gruppe setzen / dazu / „stehlen“ – auch mit Gebäuden. 1–0: auswählen, zweimal: Kamera hin |
| WASD, Pfeile, Rand, Mausrad | Kamera, Zoom. **WASD nur ohne Auswahl** (sonst gehören die Tasten der Befehlskarte) |
| Mittlere Maus ziehen | Karte verschieben |
| Strg + F5–F8 / F5–F8 | Kamera merken / hinspringen. Rücktaste: zur Basis |
| Leertaste | Pause (Befehle gehen weiter) · `+` / `-`: Tempo ×1–×8 |
| Esc | Bricht ab, was wartet → hebt die Auswahl auf → Spielmenü (Settings, **Key mapping**, Restart, New map, Main menu) |
| F3 / F12 | Messwerte, Schalter, Tastenhilfe (Abschnitte per Klick ein-/ausklappen) / Maus im Fenster halten an/aus |

- **Die Tabelle zeigt die Standardtasten – jede ist änderbar:** Esc → **Key mapping** (auch im Startbildschirm).
  - 5 Tabs: Kamera und Zeit, Auswahl und Gruppen, Einheiten (Befehle, Formationen), Kommandozentrum (die Baumenüs und
    **jedes Gebäude darin**, wie in TAB), Gebäude (Ausbilden, Sammelpunkt, Reparieren, On/Off …).
  - Je Aktion **2 Tasten**. Kästchen anklicken → neue Taste drücken; Rechtsklick leert es, Esc bricht ab.
  - **Rot** = zwei Aktionen haben dieselbe Taste auf einer Karte (oder beide überall). Maus auf der Zeile: was sonst
    auf der Taste liegt bzw. die Standardtaste. „Defaults for this tab / for all“ setzt zurück.
  - Fest bleiben **Shift, Strg** (Kette, 5 auf einmal, Gruppe setzen …), **Esc** und im Editor **Strg+S**, **Strg+Z** / **Strg+Y**.
  - Die Befehlskarte geht vor: eine Taste, die auch überall gilt, wirkt nur, wenn die Karte sie gerade nicht hat (wie
    X: Linie bei Einheiten, sonst alles reparieren). Kamera-Tasten, die auf einer Karte liegen, nur ohne Auswahl.
  - F3-Tastenhilfe, Befehlskarte und Tooltips zeigen immer die eingestellten Tasten. Gespeichert in `settings.json`
    unter `keys` – nur, was vom Standard abweicht.

### Bauen

- **Kommandozentrum:** ein **Baumenü pro Taste** (wie TABs Kategorien). Fett = gibt es schon, der Rest ist grau mit
  „soon“ (Tooltip sagt, was es tun soll). **B** oder **Esc** zurück – oder das Kommandozentrum neu auswählen (Klick, Gruppe).
  Dazu **C Scan** (unten).

| Menü | Q | W | E | R | T | A | S |
|---|---|---|---|---|---|---|---|
| **Q** Kolonisten | **Zelt** | **Steinhaus** | | | | | |
| **W** Ressourcen | **Jägerhütte** | **Fischerhütte** | **Sägewerk** | **Mine** | Ölpumpe | **Farm** | |
| **E** Energie | **Tesla-Turm** | **Windmühle** | **Köhlerei** | Ölgenerator | | | |
| **A** Industrie und Handel | **Holzwerkstatt** | **Steinwerkstatt** | **Gießerei** | **Gasthaus** | **Markt** | **Bank** | |
| **S** Militär | **Kaserne** | **Munitionsfabrik** | Belagerungsfabrik | | | | |
| **D** Verteidigung | **Mauer** | **Tor** | **Turm** | **Pfahlfalle** (R) | **Sprengfass** (T) | **Steinmauer** (A) | **Steinturm** (D) |
| **Y** Wunder | – | | | | | | |

Dazu im Verteidigungsmenü **Balliste** (F) und **MG-Turm** (G).

- **Kaserne** (ohne Voraussetzung baubar): Q Soldat (100 Gold), W Ranger (60), E Speerträger (50), R Scharfschütze
  (200), T Medic (120), G Sammelpunkt. **Soldat und Scharfschütze** (Schusswaffen) erst mit einer **Munitionsfabrik**. Strg = 5 auf
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
  (fast alles wie TAB). Der Tooltip zeigt den ganzen Preis.
- **Bauzeit** (wie TAB, `build_seconds` in `data\buildings.json`): ein Gebäude baut sich selbst – kurz, länger je
  größer und je höher im Techtree (Mauer 1 s, Zelt 3 s, Sägewerk 5 s, Kaserne 10 s, Steinwerkstatt 12 s, Gasthaus
  15 s). Es fängt mit 10 % HP an, wächst bis voll, bekommt den Schild erst fertig. Bis dahin tut es nichts (keine
  Arbeiter, kein Gold, kein Strom, keine Ausbildung), nimmt aber schon seine Arbeiter – und Zombies können es treffen.
  Blass und niedriger gezeichnet, unten „Built x %“. Im Editor steht alles sofort.
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
- **Abreißen (Entf, „Demolish“)** bei jedem Gebäude außer dem Kommandozentrum, für alle ausgewählten: sofort weg,
  **25 %** des Preises kommen zurück (Gold, Holz, Stein …), **75 %**, solange es noch gebaut wird
  (`demolish_…_percent` in `data\buildings.json`). Nicht bei infizierten. Der Tooltip zeigt, was zurückkommt.
- **Reparieren (F)** bei jedem Gebäude, für alle ausgewählten: kostet den fehlenden Anteil von `repair_cost`
  (`data\buildings.json`, z. B. Mauer 5, Zelt 50, Kommandozentrum 300 für eine volle Reparatur), sofort bezahlt; die
  HP kommen über bis zu 10 s zurück. Ein infiziertes Gebäude ist danach geheilt (zu, zahlt wieder Gold).
- **Alles reparieren (X** oder Knopf „Repair all“ unten rechts unter „Energy“): alle Gebäude. X nur, wenn keine
  Einheiten ausgewählt sind (sonst ist X die Linien-Formation). Reicht das Gold nicht: die billigsten zuerst.
- **Nicht reparierbar**, solange ein Zombie in der Bauzone steht (4 Kacheln, wie beim Bauen) – später auch nicht auf
  Creep.

### Wirtschaft

- **Sammeln** (wie TAB): Sägewerk (Wald → Holz), Jägerhütte (Wald → Nahrung), Fischerhütte (Wasser → Nahrung),
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
  - **Mine** und **Tesla-Turm** gehen ab Start (wie TAB – die Kaserne braucht Stein).
  - **Holzwerkstatt** → Farm, Windmühle, Pfahlfalle, **Balliste**, Steinwerkstatt.
  - **Steinwerkstatt** → Munitionsfabrik, **Gießerei**, Steinhaus, Steinmauer, Steinturm.
  - **Gießerei** → MG-Turm.
  - **Munitionsfabrik** (kostet auch 10 Kohle) → Soldat und Scharfschütze. Munition selbst bleibt gratis.
- **Steinhaus** (16 Kolonisten, 8 Arbeiter, isst 2, zahlt 40), **Steinmauer** (1000 HP), **Steinturm** (2000 HP) lassen sich direkt
  auf Zelt, Mauer, Turm bauen – ein Upgrade an Ort und Stelle (ein Teil des alten Goldes kommt zurück).
- **Pfahlfalle** (wie TABs Stakes): Zombies laufen darüber, greifen sie nicht an und bekommen alle 0,5 s Schaden; jeder
  Treffer nutzt sie ab. Eigene Einheiten laufen unverletzt darüber.
- **Gießerei** (3 × 3, Steinwerkstatt, 1.500 Gold + 40 Holz + 40 Stein, Strom 10, Unterhalt 15): ein Tech-Gebäude wie
  TABs – sie schaltet den MG-Turm frei – und sie **schmilzt Eisen**: bei jeder Abrechnung bis zu 3, aus je 2 Eisenerz
  und 1 Kohle (weniger bei wenig Strom, nichts ohne Erz). Laut wie eine Mine; ohne Erz still.
- **Türme, die selbst schießen** (`weapon` in `data\buildings.json`, keine Einheiten darin):
  - **Balliste** (2 × 2, Holzwerkstatt, 300 Gold + 20 Holz + 10 Stein, kein Strom): ein schwerer Bolzen alle 2,5 s,
    120 Schaden, 9 Kacheln weit – immer auf den **zähesten** Zombie in Reichweite (Fat, Riot, Riese). Lärm 6 pro Schuss.
  - **MG-Turm** (1 × 1, Gießerei, 600 Gold + 20 Stein + **20 Eisen**, Strom 6): 5 Schuss pro Sekunde, 9 Schaden,
    6,5 Kacheln, auf den **nächsten** – gegen Massen. Lärm 4 pro Schuss, also sehr laut. Wenig Strom: lädt langsamer.
    Ziehen = Reihe.
  - Ein angeschossener Zombie (und die um ihn ohne Ziel) geht auf den Turm los, wie bei Einheiten. Aus (Y) schießen
    sie nicht.
- **Abrechnung alle 8 Stunden** (wie TABs 8-Stunden-Takt, 60 s, `economy_every_hours` in `data\colony.json`):
  Zinsen des Dispo → Köhlereien nehmen Holz → Sammler und Kommandozentrum liefern → Gießereien schmelzen → Häuser essen
  und zahlen → Märkte verkaufen → **Unterhalt** wird bezahlt. Alle Werte „pro Abrechnung“.
- **Markt** (3 × 3, ab Start baubar, 400 Gold + 30 Holz): Häuser, deren Mitte auf ihm oder bis 2 Kacheln drumherum
  liegt (7 × 7), zahlen **+50 % Gold** – jedes Haus nur für einen Markt (den ältesten). Er **verkauft**, was über
  **80 % des Lagers** liegt, reihum je 1 pro Ware, höchstens **1 Ware je 4 Kolonisten** seiner Häuser pro Abrechnung
  (Holz 2, Stein/Eisenerz/Kohle 3, Nahrung 1, Eisen 10 Gold). **Q**: Verkauf aus/an. Beim Platzieren: Rahmen, seine Häuser gelb,
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
- **Rückgängig:** Undo / Strg+Z, Redo / Strg+Y (oder Strg+Shift+Z) – die letzten 20 Schritte, nur in Edit. Ein Schritt =
  alles, solange die linke Maustaste gedrückt ist (ein ganzer Pinselstrich), oder eine einzelne Änderung.
- **Gebäude** gehen im Editor überall hin und kosten nichts. **Entf** (oder „Demolish“) nimmt die ausgewählten weg –
  auch das Kommandozentrum, ohne Rückgeld –, dazu den ausgewählten Spawner. Der Radierer nimmt alles unter dem Pinsel.
- **Speichern = Custom Level:** jede im Editor gespeicherte Karte steht im Spiel unter New game → **Custom levels**.
- **Spawner:** **Rechtsklick** (ohne Auswahl) öffnet ein Fenster: Name, **gerade** (wie die Weltgrenze) oder **schräg**
  (wie die Kacheln, um 45° gedreht), was er von selbst schickt, entfernen.
- **Panel** (egui): Zahlen ziehen oder anklicken und tippen (Enter: übernehmen, Esc: abbrechen), Listen zum Aufklappen,
  Abschnitte klappen mit einem Klick auf die Überschrift zu, das Mausrad scrollt das Panel. Solange man tippt, wirkt
  keine Taste im Spiel.

| Tab | Was |
|---|---|
| **Map** | Name (= Dateiname), Save, Close, New map, Tage, Startgold, Startholz, **Playing field** (die Weltgrenze wie SC2s Map Bounds: „Drag the border“ – Kante greifen und ziehen, innen ganz verschieben – oder Breite/Höhe tippen; das Gelände bleibt, nur die Grenze wandert; „Like a new map“ setzt zurück), **Grid** (die ganze Karte darum, wie SC2s Map Size: Apply – wächst/schrumpft an allen Seiten, alles bleibt, wo es ist, neues Land ist Gras; Undo geht) |
| **Terrain** | Gelände malen (Berg, Wald, Wasser, Erde, Erase – eine einzelne Waldkachel sieht aus wie Wald), Erz malen, **Öl stempeln** (2 × 2; eine Kachel löschen nimmt die ganze Quelle), Pinselgröße |
| **Entities** | Einheiten, alle Gebäude, **Radierer**, Zombies: **Pinsel wie TAB** (pro Kachel ein Regler je Art, 0–3; nochmal drübermalen füllt nur auf), **Drop** (eine Menge einer Art), **Giant** (einer pro Klick) – schlafend/bummelnd oder stürmend, **Doom house** und **Loot** (Klick), Spawner (setzen, ziehen, umbenennen, Constant / Periodic mit Sekunden und Anzahl / Off) |
| **Waves** | Sofort-Wellen, Zeitleiste der Wellen (Tag und Stunde, **Final** = ruft alle Zombies der Karte, **Reveal** = deckt die Karte auf) – jede Welle hat **Gruppen**: woher (1 zufälliger Spawner, 1–4 zufällige Seiten oder **ausgewählte Spawner** per Häkchen bei ihren Namen – die Zahl verteilt sich auf sie) und **mehrere Arten** mit Anzahl („+ breed“, „+ group“). Und der konstante Spawn (Stufen: ab wann, Art, alle wie viele Sekunden, Gruppengröße, **woher**: die auf Constant oder eigene Spawner – Zuwachs pro Tag danach – **Specials**: Anteil einer Art an jeder Gruppe ab einem Tag) |
| **View** (immer) | All / Player / Zombies – durch wessen Augen man schaut |

### Für Entwickler

- **Gleiche Karte:** „Custom maps“ → Play, oder `LastBastion.exe --seed 12345` (ohne eigene Datei).
- **Determinismus:** mit festem Seed ist „Checksum tick 600“ (F3) bei jedem Start gleich; `cargo test` prüft das.
- **Balancing:** `cargo test --release balance_mini_round -- --ignored --nocapture` spielt die Mini-Runde zweimal
  automatisch durch (untätig / einfacher Spieler: Sägewerk, Jäger, Köhlerei, Zelte, Holzwerkstatt, Tesla + Mine,
  Kaserne, Soldaten). `$env:ROAMERS = "0"` vorher: ohne herumstreifende Zombies – nur die Wirtschaft.
- **Kartenbilder:** `cargo test --release dump_previews -- --ignored` mit `DUMP_DIR` (und `DUMP_SEEDS`, sonst 2) schreibt
  jeden Generator und jede Karte aus `data\game_maps.json` als PNG – gedreht wie die Minimap und TABs Kartenansicht.
- **Tempo der Horde:** `cargo test --release horde_tick_time -- --ignored --nocapture` misst auf einer 256er-Karte,
  wie lange ein Takt mit 22.000 bis 87.000 Zombies dauert (er hat 50 ms) – zu Beginn (fast alle schlafen) und wenn
  die Schlusswelle alle ruft. Standard (65.000, ≈ 2.200 wach): ≈ 0,4 ms / ≈ 10 ms.

## Fahrplan

- Die Schritte stehen in [DESIGN.md, Abschnitt 13](DESIGN.md#13-prototype-roadmap).
- Reihenfolge: Steuerung → Horden-Stresstest → **Mini-Runde** (macht es Spaß?) → dieselbe Runde im **Koop**.
