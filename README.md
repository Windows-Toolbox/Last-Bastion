# Last Bastion

- **Echtzeit-Strategie ums Überleben:** Einheiten steuern wie in **StarCraft**, Horden und Basisbau wie in **They Are
  Billions (TAB)**, Produktionsketten und Strom wie in **Factorio**.
- **Runden von 2–4 Stunden** – wie in TAB wählt man die Anzahl der Tage. Isometrisch wie TAB.
- **Kern:** die TAB-Seite – die Horde gegen eine Basis im Raster.
- **Stand:** spielbare Runde + Karten-Editor – was alles drin ist: [Prototyp ausprobieren](#prototyp-ausprobieren).
  Das ganze Design steht in [DESIGN.md](DESIGN.md) (Englisch).

---

## Regeln für Claude (bitte zuerst lesen)

`CLAUDE.md` und Claudes Memory liegen nur lokal – deshalb stehen die Regeln hier:

- **Ich (Maximilian) gebe die Idee, Claude schreibt den Code.** Das gilt auch für `DESIGN.md`, diese README und
  Build-/Einrichtungs-Skripte.
- **Erklären nur auf Wunsch:** Claude muss den Code nicht mehr Stück für Stück erklären. Frage ich nach, erklärt
  es den Abschnitt (was er tut, was die Syntax bedeutet, gern mit Vergleich zu Python/pygame). Kommentare im Code
  bleiben.
- **Am Ende eine kurze Zusammenfassung** im Chat: was geändert wurde, warum, und wo vom Plan abgewichen wurde. Ändert sich, was der Prototyp kann, wird
  [Prototyp ausprobieren](#prototyp-ausprobieren) an der passenden Stelle angepasst – **kurz, in Stichpunkten**.
  **Keine Verlaufs-/Stand-Einträge** mit Datum: der Verlauf steht in git.
- Passt eine Idee nicht zum Design (`DESIGN.md`) oder hat sie einen Haken, sagt Claude das, bevor es loslegt.

---

## Dateien

| Datei | Was |
|---|---|
| `DESIGN.md` | Design-Dokument: Säulen, Runde, Wirtschaft, Einheiten, Zombies, Karte, Technik, Fahrplan, offene Fragen |
| `docs\trains.md` | Plan: Züge ausbauen – Gleisnetz mit Weichen, Haltestellen, Einfahrten, Züge mit ID per Skript |
| `docs\editor.md` | Plan: der Editor im Aufbau von SC2 mit allen TAB-Funktionen (Module Terrain, Objects, Zombies, Regions, Triggers, Events, Map) – jeder Schritt ein Startpunkt für eine neue Unterhaltung |
| `docs\trigger-und-28yl.md` | Schritte zu Triggern wie SC2, geprüft an TABs Workshop-Kampagne „28 Years Later“ – jeder Schritt ein Startpunkt für eine neue Unterhaltung |
| `Cargo.toml` / `Cargo.lock` | Rust-Projekt und Bibliotheken (`macroquad`, `egui-macroquad`, `serde`/`serde_json`, `libm`, `rayon`) / ihre genauen Versionen |
| `build.ps1` | Baut beide Programme nach `dist\LastBastion.exe` und `dist\LastBastionEditor.exe` |
| `data\` | **Alle Spielwerte** als JSON (Einheiten, Gebäude, Zombies, Lärm, Streuner, Villages of Doom, Beute, Kolonie, Spawn, Schwierigkeiten, Spieldauer, Karten, Forschung, welcher Ton zu wem) – erklärt in `data\README.md` |
| `assets\` | **Bilder und Töne**, je Art ein Ordner: `units\`, `zombies\`, `animals\`, `buildings\`, `decor\`, `terrain\`, `effects\`, `projectiles\`, `ui\`, `trains\`, `sounds\` (TABs, nur lokal), `test\` (unser Testbild) – erklärt in `assets\README.md` |
| `examples\test_sprites.rs` | Malt das graue Testbild `assets\test\figure.png` (`cargo run --release --example test_sprites`) |
| `tools\tab_skins.py` | Macht aus deinem entpackten TAB die Skins in `assets\units\`, `assets\zombies\` und `assets\animals\` (Raben, Ratten) (nur lokal, nie in git) – erklärt in `assets\README.md` |
| `tools\tab_terrain.py` | Macht aus deinem TAB die Landschaftsbilder in `assets\terrain\` (Boden, Wasser, Bäume, Felsen, Kräuter, Erz, Klippen – je Landschaft) |
| `tools\tab_buildings.py` | Macht aus deinem TAB die Gebäudebilder in `assets\buildings\` (jedes Gebäude in Teilen: Körper, Licht, Maschine, Fahnen, Rauch; Mauerstücke, Dorfhäuser, TABs Gebäude, die wir noch nicht haben) und die Deko in `assets\decor\` (Weizen, Beute, Ruinen, Strommasten, Schienen) |
| `tools\tab_ui.py` | Macht aus deinem TAB die Bilder der Leiste in `assets\ui\` (Leiste, Knöpfe, Icons, Porträts, Ereignisse, Mauszeiger, Menü-Hintergründe) |
| `tools\tab_effects.py` | Macht aus deinem TAB die Effekte in `assets\effects\` (Blut, Mündungsfeuer, Explosionen, Flecken, Feuer, Rauch, Kugeln, Wetter, Wasser, Lichter) und Pfeile und Rakete in `assets\projectiles\` |
| `tools\tab_trains.py` | Macht aus deinem TAB die Züge in `assets\trains\` (Lok, drei Wagen, Räder – je 91 Blickwinkel) |
| `tools\make_logo.py` | Rendert unser eigenes Logo `assets\logo\last_bastion.png` (Messing-Zahnrad mit Bastionsturm, rote Banner, „LAST BASTION“ – nur Formen und eine Windows-Schrift, nichts von TAB) |
| `tools\tab_sounds.py` | Kopiert TABs Töne nach `assets\sounds\` (Geräusche, Stimmen, Meldungen, Umgebung, Musik; mp3 → ogg mit ffmpeg) |
| `src\main.rs`, `src\bin\editor.rs` | Die zwei Programme (je ein paar Zeilen) |
| `src\lib.rs`, `src\app.rs` | Liste aller Module / Fenster, Bildschirm-Wechsel, Kommandozeile |
| `src\game.rs` | Eine Karte auf dem Bildschirm: Takte, Zeichnen, Endbildschirm, Edit/Test im Editor |
| `src\sim\` | **Die Simulation** (deterministisch, zeichnet nichts): Befehle, Spawn, Zombies, Gedränge (`crowd.rs`), Fernkampf-Zombies (`ranged.rs`), fliegende Pfeile (`arrows.rs`), Villages of Doom und Beute (`doom.rs`), Einheiten, Kampf, Gebäude, Wirtschaft (`economy.rs`), Markt (`market.rs`), Schulden und Bank (`debt.rs`), Forschung (`research.rs`), Sprengfässer und Minen (`blast.rs`), Kegel- und Flächentreffer (`areas.rs`), Feuer (`fire.rs`), Deko-Ornamente (`ornaments.rs`), Scan (`scan.rs`), Runde, Tests |
| `src\placement.rs` | Alle Bauregeln an einem Ort |
| `src\data.rs`, `src\kinds.rs` | `data\` lesen / Einheiten- und Gebäudetypen |
| `src\aggro.rs`, `hunt.rs`, `flow.rs` | Wegfindung der Zombies und Einheiten (Bedrohungs-, Jagdkarte, Flow Field mit Sichtlinie) |
| `src\formation.rs`, `vision.rs`, `grid.rs` | Formationen / Sicht und Nebel / räumliches Raster |
| `src\map.rs` | Die Karte: Gelände, Erz/Öl, Gebäude, Weltgrenze, Sichtlinie |
| `src\mapgen\` | Kartengenerator: **TABs** (`tab.rs`, seine Rausch-Ebenen `layer.rs`) – dazu „Blank“, die leere Karte der Tests (nie angeboten) mit Startgebiet, Erz und Öl, Dörfern und Beute (`villages.rs`), Werkzeuge (auch Erde) |
| `src\mapfile.rs` | Eine Runde = eine Datei (`%APPDATA%\Last Bastion\maps\`) |
| `src\home.rs`, `menu.rs`, `widgets.rs`, `settings.rs` | Startbildschirm und Karten-Einstellung / Esc-Menü / Knöpfe und Regler / Einstellungen |
| `src\keys.rs`, `keymap.rs` | Welche Taste was tut (Standard, gespeichert in `settings.json`) / die Seite „Key mapping“ |
| `src\ui.rs`, `cursor.rs` | Maus und Tastatur → Auswahl, Gruppen, Kamera, Befehle / echte Windows-Maus |
| `src\hud.rs` | Die Leiste unten wie in TAB (Minimap, Uhr, Gruppen, Auswahl, Befehlskarte, Vorräte, Ereignisse) |
| `src\ui_art.rs` | TABs Bilder der Leiste (`assets\ui\`) laden, Mauszeiger zeichnen |
| `src\audio.rs` | TABs Töne (wie sein `SysSoundSystem`): Listen, Ort und Lautstärke, eine Stimme zur Zeit, Alarme, Musik und Umgebung; lädt auf einem eigenen Thread |
| `src\editor.rs`, `src\undo.rs` | Der Editor (egui): Menüs, Module, Panels, Statuszeile / Rückgängig und Wiederholen |
| `src\editor\objects.rs`, `thumbs.rs` | Modul Objects: Katalog aller Arten, Tabelle des Platzierten, Eigenschaften (eines oder mehrerer) / die kleinen Bilder dazu |
| `src\editor\terrain.rs` | Modul Terrain: wie die Malwerkzeuge über die Karte gehen (Pinsel, Linie, Rechteck, Füllen, Dichte), Bildwahl, Bereich, Generator, Landschaft – rechts Minimap, Ebenen („Show“), „Under the mouse“ |
| `src\sim\landscaping.rs` | Die Orders dazu: Glätten, Boden eines Bereichs kopieren/einfügen, Generator im Bereich (Wald, See, Berge, Erzfeld, Village of Doom), „Gebäude und Gleise frei lassen“ |
| `src\view.rs` | Welcher Teil des Fensters die Karte zeigt (im Editor: zwischen den Panels) |
| `src\picking.rs`, `src\sim\clipboard.rs` | Editor: Auswahl von allem (Box), Kopieren / Ausschneiden / Einfügen |
| `src\draw.rs`, `sprites.rs` | Zeichnen (Bilder, sonst Platzhalter) / Sprite-Sheets laden, Richtung und Animation wählen |
| `src\building_sprites.rs` | Gebäudebilder laden und zeichnen: drei Looks (arbeitet, aus, im Bau) aus Teilen, Licht leuchtend (addiert), drehende Geschütze, Mauer- und Torstücke, Turm mit Vorderwand |
| `src\effects.rs` | TABs Effekte: Blut, Mündungsfeuer, fliegende Pfeile, Explosionen, Flecken am Boden, wohin Ballista und MG zielen – aus dem, was die Simulation meldet (`src\sim\fx.rs`) |
| `src\effects\lasting.rs` | Bleibende Effekte aus dem Zustand der Welt: Feuer und Rauch an Gebäuden, Infektion, brennende Zombies, Wetter, Blätter, Wasser, Reichweitenkreis |
| `src\critters.rs` | Raben und Ratten – nur im Bild, nicht in der Simulation |
| `src\sim\rails.rs`, `trains.rs`, `src\train_sprites.rs`, `src\rail_planner.rs` | TABs Bahn: Gleisstücke und die Strecken daraus / der Zug (fahren, halten, überrollen) / seine Bilder mit Rädern und Schatten / der Gleisplaner des Editors |
| `src\ground.rs` | Die Karte mit TABs Landschaftsbildern: Boden per Maske und Shader, Bäume, Felsen, Kräuter, Erz wie TABs Kartenzeichner |
| `src\post.rs` | TABs Nachbearbeitung: die Karte erst in ein Bild, dann heller (× 1,5), mit Leuchten (Bloom) und dem Farbfilter der Landschaft auf den Schirm |
| `src\picture.rs`, `iso.rs` | Kartenbild für Minimap und Vorschau / isometrische Kamera |
| `src\noise.rs`, `rng.rs` | Rauschen für die Generatoren (auch TABs Simplex-Rauschen) / eigener Zufallsgenerator |

## Anbindung an die Windows-Toolbox

- Das Spiel ist ein eigenes Repo; die [Windows-Toolbox](https://github.com/Windows-Toolbox/Toolbox) hat nur eine
  Seite im Tab „Games“, die es startet (DESIGN.md 12.7).
- Dafür gilt: das Spiel liegt immer unter **`dist\LastBastion.exe`**, das Design heißt `DESIGN.md`, die Einstellungen
  liegen fest unter `%APPDATA%\Last Bastion` (DESIGN.md 11.4).

## Rust lernen

- **Einrichten:** `winget install Rustlang.Rustup` (Toolchain MSVC) + Visual Studio mit „Desktopentwicklung mit C++“
  (Linker und Windows SDK). Im Editor **rust-analyzer**.
- **Starten:** `cargo run` (Spiel) / `cargo run --bin editor` (Editor) – Debug-Build mit Konsole.
  `cargo build` baut beide (dann findet der Knopf „Map editor“ den Editor).
- **Fertig bauen:** `.\build.ps1` → `dist\` (Release, ohne Konsole). Nach einer kleinen Änderung ≈ 4 s. Geht auch,
  während das Spiel läuft: die laufende Exe wird zu `*.old.exe` umbenannt, die neue startet beim nächsten Mal.
- **Tests:** `cargo test` – läuft nur, wenn man es aufruft (nicht im Spiel, nicht in `build.ps1`), dauert ≈ 2 s.
- **Lesen:** [Rust-Buch](https://doc.rust-lang.org/book/), [auf Deutsch](https://rust-lang-de.github.io/rustbook-de/)
  – wichtig: Kap. 1–10, später 13 und 15. Zum Üben [Rustlings](https://github.com/rust-lang/rustlings).

## Prototyp ausprobieren

### Die Runde (`dist\LastBastion.exe`)

- **Start:** 4 Ranger, 1 Soldat, ein Kommandozentrum, 100 Gold, 20 Holz – sonst nichts (Schwierigkeit Normal, jede
  anders: `data\game_difficulties.json`).
- **Länge wie TAB:** 80 / 100 / 120 / 150 Tage, dazu 20 zum Ausprobieren (1 Tag = 60 Sekunden, 100 Tage ≈ 1 h 40 min,
  `data\game_durations.json`). Abgerechnet wird alle 28 s (wie TAB).
- **Ziel:** Wirtschaft aufbauen (Holz, Nahrung, Strom, Stein), Zelte (Gold und Arbeiter), Kaserne (nur sie bildet aus),
  Einheiten, Mauern, Türme.
- **Gewonnen wie TAB nach Zeit:** am Ende des letzten Tags, wenn das Kommandozentrum noch steht (die Schlusswelle
  kommt bei 92 %); **verloren**, wenn das Kommandozentrum fällt oder **infiziert** wird (wie TAB: sein Schild von
  2.500 bricht – bei halbem Leben) → Endbildschirm (Restart, „Look at the map“, Main menu).
- **Geschichte der Karte** (aus dem Editor, Modul „Triggers“): Fenster und Auswahlen pausieren das Spiel, die Ziele
  stehen oben links, die Kommandozentrale kann einen eigenen Knopf haben (V).
- **Zug wie TAB** (Skript-Aktion „Train“, Gleise aus dem Editor): Lok + drei Wagen kommen vom Streckenende näher am
  Kartenrand, halten 3 s am Kommandozentrum („The train has arrived.“, dann das Ankunfts-Skript) und fahren bis zum
  Prellbock – liegt er außerhalb der Weltgrenze, ist der Zug dort weg, sonst bleibt er stehen. Jedes „Train“ schickt
  einen eigenen Zug (Abstand halten noch nicht: sie fahren durcheinander durch). Fahrend überrollen sie alles (100 Schaden je Takt, eigene Einheiten auch), stehend schieben sie zur Seite.
  Auf und neben Gleisen wird nichts gebaut – nur ein Tor quer darüber.
- **Eine Runde = eine Datei** (wie TAB) unter `%APPDATA%\Last Bastion\maps\`. **Restart** liest sie neu (Änderungen aus
  dem Editor gelten sofort). Speichern mitten in der Runde kommt später (DESIGN.md 11.5).

### Startbildschirm und Karten-Einstellung

- New game, Map editor, **Custom maps** (alle Dateien: Play / Edit / Delete), Settings, **Key mapping**, Quit – vor
  TABs Menübild (Himmel, Horde, Kolonie; bewegt sich leicht mit der Maus).
- **New game** (wie TAB): **Karte**, **Schwierigkeit** und **Dauer** wählen, Seed, Vorschau – dazu der
  **Score-Faktor** (Karte × Schwierigkeit × Dauer; eine Punktzahl gibt es noch nicht).
  - **Custom levels** (zweiter Reiter, wie TABs „Custom Levels“): jede **im Editor gespeicherte** Karte – nicht die
    Dateien, die jede neue Runde anlegt. Ihre Runde steht in der Datei (Tage, Schwierigkeit, Wellen, Start), also nur
    Vorschau (oder ihr eigenes Bild), Autor, Beschreibung, „Start game“ und „Edit in the editor“. Mit „New map every
    start“ macht das Spiel die Karte bei jedem Start neu (wie TABs zufällige Generierung) – Runde und Events bleiben.
  - **Karten** (`data\game_maps.json`, je zwei nebeneinander): zuerst **TABs sechs Landschaften** mit TABs Generator,
    Namen und Score-Faktoren: FA The Deep Forest 80 %, **BR The Dark Moorland** 100 % (Standard), TM The Peaceful
    Lowlands 115 %, AL The Frozen Highlands 135 %, DS The Desolated Wasteland 155 %, VO Caustic Lands 175 %.
    **Landschafts-Effekte** wie TAB gelten für die ganze Runde und stehen unter der Vorschau: **AL** (Kälte) Zombies hören
    halb so gut, alle laufen 80 %, Gebäude brauchen 130 % Strom; **DS** (Hitze) Zombies hören 130 %, alle sehen 110 %.
    Dazu je Landschaft TABs Jagd auf Wald / Gras / Erde (`modifiers` in `data\game_maps.json`). Andere Karten gibt es
    nicht mehr – unsere alten Generatoren (Wild, Regions, Rings, Canyons) sind gelöscht; ihre alten Kartendateien öffnen
    sich als TAB-Karten (das Gelände steht in der Datei).
  - **Schwierigkeit** (Easy / Normal / Hard / Brutal, `data\game_difficulties.json`): Startgold, -holz und -einheiten,
    Streuner, wie nah jede Art ans Kommandozentrum darf, Wächter der Beute – und TABs **Bevölkerung** (0,6 / 1 / 1,2 /
    1,5): so stark die Wellen, so viele Zombies ein Dorfhaus hat, auf TABs Karten statt Streuner und Abständen.
  - **Dauer** (`data\game_durations.json`): 20–150 Tage – TABs Wellenplan wird darauf gestreckt.
- **Im Editor** zeigt **„New map“** dieselbe Liste von TABs sechs Karten (Knopf „Open in the editor“).
- **Custom setup…** (in beiden): TABs Generator mit seinen Reglern (Maus drauf = Erklärung), Schwierigkeit und Dauer.
  „Random values“ würfelt, „Defaults“ setzt zurück. Die letzten Werte merkt sich das Spiel.
- **TABs Generator** (Rausch-Ebenen wie TAB, mit TABs Simplex-Rauschen): Land/Wasser, Berge in 2 × 2-Blöcken, Gras,
  Wald auf Gras und auf Erde. Das **Kommandozentrum** steht auf dem freien Platz nächst der Mitte; weniger als 18 Wald
  in 11 Kacheln, kein Stein (4) oder Eisen (10) in der Nähe oder eingesperrt (unter 30 % der Fläche erreichbar) →
  neuer Seed. Was es nicht erreicht, bleibt leer. **Dörfer** mit einem Haus in der Mitte und Häusern in einer Spirale
  (klein, mittel, groß); **Ruinen** nahe der Basis mit 2–4 Haufen Beute (Gold oder eine Ware, weiter draußen
  wertvoller); **Stein, Eisen, Kohle, Gold** (gelb) als Flecken auf freier Erde – Kohle ist unsere (TAB hat keine),
  wie Eisen gelegt, mindestens 10 Kacheln nahe der Basis, nie auf Stein oder Eisen; **Öl** eine Quelle pro Block. **Zombies** aus einem Rauschen: je höher, desto mehr auf einer Kachel (1–3), weiter draußen
  stärkere Arten; **Riesen** ab 70 Kacheln. Die Zahlen jeder Landschaft sind **TABs eigene** (`MapThemes` in
  `ZXRules`). Dazu wie TAB **Schatz-Ruinen** unter der Beute und zwei **Strommasten** in 5 × 5-Wäldern (nur Bild,
  in der Kartendatei als `ornaments`). Wie die Ruinen TABs **Vorräte:** ein Strommast mit 2 × Strom, ein
  brennender Lastwagen mit 2 × Nahrung (je Landschaft verschieden viele). Nicht übernommen: alte Türme, Mutanten.
- **Weltgrenze:** gerade auf dem Bildschirm wie in TAB – die Spielfläche ist ein Rechteck (√2-mal so breit wie
  hoch, TABs Kachelform), die Minimap auch. Außen ein Ring offenen Geländes (≈ 17 Kacheln) mit den **Spawnern** außen am Rand (3 pro Seite, fast die
  ganze Seite breit) – dazwischen Platz für Gleis, auf dem ein Zug ungesehen hereinkommt. Im Spiel sieht man den Ring nicht, Einheiten kommen nicht hinein, bauen geht dort nicht. Im Editor ist er sichtbar und bebaubar; das Schwarze dahinter gehört nicht zur Karte. Größe = so viele Kacheln wie ein Quadrat dieser Kante.
  Im Editor lässt sich die Spielfläche überallhin ziehen und beliebig groß machen – Kamera und Minimap folgen ihr.
- **Nebel:** wie TAB eine weiche Schicht über allem auf der Karte: nie Gesehenes schwarz, Erkundetes samt Bäumen und
  Felsen nur gedimmt (TABs 70 %, Dunkles Moor 78 %), um jeden, der sieht, ein weicher Lichtkreis. Zombies nur, wo
  jemand hinschaut. In einen Wald sieht man 3 Kacheln tief, durch ihn nicht – schießen geht nur bis zum Waldrand.
- **Erz:** Eisenerz (rostbraun), Kohle (schwarz), Stein (hellgrau) als Felder; neben der Basis je ein kleines (Stein
  12–16 Kacheln weg, Eisen und Kohle 14–24 – ein, zwei Tesla-Türme), draußen größer und reicher. Abgebaut von der **Mine** –
  am besten mitten aufs Feld, sie baut auch das Erz unter sich ab. **Auf Erz** stehen nur **Verteidigung** (Mauern,
  Tore, Türme, die selbst schießen, Fallen) und was es abbaut (Mine, Steinbrüche) – unsere Abweichung, TAB baut gar
  nichts darauf. Unter so einem Bau bleibt das Erz zu sehen (kein Pflaster), und das Pflaster rund um Gebäude reicht
  nie aufs Erz.
- **Gold** (gelb, nur auf TABs Karten): eine Mine darauf bringt wie in TAB das 20-Fache – 10 Gold pro Kachel und
  Abrechnung, direkt ins Gold (zählt als Einnahme der Mine).
- **Öl:** Quellen von genau **2 × 2 Kacheln** (lila mit schwarzer Lache), berühren sich nie. Neben der Basis eine
  einzelne, draußen Gruppen von 2 bis 6. **Niemand läuft hinein:** Wege gehen um die Quelle herum, Einheiten und
  Zombies halten Abstand von einem Kreis in ihrer Mitte (die Ecken sind frei). Darauf steht nur die **Ölplattform**.

### Zombies und Wellen

- **Wellen = TABs Events** (aus dessen Code, `src\sim\events.rs`, Plan in `data\spawning.json`, für 100 Tage – bei
  anderen Tagen gestreckt, Anzahlen × Bevölkerung):
  - **Streuner:** ab Tag 3 alle 2 Tage ein paar Walker von **jeder** Seite (10×, jedes Mal +10 %), ab Tag 26 alle 6
    Tage Runner und Riots – still, ohne Ankündigung.
  - **Schwärme:** ab Tag 14 alle 10 Tage von **einer** zufälligen Seite (16–24 Runner, jedes Mal +300 %: 20, 80, 140,
    200), ab Tag 51 alle 7 Tage die harten (Runner, Riots, Fat – bis zum 15-fachen). **8 Stunden vorher angekündigt**
    („A horde of infected is coming from the north“), Countdown an der Uhr, aus einem Quadrat von 8 bzw. 20 Kacheln.
  - **Schlusswelle** an Tag 93, einen Tag vorher angekündigt: von allen 4 Seiten je 2.200 Runner, 550 Riots und je
    50 Fat, Harpyien, Spitter. Sie **ruft alle Zombies der Karte** (auch Dörfer und Beute – sie gehören dann dazu)
    und **deckt die Karte auf**.
  - Die Zombies kommen **nicht auf einmal**, sondern wie bei TAB als **Kolonne** – jede Seite ein paar alle 0,15 s.
  - Getötete Zombies kommen nie wieder.
- **Auf TABs Karten** legt sie der Generator (oben): auf 256 und Normal ≈ 13.000–25.000 je nach Landschaft (TABs
  `InfectedFactor` 0,35–0,45), bis 34 Kacheln um die Basis ausgedünnt, Runner und Riots ab 52, Fat, Harpyie und
  Spitter ab 92 (BR). Der Rest dieses Abschnitts gilt für unsere Karten.
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
- **Arten** (Werte wie TAB): **Walker** (langsam, 35 HP), **Runner** (schnell, 45 HP), **Fat** (groß, 500 HP),
  **Riot** (TABs gepanzerter Runner: Rüstung 0,1 – ein Pfeil macht 9 statt 10), **Spitter** (TABs Venom: spuckt
  **Gift** 4 Kacheln weit, über Mauern; es fliegt kurz und trifft alle im Umkreis von 0,75 Kacheln – wer rechtzeitig
  weggeht, entkommt; keine Rüstung hilft, Soldaten nehmen nur die Hälfte), **Infested** (SC2s
  Infested Marine: schießt 4,5 Kacheln, knapp unter dem Soldaten), **Harpyie** (TABs Harpy, gelb: schnell, ihr
  hört 2–4× sicherer als die anderen und 32 Kacheln weit, kommt zu jedem Kampf als Erste; sie schlägt alle 0,3 s zu,
  jeder Treffer macht Lärm 10 – schneller, als er sich halbiert, also wird es laut – zuerst erschießen! **Springt über Mauern und
  Tore** bis 3 Kacheln dick (0,6 s in der Luft), wenn ihr Ziel dahinter liegt; eine dickere Mauer schlägt sie ein),
  **Riese** (wie TAB: 10.000 HP, Rüstung 0,3, 160 Schaden, rennt 4 Kacheln/s – eine Mauer in 3 Schlägen; **jeder Schlag
  trifft alle vor ihm** in 120° und 3,5 Kacheln – Einheiten, Gebäude, auch Zombies; **wen er tötet, der fliegt weg** (TABs
  Rückstoß, nur Tote: ein Walker oder Ranger gut 3 Kacheln, ein Fat kaum); nur ein paar pro
  Karte weit draußen – **wie viele, sagt die Karte** (Einstellung „Giants“, Standard 4 auf 256er-Karten, Peaceful
  Lowlands 2, Broken Lands 5, Open Plains 6), die Schwierigkeit macht daraus 50 % bis 225 % – die Schlusswelle ruft
  auch sie). Weiter draußen mehr von den harten Arten.
- **Schlafen weit weg** (DESIGN.md 8.5): Wer weit weg von deinen Augen liegt, schläft – kostet fast nichts. Wach werden
  sie, wenn eine Einheit oder ein Gebäude auf Sichtweite + 8 Kacheln herankommt (bevor du sie siehst), durch Lärm
  (genau wie die wachen – und gehen hin), durch Scan, Sprengfass und die Schlusswelle (alle, 2.000 pro Takt, die
  nächsten zuerst). Wer 20 s ruhig und weit weg von allen Augen ist, schläft wieder ein.
- **Auch Schlafende wandern** – nach denselben Würfeln wie die wachen (nur kommen sie sofort an und warten dann, so
  lange der Weg gedauert hätte). Ein Ausflug, der nahe deiner Augen endet, weckt sie unterwegs: Sie laufen wach von
  außen herein. Mit TABs kurzen Ausflügen füllt sich ein leergeräumter Ort aber **nur langsam** wieder. Eine Welle, die
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
  **wandern** (150), im Gehtempo. Stehen und Knurren dauern so lange wie TABs Animationen (Walker 2,35 / 2 s). Ein
  Ausflug geht wie TAB auf jeder Achse für sich bis knapp 2 Kacheln weit (Harpyie 1–3, Riese 2–4; ein Quadrat, kein
  Kreis), ohne festen Platz. Der **Riese brüllt** alle 12 s und bleibt dafür stehen, egal was er tut.
- **Villages of Doom** (wie TAB, aus dessen Code, `src\sim\doom.rs`): dunkle Häuser mit rotem Dach, klein (2 × 2),
  mittel (3 × 3) oder groß (5 × 5). Wo und wie viele: TABs Generator bzw. die Karte (Green Valley ≈ 10 Dörfer mit 3–6
  Häusern, Broken Lands **2 riesige** mit 15–25). Die Zombies liegen nicht drin – sie **entstehen in Schüben**, wenn
  etwas passiert, bis ein Haus leer ist (wie TAB: klein 40, mittel 150, groß 500 – mal der Bevölkerung). Ein Schub ist etwa
  1 / 2 / 5 Zombies (zufällig halb bis anderthalb), höchstens einer pro Sekunde und Haus, das Dach leuchtet dabei rot:
  - **Lärm** in 20 / 24 / 32 Kacheln (je Größe): ein halber Schub geht hin.
  - **Alle 12 Stunden:** stehen zu wenige ums Haus, bleibt ein Schub als **Wache** dort.
  - **Ab Tag 20 alle 5 Tage** (jedes Haus an seinem Tag): ein Schub **marschiert zum Kommandozentrum**.
  - **Getroffen:** drei Schübe gehen auf die nächste Einheit in 10 Kacheln, jedes Nachbarhaus schickt einen mit.
  - Eine Einheit direkt daneben weckt nichts. Zombies: klein Walker, mittel Runner, groß Runner, Riots und selten Fat.
  - Haus abschießen (400 / 1.500 / 4.000 HP): es lässt Beute fallen (1.000 Gold oder 20 Holz/Stein/Eisenerz/Kohle) – wer noch
    nicht draußen war, kommt nie.
- **Beute** (≈ 40 Haufen): Gold (500) oder 10 Holz/Stein/Eisenerz/Kohle – drauflaufen hebt sie auf. Um jeden Haufen
  liegen Zombies versteckt (Normal 8–50, mehr weiter draußen), die auf 5 Kacheln oder bei Lärm aus dem Boden kommen.
  **Vorräte** (Strom, Nahrung, je 20) wie TAB: aufgehoben macht das Kommandozentrum für den Rest der Runde 20 mehr.
- Häuser und Beute bleiben auf der Karte sichtbar, sobald man sie einmal gesehen hat, auch auf der Minimap (rot).
- **Der Entscheidungsbaum wie TAB** (aus dessen Code, DESIGN.md 8.1): Ziel in Sicht → hin und zuschlagen; aus den
  Augen verloren → zu dem Platz, stehen bleiben; Welle → zum Kommandozentrum; Lärm → hin; sonst Leerlauf.
- **Ziele:** wie TAB schaut sich jeder alle 1 s um (Walker und Riese 1,5 s, Harpyie 0,67 s; mit Ziel doppelt so selten, außerhalb deiner Sicht 3×, im Unerkundeten
  5× so selten) – **Sehen** 5–8 Kacheln, nicht durch Wald. Ziele sind nur **deine Einheiten und Gebäude mit Leuten**
  (Kommandozentrum, Häuser, Gebäude mit Arbeitern, Türme mit Einheiten) – **keine Mauern, leeren Türme, Strommasten**.
- **Wand-Regel:** Steht eine Mauer, ein Tor oder Turm auf der geraden Linie zum Ziel, reißt er **die** ein. Niemand
  sucht eine Lücke. Die Harpyie springt stattdessen.
- **Angeschossen:** Er und alle ohne Ziel auf seiner Kachel und den vier daneben gehen auf den Schützen los (sitzt der
  im Turm: auf den Turm) – auch außer Sicht, dann zu seinem Platz.
- **Zuschlagen wie TAB:** Ein Zombie holt aus (`windup_seconds`, in TABs Takt: Walker, Runner, Fat 0,53 s, Riot 0,4,
  Spitter 0,53, Harpyie und Riese 0,27) und steht, solange sein Angriff dauert. Ist die Einheit am Ende des Ausholens aus seiner
  Reichweite, geht der Schlag ins Leere – Ranger können zwischen Walkern hindurch kiten. Schüsse (Spitter, Infested)
  fliegen trotzdem, der Hieb des Riesen trifft den ganzen Bogen.
- **Ansteckung:** Stehen auf der Kachel eines Zombies mindestens 3 Wellen-Zombies mit gleichem oder höherem Rang
  (wie TAB: Walker 1, Runner 2, Riot 3, Fat/Harpyie/Infested 4, Spitter 5), läuft er mit – eine Welle wächst auf ihrem
  Weg. Ist es die Schlusswelle, gehört er auch zu ihr.
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
  und **sinkt nie**: je 50.000 bringt jede Seite der Streuner **für immer** einen Zombie mehr. Die Fabrik
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
- **Schießen wie TAB:** Waffe heben (Ranger 0,5 s, Soldat 0,2 s), ausholen, Schuss, nachladen – mit TABs Zeiten und
  Animationen, **und in TABs Takt**: TABs Einheiten und Zombies denken nur alle 133 ms, jede Wartezeit endet erst
  dann, und der nächste Schuss beginnt einen Takt nach dem letzten. Der Soldat schießt so alle 0,8 s (nicht 0,5),
  der Ranger alle 1,2 s, der Walker schlägt alle 1,2 s zu – die Zeiten stehen so in `data\`, frei änderbar. Ohne Ziel sucht eine Einheit nur alle 0,35 s ein neues – im Angriffsmarsch und auf Patrouille jeden
  Tick, sonst liefe sie erst ein Stück weiter.
- **Animation Cancelling:** Losgehen beim Ausholen bricht den Schuss ab, Losgehen nach dem Schuss kostet nichts
  (nachgeladen wird beim Gehen weiter). Beim Gehen sinkt die Waffe – ein kurzer Schritt kostet wenig Heben. Jeder neue
  Befehl (auch derselbe Angriff nochmal, auch Stopp) fängt das Ausholen von vorn an.
- **Eigene angreifen:** T + Klick auf eine eigene Einheit oder ein Gebäude schießt darauf, bis es weg ist (Medics:
  Strahl auf die Einheit). Gebäude: erst Schild, dann Lebenspunkte – eigene Schüsse infizieren nie, aber ohne Schild
  infiziert der nächste Zombietreffer sofort (und macht Schaden). Reparieren macht es wieder ganz. Von selbst schießt
  niemand auf eigene.
- **Im Bau** wächst erst das Leben, ab vollen Lebenspunkten der Schild. Infiziert werden kann es erst fertig – vorher
  wohnt und arbeitet niemand darin.
- **Pfeile fliegen** wie in TAB (12 Kacheln/s, im Bogen mit Schatten am Boden) und treffen erst beim Ankommen; der
  Ranger zielt dorthin, wo der Zombie sein wird. Ein Pfeil unterwegs zählt schon: andere schießen nicht auf denselben
  todgeweihten Zombie.
- **Speerträger** (Kaserne, 50 Gold, ohne Munitionsfabrik): Nahkampf mit 1,2 Kacheln Reichweite, **jeder Treffer stößt
  den Zombie zurück** (0,6 Kacheln, große weniger: Fat ≈ die Hälfte, Riese ≈ ein Viertel), **fast lautlos** (Lärm 2 pro
  Stich – etwas mehr als ein Zombie-Treffer, weniger als ein Gewehrschuss). Hält früh eine
  Linie oder ein Tor – gegen Spitter und Infested stirbt er.
- **Veteranen:** jeder Zombie, der stirbt, gibt Erfahrung (Walker 1, Riot/Spitter/Infested/Harpyie 3, Fat 10, Riese
  100), geteilt unter allen Einheiten in 8 Kacheln – Veteranen nehmen einen halben Anteil. Bei 100 wird eine Einheit
  **Veteran**: +25 % HP (sofort), +20 % Schaden, silberne Marke, desertiert dreimal seltener. Söldner lernen nicht.
- **Entlassen (Entf):** die ausgewählten Einheiten verlassen die Kolonie – kein Unterhalt mehr, Arbeiter frei, die
  **Hälfte ihres Holzes und Eisens** zurück (aufgerundet: Söldner-Ranger 5 Holz → 3), kein Gold.
- **Selbstheilung** (wie TAB): 2 s nach dem letzten Treffer heilen sich Einheiten **und Zombies** jede Sekunde um
  einen Anteil ihrer HP (Soldat 4 %, Ranger 1 %; Walker 8 % = 3 HP/s, Riese 0,5 % = 50 HP/s) – auch im Schlaf. Einen
  Zombie anschießen und laufen lassen bringt also nichts.
- **Lucifer** (Flammenwerfer): Feuerkegel 3,5 Kacheln, 20° breit – trifft **alles darin, auch eigene Einheiten**, und
  zündet 70 % der Zombies an (brennen 3 s, je ½ s 5 Schaden ohne Rüstung). Selbst feuerfest.
- **Thanatos:** Rakete 3–10 Kacheln weit, explodiert (1,8 Kacheln, außen 20 %), zielt 2,4 s; näher als 1,5 Kacheln
  schlägt er mit dem Werfer (120°-Kegel). Dazwischen trifft er nichts. Sehr laut.
- **Titan:** 9 Kacheln, harte Schüsse mit kleiner Fläche (0,8), 800 HP, schnell.
- **Söldner** aus dem Gasthaus (Söldner-Soldat, -Ranger, -Scharfschütze): stärker als Veteranen (×1,5 HP, ×1,4
  Schaden, schneller), 2,5 × so teuer, doppelter Unterhalt, **keine Arbeiter** – das Limit ist nur das Gold. Goldene
  Marke über dem Kopf. Bei Schulden gehen sie zuerst, schon ab Stufe 2.
- **Sprengfass tragen:** Rechtsklick auf die Munitionsfabrik – jede Einheit nimmt eins (30 % langsamer, schießt
  nicht), **D** stellt es ab. Stirbt der Träger, geht es hoch.
- **Größe** (wie TAB): eine Einheit braucht 0,5 Kacheln (Walker/Runner 0,4, Fat 0,7, Riese 1,6). Formationen stehen
  0,5 Kacheln auseinander.
- **Aussehen:** mit TABs Bildern (nur auf deinem PC, `tools\tab_skins.py`) trägt jede Einheit und jeder Zombie einen
  eigenen **Skin** (Walker 3, Runner 4 Varianten) – Stehen, Gehen, Rennen, Angriff, Knurren, Sterben, 10 Richtungen
  gespiegelt wie TAB. Der Medic trägt den silbernen Soldaten, der Speerträger den Marine (beide ohne
  Angriffsbild: sie stehen, während sie heilen oder stechen). Ohne Bilder: die graue Testfigur. Getötete Zombies spielen 1 s ihren Tod, tote Einheiten liegen im
  letzten Bild, bis sie aufstehen.
- **Landschaft wie TAB** (mit `tools\tab_terrain.py`, nur auf deinem PC): Erde, Gras weich drübergeblendet, Wasser,
  Felsstücke auf jedem 2 × 2-Berg (tief im Gebirge größer), ein Baum pro Waldkachel (auf Gras andere als auf Erde,
  tief im Wald größer), Kräuter, Erzbrocken – jede TAB-Karte in ihrer Landschaft (`look` in `game_maps.json`), unsere
  Karten wie BR. Öl als TABs Ölquelle (2 × 2); Kohle hat TAB nicht: unsere sind seine Steinbrocken, schwarz und zerbrochen (`tab_terrain.py`). **Klippen** um alles Wasser wie TAB: Felswände, die ins Wasser
  abtauchen, je nach Ufer-Form und je Landschaft getönt. Ohne die Bilder: die farbigen Rauten wie bisher.
- **Farben wie TAB** (`src\post.rs`): die Karte geht wie bei TAB durch dessen Nachbearbeitung – heller, helle Stellen
  leuchten weich nach, dazu der Farbfilter der Landschaft (Caustic Lands türkis, Moor und Tiefer Wald warm, Hochland
  kalt). Auch im Editor (dort, ohne das Licht der Spieler-Sicht, nur × 1,1 heller); die Leiste unten und die
  Lebensbalken bleiben ungefiltert (durch das Leuchten glühten sie).
- **Gebäude wie TAB** (mit `tools\tab_buildings.py`, nur auf deinem PC): jedes Gebäude und jedes Dorfhaus als TABs
  Bild – arbeitend mit Licht, laufender Maschine, Fahnen und Rauch; aus/infiziert blass ohne Licht; im Bau als grüner
  Geist. Zelte, Steinhäuser, Pfähle, Fässer, Dorfhäuser in TABs Varianten, die Fischerhütte schaut zum Wasser. Mauern
  verbinden sich wie bei TAB (nicht mit Türmen), das Tor öffnet seine Tür, Einheiten stehen im Turm hinter der
  Brüstung. Köhlerei und Munitionsfabrik (hat TAB nicht) tragen TABs Kraftwerk und Lagerhaus. Ohne die Bilder: Klötze.
  Jede Maschine, Fahne und Rauchfahne läuft mit **allen** TAB-Bildern im eigenen Takt, Licht leuchtet (wird addiert).
  **Ballista und MG drehen sich** zu ihrem Ziel (240°/s wie TAB), die Ballista spielt dabei ihren Schuss.
- **Effekte wie TAB** (mit `tools\tab_effects.py`, nur auf deinem PC): Mündungsfeuer bei Soldat, Scharfschütze und MG,
  Pfeile von Ranger und Ballista fliegen, getroffene Zombies bluten, Tote hinterlassen eine Blutlache (wächst 1 s,
  verblasst 10 s), Säure spritzt grün, Fässer explodieren im Feuerball, Gebäude fallen in Holz- oder Steintrümmern
  (`explosion` in `data\buildings.json`), Infizierte mit TABs Kristall-Effekt. Ohne die Bilder: Linien wie bisher.
- **Bleibende Effekte** (`src\effects\lasting.rs`): Gebäude, deren Schild weg ist und die Leben verloren haben,
  brennen und rauchen (unter einem Viertel mehr), mit schwachem orangem Schein – Gebäude ohne Schild (Mauern, Tore,
  Türme) erst unter 5 % ihrer Leben, kurz vorm Einsturz; nur Bild – kein Schaden, kein Lärm; infizierte qualmen grün; brennende Zombies
  brennen. **Wetter** wie TAB: eine Runde auf dem
  Frozen Highland (AL) beginnt mit Schnee, auf den Caustic Lands (VO) mit Regen – ab dann **steuern es Skripte**
  (Aktion „Weather“: Snow / Rain / Clear, beim Start, mit einem Event, per Zufall oder Auswahl); es blendet über 4 s
  über, der Regen-Klang mit. Schnee und Regen mit allen TAB-Bildern (30 pro Sekunde). Blätter fallen im Deep Forest
  (FA); Wasser schimmert.
- **Deko wie TAB:** **Weizen** auf den Feldern der Farm, **Beute** als TABs Kisten und Säcke, **Schatz-Ruinen** unter
  der Beute (TABs Generator), zwei **Strommasten** in großen Wäldern, **Raben** nahe der Basis (fliegen auf, wenn jemand
  bis 2 Kacheln kommt – oder einfach so) und **Ratten** an Ruinen und Dorfhäusern (rennen weg). Nur Bild, nicht in der
  Simulation.
- **Töne wie TAB** (`src\audio.rs`, welcher zu wem: `data\audio.json`; die Dateien mit `tools\tab_sounds.py`, nur
  lokal):
  - **Schüsse, Explosionen, Tode, Brüllen** dort, wo es passiert – nur auf dem Bildschirm, leiser zum Rand und
    herausgezoomt; Zombies brüllen beim Knurren und Rennen, der Riese stampft.
  - **Stimmen:** Auswahl („What“), Befehl („Yes“), Angriff („Attack“), ausgebildet, Tod, Desertieren – immer nur eine.
    Ein gewähltes Gebäude spielt sein Geräusch, Platzieren sein „Bauen“.
  - **Der Offizier:** Welle angekündigt (wie viele, dann woher), Einheiten / Gebäude / Mauern angegriffen, Gebäude
    infiziert (nur wenn die Stelle nicht auf dem Bildschirm ist, je Art höchstens alle 10 s), Gebäude fertig,
    Forschung fertig, neue Gebäude / Einheiten.
  - **Musik und Umgebung:** Intro im Startmenü; in der Runde der Klang der Landschaft (Regen dazu, solange es regnet, Wasser
    lauter über Wasser), „Game Start“; vor einer Welle der Countdown, dann ihre Musik, danach ihr Ende und die
    Landschaft kommt zurück; Sieg und Niederlage.
  - **Oberfläche:** Klicks, „ungültig“, Pause, Fenster, Meldungen, Veteran, Wunder fertig. Raben und Ratten.
  - **Lautstärken** wie TAB in den Settings (rechte Spalte): **Master volume** (alles auf einmal), darunter Musik, Effekte, Stimmen, Alarme, Umgebung,
    Oberfläche. F3 zeigt, wie viele Töne geladen sind.
- **Getragenes Fass** wie TAB: das Fass-Bild hinter der Einheit, etwas angehoben, mit Schatten.
- **Schießen wie StarCraft:** drehen → zielen → schießen → nachladen. Eine Einheit bleibt bei ihrem Ziel.
  Schüsse bis zum Tod (TABs Werte): Walker Ranger 4 / Soldat 3 / Scharfschütze 1, Runner 5 / 3 / 1.
- **Feuer verteilen:** neue Ziele sind Zombies, die die anderen nicht schon sicher töten.
- **Medic:** greift nie an, heilt mit seinem Strahl 4 HP/s – aber **nur, wer 3 s nicht getroffen wurde**: kein
  SC2-Heilen mitten im Kampf, Verletzte muss man zurückziehen.
- **Formationen** (wie *Diplomacy is Not an Option*): nur beim **Ziehen** – Linie, Block, Kreis.

### Die Leiste unten (wie TAB)

- **TABs Leiste:** Anordnung aus TABs `MapGUI` (1280 × 720), mit TABs Bild, Knöpfen, Icons, Porträts und
  Mauszeigern (`tools\tab_ui.py`). Ein größeres Fenster macht alles gleich viel größer; breiter als 16 : 9 → die
  Auswahl wird breiter. Rohre und Zahnräder oben auf der Leiste gehören zur Leiste, die Lücken dazwischen zur Karte.
  Ohne TABs Bilder: dieselben Plätze in Messing-Platzhaltern.
- **Minimap** (klicken/ziehen = Kamera, Rechtsklick = hinlaufen), quadratisch wie TABs – die Spielfläche ist breiter
  als hoch, oben und unten bleibt ein Streifen. Immer sichtbar, auch im Nebel: jeder **Riese** und jeder **Schwarm
  einer Welle** (TABs Icons, größer je mehr er hat – eine Welle von zwei Seiten sind zwei Schwärme; noch im
  Spawn-Ring: am Rand der Minimap). Darüber der **Countdown** der nächsten angekündigten Welle (Schlusswelle rot).
- **Uhr:** Tag, Stunde, runder **Pause-Knopf**, Tempo; dahinter drehen sich TABs Zahnräder mit der Spielzeit.
- **Kontrollgruppen** übereinander, 1 unten, 0 oben – jede mit dem Porträt ihrer häufigsten Einheit (nur Gebäude:
  des ersten) und der Anzahl.
- **Auswahl:** eine Sache = Porträt (Figuren 3 : 4, Gebäude ihr Bild), Name, Lebensbalken (HP + Schild in einem),
  Text, rechts die Werte in Kästchen; mehrere = Porträts (der aktive Typ hell).
- **Befehlskarte** 5 × 3 mit TABs Icons (Bauen und Ausbilden: das Porträt), Taste oben links, Preis unten rechts
  (rot, wenn das Gold nicht reicht), Tooltips.
- **Vier kleine Knöpfe** neben dem Lager: Kamera zum Kommandozentrum, alles reparieren (X), Raster, Menü.
- **Ereignisse** oben links wie TABs Liste (unter den Zielen der Karte): angekündigte und gekommene Wellen, Beitritte,
  Meldungen der Karte – je mit TABs Bild, 12 Sekunden lang.
- **Mauszeiger** wie TAB: Fadenkreuz auf einem Zombie (mit Einheiten) und wenn ein Befehl sein Ziel sucht, „Hinein“
  auf einem Turm mit Platz, „Geht nicht“ auf einem vollen.
- **Gold im Minus:** gelb („-250 of 500“), solange es im Dispo der Banken liegt, rot darüber.
- **Vorräte:** Kolonisten, **Arbeiter** (frei/alle), **Nahrung**, **Energie** (gebraucht/erzeugt), Gold, Holz, Stein,
  Eisenerz, Kohle, **Eisen**, **Öl** – jeweils mit „+/− pro Abrechnung“ (alle 8 Stunden); rot, wenn etwas knapp wird.

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
| T / P / M + Linksklick | Angriff (Attack-Move) / Patrouille / Laufen – ziehen = in Formation. T auf Zombie, eigene Einheit oder eigenes Gebäude: genau das angreifen |
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
| F4 | Gitter an/aus wie TAB: dünne Linien zwischen den Kacheln; Berge schwarz, Wald dunkelgrün (halb durchsichtig, weißer Rand), Bäume und Felsen dafür ausgeblendet – so sieht man, wo man nicht bauen kann (auch: Knopf neben der Pause, F3-Schalter, Settings) |

- **Die Tabelle zeigt die Standardtasten – jede ist änderbar:** Esc → **Key mapping** (auch im Startbildschirm).
  - 5 Tabs: Kamera und Zeit, Auswahl und Gruppen, Einheiten (Befehle, Formationen), Kommandozentrum (die Baumenüs und
    **jedes Gebäude darin**, wie in TAB), Gebäude (Ausbilden, Sammelpunkt, Reparieren, On/Off …).
  - Je Aktion **2 Tasten**. Kästchen anklicken → neue Taste drücken; Rechtsklick leert es, Esc bricht ab.
  - **Mäuse mit vielen Tasten:** Maustaste 4/5 direkt belegbar. Alle weiteren Tasten in der Maus-Software auf
    **F13–F24** legen – die gehen auch. Kommt eine Taste, die das Spiel nicht kennt, steht das rot unter der Liste.
  - **Rot** = zwei Aktionen haben dieselbe Taste auf einer Karte (oder beide überall). Maus auf der Zeile: was sonst
    auf der Taste liegt bzw. die Standardtaste. „Defaults for this tab / for all“ setzt zurück.
  - Fest bleiben **Shift, Strg** (Kette, 5 auf einmal, Gruppe setzen …), **Esc** und im Editor **Strg+S**, **Strg+Z** / **Strg+Y**.
  - Die Befehlskarte geht vor: eine Taste, die auch überall gilt, wirkt nur, wenn die Karte sie gerade nicht hat (wie
    X: Linie bei Einheiten, sonst alles reparieren). Kamera-Tasten, die auf einer Karte liegen, nur ohne Auswahl.
  - F3-Tastenhilfe, Befehlskarte und Tooltips zeigen immer die eingestellten Tasten. Gespeichert in `settings.json`
    unter `keys` – nur, was vom Standard abweicht.

### Bauen

- **Kommandozentrum:** ein **Baumenü pro Taste** (wie TABs Kategorien). **B** oder **Esc** zurück – oder das
  Kommandozentrum neu auswählen (Klick, Gruppe).
- **Nur was geht, steht auf der Karte:** ein Gebäude, eine Einheit erst, wenn ihre Forschung fertig ist und das
  Gebäude steht, das sie braucht; ein Baumenü erst, wenn etwas darin geht. Geplantes („soon“) gar nicht. Im Editor
  (nicht beim Testen) alles.

| Menü | Q | W | E | R | T | A | S |
|---|---|---|---|---|---|---|---|
| **Q** Kolonisten | **Zelt** | **Holzhaus** | **Steinhaus** | | | | |
| **W** Ressourcen | **Jägerhütte** | **Fischerhütte** | **Sägewerk** | **Mine** | **Steinbruch** | **Farm** | **Große Farm** |
| **E** Energie | **Tesla-Turm** | **Windmühle** | **Köhlerei** | **Kraftwerk** | Ölgenerator | | **Eisenmühle** |
| **A** Industrie und Handel | **Holzwerkstatt** | **Steinwerkstatt** | **Gießerei** | **Gasthaus** | **Markt** | **Bank** | **Lagerhaus** |
| **S** Militär | **Kaserne** | **Munitionsfabrik** | **Ingenieurszentrum** | **Aussichtsturm** | **Radar** | | |
| **D** Verteidigung | **Mauer** | **Tor** | **Turm** | **Pfahlfalle** (R) | **Sprengfass** (T) | **Steinmauer** (A) | **Klingenfalle** (S) |
| **Y** Wunder | **Spire** | **Academy** | **Victorious** | **Transmutator** | **Crystal Palace** | | |

Dazu: Ressourcen D großer Steinbruch, F Ölplattform; Verteidigung D Steinturm, F **Balliste**, G **MG-Turm**, unten
Y **Mine**, X **Shocking Tower**, C **Executor**.

- **Kaserne** (ohne Voraussetzung baubar): Q Soldat (240 Gold + 2 Eisen), W Ranger (120 + 2 Holz), E Speerträger (50),
  R Scharfschütze (300 + 2 Holz + 2 Eisen), T Medic (120), G Sammelpunkt – Preise und Ausbildung (25–31 s) wie TAB,
  das Eisen kommt aus der Gießerei. **Soldat und Scharfschütze** (Schusswaffen) erst mit einer **Munitionsfabrik**.
  Strg = 5 auf einmal, **Taste halten = immer wieder** (wie SC2), höchstens 5 in der Schlange, Klick auf die Schlange
  bricht ab (Gold, Holz, Eisen und Arbeiter zurück).
- **Gasthaus** (Steinwerkstatt, 2.000 Gold): Q Söldner-Soldat (600 + 5 Eisen), W Söldner-Ranger (300 + 5 Holz),
  R Söldner-Scharfschütze (750 + 5 Holz + 5 Eisen) – dieselben Tasten wie in der Kaserne, Schusswaffen auch hier erst
  mit Munitionsfabrik.
- **Ingenieurszentrum** (TABs, Gießerei, 3.000 Gold + Stein, Eisen, 20 Öl): Q **Lucifer** (600 + 15 Öl), W **Thanatos**
  (600 + 10 Eisen + 20 Öl), E **Titan** (2.000 + 40 Eisen + 40 Öl) – mit TABs Werten; sie verbrauchen Öl pro Abrechnung.
- **Sprengfass:** die Munitionsfabrik macht es (Q, 40 Gold + 2 Kohle, 8 s), ihr Lager fasst 6. Im Verteidigungsmenü
  (T) setzen – nimmt eins aus dem Lager, ziehen = Reihe – oder von Einheiten wegtragen lassen. **Jeder Schaden** und es
  geht hoch: 1.000 Schaden (wie TAB) in 2,5 Kacheln für alle (Zombies, eigene Einheiten, Gebäude, Häuser der Dörfer), Fässer
  daneben gehen mit, Lärm 400.
- **Scan** (C am **Radar**, wie SC2 – also erst nach seiner Forschung in der Gießerei): Klick auf die Karte → 10 s ein
  Kreis von 10 Kacheln sichtbar (der Ring), das Licht reicht 2 Kacheln weiter, damit im Ring nichts halb dunkel ist;
  auch über Wald, ohne Lärm. 2 Ladungen, am Anfang keine – der Radar lädt alle 120 s eine, und solange nimmt er seinem
  Netz **30 Strom**.
- **Preise** (Gold + Holz/Stein, alles in `data\buildings.json`): Zelt 30, Kaserne 450 + 20 Holz + 20 Stein,
  Holzwerkstatt 500 + 40 Holz, Mine 300 + 30 Holz, Sägewerk 300, Mauer 10 + 3 Holz, Tor 50 + 10 Holz, Turm 120 + 10 Holz
  (wie TAB). Der Tooltip zeigt den ganzen Preis.
- **Bauzeit** (wie TAB, `build_seconds` in `data\buildings.json`): ein Gebäude baut sich selbst – kurz, länger je
  größer und je höher im Techtree (TABs Formel: Mauer 14 s, Zelt 20 s, Sägewerk 36 s, Kaserne 56 s, Werkstätten und
  Gasthaus 84 s). Es fängt mit 20 % HP an, wächst bis voll, bekommt den Schild erst fertig. Bis dahin tut es nichts (keine
  Arbeiter, kein Gold, kein Strom, keine Ausbildung), nimmt aber schon seine Arbeiter – und Zombies können es treffen.
  Blass und niedriger gezeichnet, unten „Built x %“. Im Editor steht alles sofort.
- **Arbeiter** (wie TAB): Zelt gibt 2, Steinhaus 8, Kommandozentrum 10. Kaserne braucht 8, Sägewerk und Mine 4, jede Einheit 1, Mauer, Tor, Turm 0
  – solange sie steht oder lebt. Keine freien Arbeiter = nichts Neues. Werte: `worker_cost` / `workers` in `data\`.
- **Ziehen** baut eine Reihe (Mauern schräg 1 breit wie TAB, Türme mit Lücke). Das Gebäude bleibt an der Maus –
  Rechtsklick/Esc legt es weg.
- **Regeln:** nicht hinter der Grenze, kein Zombie auf dem Platz + 4 Kacheln drumherum. **Auf eigenen Einheiten
  erlaubt** – sie werden zur Seite geschoben. Die **Ölplattform** nur genau auf eine Ölquelle, jedes **Wunder** nur
  einmal. **Auf Erz** nur Verteidigung (Mauern, Tore, Türme, Schussgebäude, Fallen) und Mine und Steinbrüche, **auf Öl**
  nur die Ölplattform.
- **Beim Platzieren** wie TAB: grünes **Bau-Raster** auf den freien Kacheln rundum (`assets\ui\cell_available.png`),
  bei Türmen, die selbst schießen, der grüne **Reichweitenkreis** (auch unter ausgewählten).
- **Tor** (3 × 1, R dreht): Einheiten gehen durch, Zombies nicht. Darf auf Mauern (wird billiger).
- **Turm:** bis zu 4 Einheiten per Rechtsklick hinein – schießen und sehen 2 Kacheln weiter, unverwundbar. Q = raus,
  F = Sammelpunkt. 1 Kachel Abstand zum nächsten Turm.
- **Zelt:** 4 Kolonisten, 2 Arbeiter, 45 HP + 15 Schild. Isst 4 Nahrung pro Abrechnung – satt zahlt es 8 Gold
  (× Stromanteil), hungrig nichts. Schild weg = infiziert (dann keine Arbeiter).
- **Abreißen (Entf, „Demolish“)** bei jedem Gebäude außer dem Kommandozentrum, für alle ausgewählten: sofort weg,
  **50 %** des Preises kommen zurück (Gold, Holz, Stein …), wie TAB – auch, solange es noch gebaut wird
  (`demolish_…_percent` in `data\buildings.json`). Nicht bei infizierten. Der Tooltip zeigt, was zurückkommt.
- **Reparieren (G**, direkt über Entf**)** bei jedem Gebäude, für alle ausgewählten: kostet den fehlenden Anteil von `repair_cost`
  (`data\buildings.json`, z. B. Mauer 5, Zelt 50, Kommandozentrum 300 für eine volle Reparatur), sofort bezahlt; die
  HP kommen über bis zu 10 s zurück. Ein infiziertes Gebäude ist danach geheilt (zu, zahlt wieder Gold).
- **Alles reparieren (X** oder Knopf „Repair all“ unten rechts unter „Energy“): alle Gebäude. X nur, wenn keine
  Einheiten ausgewählt sind (sonst ist X die Linien-Formation). Reicht das Gold nicht: die billigsten zuerst.
- **Nicht reparierbar**, solange ein Zombie in der Bauzone steht (4 Kacheln, wie beim Bauen) – später auch nicht auf
  Creep.

### Wirtschaft

- **Sammeln** (wie TAB): Sägewerk (Wald → Holz), Jägerhütte (Wald, Gras und Erde → Nahrung, je Kachel 0,8 × Anteil
  der Landschaft, auf BR 60 / 30 / 15 %), Fischerhütte (Wasser → Nahrung),
  **Mine** (Erzfelder → Eisenerz, Kohle, Stein – was eben dort liegt), **Farm** (3 × 3, Felder auf Gras → viel Nahrung;
  steht nur auf Gras, Erde und Erz tragen nichts). Jedes zählt die passenden Kacheln in einem **Quadrat**: seine
  Fläche + `gather_range` Kacheln rundum (wie TAB: Mine 2 → 6 × 6, Sägewerk 3, Jäger 3, Fischer 3, Farm 2). Pro
  Kachel und Abrechnung wie TAB: Sägewerk und Mine 0,5, Jäger 0,48, Fischer 0,66, Farm 2. Die
  Kacheln darunter nur mit `gather_below` – **die Mine** steht auf dem Erz und baut es mit ab (wie Factorios Bohrer).
  Zwei gleiche teilen sich überlappende Kacheln. Beim Platzieren: Rahmen, gelbe Kacheln, „+x pro Abrechnung“; in der
  Auswahl „Range“ und „Below“.
- **Strom** (DESIGN.md 5.5): Kommandozentrum erzeugt 30 und reicht 10 Kacheln; **Tesla-Turm** im Netz trägt es 8
  Kacheln weiter. Alles, was Strom erzeugt, braucht oder trägt, muss im Netz stehen (blaue Kreise beim Platzieren).
  Zu wenig Strom → alles im Netz arbeitet anteilig langsamer (Sammeln, Gold der Zelte, Ausbilden). Tesla-Turm weg →
  das Netz zerfällt.
- **Windmühle** +30 (300 Gold + 20 Holz), **Köhlerei** +20, verbrennt aber 1 Holz pro Abrechnung (ohne Holz: nichts).
- **Bauen braucht freien Strom** (wie TAB): was Strom nimmt, wird nur gebaut, wenn sein Netz so viel übrig hat.
- **Forschung wie TAB** (`data\research.json`, `src\sim\research.rs`): die Werkstätten stellen nichts her – sie
  **erforschen**. Ein Knopf je Forschung auf ihrer Karte (TABs Porträt dessen, was sie freischaltet), einmal bezahlt,
  70 s × Faktor (Wunder 140 s), solange die Werkstatt Strom hat; fertig gilt sie die ganze Runde, auch wenn die
  Werkstatt fällt. Eine nach der anderen je Werkstatt; Klick in die Warteschlange bricht ab (alles Gold zurück).
  - **Holzwerkstatt:** Holzhaus 350, Farm 400, Markt 450, Steinwerkstatt 500, Scharfschütze 700, Aussichtsturm 300,
    Pfahlfalle 300, Balliste 700, Gasthaus 1.000, Crystal Palace 6.000 + Waren.
  - **Steinwerkstatt:** Steinturm 500, Steinmauer 500, MG-Turm 800, Gießerei 1.000, Steinhaus 600, Kraftwerk 800,
    Bank 800, Shocking Tower 900, Victorious und Academy 7.000 + Waren, Mine 600.
  - **Gießerei:** großer Steinbruch 1.500, große Farm 1.500, Eisenmühle 1.400, Ölplattform 1.200, Spire 8.000 + Waren,
    Radar 1.000, Klingenfalle 900, Executor 3.000, Ingenieurszentrum 2.000, dann Thanatos 2.000 und Titan 6.000,
    Transmutator 8.000 + Waren.
  - Dazu muss stehen, was es `requires` (wie TABs `BuildingRequisite`): Steinhaus, -mauer, -turm, Kraftwerk, Bank
    die Steinwerkstatt; Markt, Gasthaus, Balliste die Holzwerkstatt; Lucifer, Thanatos, Titan die Gießerei.
  - Unsere eigenen ohne Forschung: **Munitionsfabrik** (Steinwerkstatt, kostet auch 10 Kohle) → Soldat und
    Scharfschütze; MG-Turm braucht die Gießerei. **Mine** und **Tesla-Turm** gehen ab Start.
- **Holzhaus** (TABs, Holzwerkstatt: 8 Kolonisten, 4 Arbeiter, isst 8, zahlt 18) auf dem Zelt, **Steinhaus** (16
  Kolonisten, 8 Arbeiter, isst 16, zahlt 40) auf Zelt oder Holzhaus, **Steinmauer** (1000 HP), **Steinturm** (2000 HP)
  lassen sich direkt auf Zelt, Mauer, Turm bauen – ein Upgrade an Ort und Stelle (ein Teil des alten Goldes kommt
  zurück). Ebenso großer Steinbruch auf Steinbruch, Eisenmühle auf Windmühle, Klingen- auf Pfahlfalle.
- **TABs späte Gebäude** (TABs Werte, `data\buildings.json`; viele kosten **Öl**, das die **Ölplattform** pumpt – 10
  pro Abrechnung auf einer Ölquelle):
  - **Steinbruch** (nur Stein, wie die Mine darunter und 2 rundum), **großer Steinbruch** (doppelt), **große Farm**
    (doppelt pro Feld), **Eisenmühle** (+60 Strom), **Kraftwerk** (3 × 3, +160 Strom, verbrennt wie TABs 10 Holz).
  - **Lagerhaus** (4 × 4): +50 Lager für jede Ware (nicht Nahrung, nicht Gold); Sammler bis 12 Kacheln drumherum +20 %.
  - **Aussichtsturm** (24) und **Radar** (40) sehen weit, **über Wald** – wie die Wunder.
  - **Wunder** (je nur eins): **Spire** +800 Strom, reicht 30 Kacheln; **Academy** macht jede Einheit zum Veteran;
    **Victorious** (10.000 HP, Rüstung) – Häuser bis 24 Kacheln +20 % Gold, isst 10 Öl; **Transmutator** macht aus
    20 Holz, 20 Stein, 10 Eisen 40 Öl; **Crystal Palace** 60 Kolonisten und 800 Nahrung pro Abrechnung.
- **Pfahlfalle** (wie TABs Stakes): Zombies laufen darüber, greifen sie nicht an und bekommen alle 0,5 s Schaden; jeder
  Treffer nutzt sie ab. Wer darauf steht, läuft **halb so schnell** – Zombies und eigene Einheiten (die unverletzt).
- **Gießerei** (3 × 3, Steinwerkstatt, 3.000 Gold + 30 Holz + 50 Stein, Strom 50, Unterhalt 100): ein Tech-Gebäude wie
  TABs – sie schaltet den MG-Turm frei – und sie **schmilzt Eisen**: bei jeder Abrechnung bis zu 3, aus je 2 Eisenerz
  und 1 Kohle (weniger bei wenig Strom, nichts ohne Erz). Laut wie eine Mine; ohne Erz still.
- **Türme, die selbst schießen** (`weapon` in `data\buildings.json`, keine Einheiten darin):
  - **Balliste** (2 × 2, Holzwerkstatt, 500 Gold + 20 Holz, Strom 3): ein schwerer Bolzen jede Sekunde, 150 Schaden,
    9 Kacheln weit – immer auf den **zähesten** Zombie in Reichweite (Fat, Riot, Riese). Lärm 5 pro Schuss.
  - **MG-Turm** (1 × 1, Gießerei, 320 Gold + **10 Eisen**, Strom 1): 10 Schuss pro Sekunde, 20 Schaden, 6 Kacheln, auf
    den **nächsten** – gegen Massen. Lärm 3 pro Schuss, also sehr laut. Wenig Strom: lädt langsamer.
    Ziehen = Reihe.
  - **Shocking Tower** (2 × 2, Steinwerkstatt, Strom 20): lädt 4,5 s, dann trifft er **jeden** Zombie in 6 Kacheln
    (60), mit TABs Schockstrahlen.
  - **Executor** (2 × 2, Gießerei, 10 Öl): dreht sich wie die Balliste, 2 Schuss pro Sekunde, 100 Schaden, 9 Kacheln,
    trifft alles in 0,6 Kacheln um sein Ziel.
  - **Klingenfalle** (Gießerei): die Pfahlfalle, 10 pro Treffer, 350 HP. **Mine** (Steinwerkstatt): tritt ein Zombie
    drauf, geht sie hoch – 500 Schaden in 1,9 Kacheln, auch an deinen – und ist weg. **Verzögerung** wie TAB auf ihrer
    Karte (Q/W/E/R): sofort, 0,5 s, 1 s, 1,5 s – dann sind mehr der Nachdrängenden im Knall.
  - Ein angeschossener Zombie (und die um ihn ohne Ziel) geht auf den Turm los, wie bei Einheiten. Aus (Y) schießen
    sie nicht.
- **Abrechnung alle 28 s** (wie TABs `CResourceGenerator`, `economy_every_seconds` in `data\colony.json`):
  Zinsen des Dispo → Köhlereien nehmen Holz → Sammler und Kommandozentrum liefern → Gießereien schmelzen → Häuser essen
  und zahlen → Märkte verkaufen → **Unterhalt** wird bezahlt. Alle Werte „pro Abrechnung“.
- **Markt** (3 × 3, ab Start baubar, 400 Gold + 30 Holz + 40 Stein): Häuser, deren Mitte auf ihm oder bis 2 Kacheln drumherum
  liegt (7 × 7), zahlen **+50 % Gold** – jedes Haus nur für einen Markt (den ältesten). Er **verkauft**, was über
  **80 % des Lagers** liegt, reihum je 1 pro Ware, höchstens **1 Ware je 4 Kolonisten** seiner Häuser pro Abrechnung
  (Holz 2, Stein/Eisenerz/Kohle 3, Nahrung 1, Eisen 10 Gold). **Q**: Verkauf aus/an. Beim Platzieren: Rahmen, seine Häuser gelb,
  „+x Gold, bis zu y Waren“.
- **Bank** (2 × 2, Steinwerkstatt, 1.000 Gold + 50 Holz + 50 Stein): Goldlager **+2.000** und **Dispo 500**. Fehlt beim Bauen oder
  Ausbilden nur Gold, geht es mit **Shift** ins Minus – ohne Schuldenstrafen, aber **10 % Zinsen** pro Abrechnung.
  Fällt die Bank, fällt ihr Teil der Dispo-Schulden weg – und das Gold über dem kleineren Lager. Nicht abschaltbar.
- **Kommandozentrum** (wie TAB): 2.500 HP + 2.500 Schild, keine Rüstung, wird nie infiziert. Zahlt 200 Gold und
  bringt 20 Nahrung pro Abrechnung, gratis.
- Ausgewähltes Gebäude zeigt, was fehlt: KEIN STROM, WENIG STROM, HUNGRIG, KEIN HOLZ, AUS.
- **Unterhalt** (wie TAB, `upkeep` in `data\`): jede Einheit (Ranger 1, Soldat 3, Medic 3, Scharfschütze 5 pro Abrechnung)
  und jedes laufende Gebäude (Sägewerk 4, Mine 6, Farm 20, Windmühle 6, Köhlerei 4, Kaserne 14 …). Gold/h im Vorrat = Einnahmen
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
- **Aufbau wie SC2s Editor:** oben Menüleiste (File, Edit, View, Map, Test, Help; rechts der Dateiname, • = nicht
  gespeichert) und **Modulleiste** (Terrain · Objects · Zombies · Regions | Triggers · Events · Map; rechts Undo, Redo, Save,
  **▶ Test**). Links die Werkzeuge des Moduls, rechts die **Eigenschaften** des Gewählten, dazwischen die Karte – nur
  dort Kamera, Klicks, TABs Leiste und Minimap. Über der Karte schwebend: **Sight** (All / Player / Zombies), **Grid**,
  **Zombie tiles**, **Zoom** (− Reset +), unten links, was die Maus gerade tut. Unten die **Statuszeile**: Kachel und Boden unter der
  Maus, Auswahl, was in der Hand ist, Undo/Redo, Tag und Uhrzeit. Die Ränder der Panels lassen sich ziehen. Abzeichen
  an den Abschnitten: **TAB** (gelb), **SC2** (blau), **new** (grün). Help → Keys listet die Tasten.
- **Zombies als Kacheln** (wie TABs gemalte Zellen): ab dem Start-Zoom heraus von selbst, oder immer/nie
  (View-Menü, „Zombie tiles“). Jede Art ihre Farbe (Legende im Modul Zombies), die Art mit den meisten auf der Kachel
  zählt, je mehr desto deckender, heller Punkt = stürmt. Über TABs Bildern liegen sie über allem (Bäume verdecken nichts),
  mit feiner Fuge – nur die Dorfhäuser liegen über ihnen; mit **Grid** liegen sie wie TABs Zellen im Gitter, unter
  seinen Linien.
- **Auswählen, Kopieren, Einfügen** (in Edit): Klick wählt auch schlafende Zombies, neutrale Dinge und Deko (große
  Felsen, Ruinen, Strommasten); eine **Box** nimmt alles darin (Einheiten, Gebäude, Zombies, Dorfhäuser, Loot, Neutrale,
  Deko), Shift fügt hinzu. **Strg+C** kopiert, **Strg+X** schneidet aus, **Strg+V** setzt an der Maus ein (beliebig oft),
  **Entf** entfernt – auch im Edit-Menü. Kopiert wird alles, was die Kartendatei von einem Ding behält (Leben, infiziert/
  Veteran, Besitzer, Tag, Beitritts-Trigger, Spawner-Form und -Modus, Zombies im Dorfhaus). Was nicht passt, bleibt weg;
  ein zweites Kommandozentrum nie. Strg+Z/Strg+Y nehmen es zurück wie alles andere.
- **Edit / Test** (F10): Edit = keine Zeit vergeht, alles wird so gespeichert. Test = die Runde läuft; zurück in Edit
  ist die Karte wieder wie vorher.
- **Speichern:** Save / Strg+S. Ungespeichertes → Frage Save / Don't save / Cancel. „Revert to saved“ im Esc-Menü.
- **Rückgängig:** Undo / Strg+Z, Redo / Strg+Y (oder Strg+Shift+Z) – die letzten 20 Schritte, nur in Edit. Ein Schritt =
  alles, solange die linke Maustaste gedrückt ist (ein ganzer Pinselstrich), oder eine einzelne Änderung.
- **Setzen:** Klick auf eine Art im Katalog nimmt sie in die Hand, Klick auf die Karte setzt sie – danach ist die
  Hand leer; mit **Shift** bleibt sie für das nächste. Gebäude: Ziehen = eine Linie. Pinsel (Gelände, Zombies,
  Radierer) bleiben bis Rechtsklick/Esc.
- **Gebäude** gehen im Editor überall hin und kosten nichts. **Entf** nimmt alles Ausgewählte weg – auch das
  Kommandozentrum, ohne Rückgeld –, dazu den ausgewählten Spawner. Der Radierer nimmt alles unter dem Pinsel.
- **Speichern = Custom Level:** jede im Editor gespeicherte Karte steht im Spiel unter New game → **Custom levels**.
- **Spawner:** **Rechtsklick** (ohne Auswahl) öffnet ein Fenster: Name, **gerade** (wie die Weltgrenze) oder **schräg**
  (wie die Kacheln, um 45° gedreht), was er von selbst schickt, entfernen.
- **Felder** (egui): Zahlen ziehen oder anklicken und tippen (Enter: übernehmen, Esc: abbrechen), Listen zum Aufklappen,
  Abschnitte klappen mit einem Klick auf die Überschrift zu, das Mausrad scrollt das Panel. Solange man tippt, wirkt
  keine Taste im Spiel.

| Modul / Menü | Was |
|---|---|
| **File, Edit, View** | New map, Save, Close · Undo, Redo, Cut, Copy, Paste, Delete, **Clear map** (alles runter, alles Gras – Spawner, Events, Runde bleiben) · Sicht, Grid, Zoom, Kamera zur Basis |
| **Map** | Eine **Seite über die ganze Mitte** in drei Spalten (die Karte darunter nimmt keine Klicks). Links: Name (= Dateiname), **Info** wie TAB (Autor, Bild – eine PNG neben der Kartendatei –, Beschreibung, **New map every start**), die Karte, wie sie in **Custom levels** erscheint, **World**: Landschaft (BR … VO), **Playing field** (Breite/Höhe tippen oder „Edit in Regions“ = Kante auf der Karte ziehen; „Like a new map“ setzt zurück), **Grid** (die ganze Karte darum: Apply – wächst/schrumpft an allen Seiten, alles bleibt, wo es ist, neues Land ist Gras). Mitte: **Round** (Tage, Startgold, Startholz), **Difficulty** (TABs Zombie-Bevölkerung und mehr: wie nah jede Art kommen darf, Dorfhäuser; schon gesetzte Zombies bleiben), **Players** (nur zum Lesen). Rechts: **Victory / Defeat** – die Ziele wie im Modul Triggers. Undo geht überall |
| **Terrain** | Gelände malen (Berg, Wald, Wasser, Erde, Erase – eine einzelne Waldkachel sieht aus wie Wald), **Big rocks** (TABs große Felsen 3×3/4×4/5×5, Bild A/B/C oder gemischt – Klick, die Kacheln werden Berg; anderes Gelände drübermalen nimmt ihn weg), Erz malen, **Öl stempeln** (2 × 2; eine Kachel löschen nimmt die ganze Quelle). **Brush** (gilt auch für Radierer und Zombie-Pinsel): **Brush** (halten und ziehen), **Line** und **Rectangle** (drücken, ziehen, loslassen – Vorschau auf der Karte, ein Undo-Schritt), **Fill** (Klick: der ganze zusammenhängende Fleck desselben Bodens bzw. Erzes – nur Boden und Erz), **rund oder eckig**, Größe, **density** (unter 100 % nur so viele Kacheln, verstreut – Wald mit Lichtungen, Erz in Krümeln). **Landscape** (FA … VO: als welche von TABs Landschaften die Karte gezeichnet wird, mit ihrem Wetter). **Pictures** für Wald, Berg, Erz: mixed oder Gruppe A/B/C der Landschaftsbilder (drübermalen ändert sie; steht in der Kartendatei als `pictures`). **Smooth** (jede Kachel nimmt, was die meisten Nachbarn sind). **Keep buildings and tracks free** (Berg, Wald, Wasser nicht auf oder direkt neben Gebäuden, Dorfhäusern, Gleisen). **Area**: „Select area“ zieht ein Rechteck aus Kacheln – **Copy** (Strg+C: Boden, Wald, Felsen, Erz, Bilder, große Felsen – nicht, was draufsteht), **Paste** (Strg+V: an der Maus, Klick setzt, beliebig oft), Esc hebt auf. **Generate in the area** (TABs Generator nur im Bereich: Forest, Lake, Mountains, Stone/Iron/Coal/Gold field, Village of doom – Amount in %, Seed: gleicher Seed = gleiches Ergebnis). Rechts **Minimap** (Klick/Ziehen bewegt die Kamera), **Show** (auch im View-Menü: Ground, Forest/rocks/ore, Buildings and units, Zombies, Grid, Fog of war, **Paths of the zombies** – Pfeil je Kachel, roter Punkt = kein Weg –, **Noise as a heat map** – nur im Test laut) und **Under the mouse**: Kachel, Boden (mit Bildgruppe), Erz mit Menge, was drauf steht, begehbar, bebaubar, innerhalb der Spielfläche |
| **Objects** | Links der **Katalog**: jede Art mit Bild, in Gruppen zum Aufklappen (Housing and economy, Defense, Wonders, Units, Villages of doom, Decor, Railway, Loot – alle Gebäude des Spiels, auch TABs neue und die Wunder), **Suche** und Filter (All, Colony, Defense, Units, Neutral, Decor, Loot). **Placing → Owner**: das nächste Gebäude / die nächste Einheit gehört der Kolonie, ist **neutral** oder **tritt bei**. Unter der Karte die Tabelle **Placed** wie TABs Entity-Liste: Kind, Name ID, Tile, Owner, Life, Scripts – auch Dorfhäuser und Loot; Filter All/Buildings/Units/Neutral/Tagged/With script, Suche nach Art, Name oder ID; **Klick wählt auf der Karte** (Shift/Strg: dazu oder heraus), **Doppelklick** springt hin; die Oberkante lässt sich ziehen. **Railway** (TABs Gleise: **Gleisplaner** wie Factorio – offenes Ende oder freien Boden anklicken – oder Gleis: dort zweigt es ab, an einem Prellbock geht die Strecke weiter; auch ins Dunkle hinter dem Kartenrand, wo der Zug ungesehen hereinkommt –, Pfeile zeigen die Richtungen, das Stück unter der Maus steht als Ghost da, Klick baut, Ziehen baut weiter, Rechtsklick hört auf; einzelne Stücke gerade `\` `/`, Kurven 6 × 6, Prellböcke rasten an ein offenes Ende an; dazu Bahnbrücke, Schranken, Bahnübergang, Loren, Lampe – `/` `\` im Namen = wie sie liegen. Auf der Karte steht, wo der Zug reinkommt und wo er am CC hält), **Radierer** |
| **Zombies** | **Pinsel wie TAB** (pro Kachel ein Regler je Art, 0–3; nochmal drübermalen füllt nur auf), **Drop** (eine Menge einer Art), **Giant** (einer pro Klick) – schlafend/bummelnd oder stürmend, Spawner (setzen, ziehen), **Dorfhäuser** wie TABs drei: klein 2 × 2, mittel 3 × 3, groß 5 × 5 (Klick; an der Maus ihre Kacheln – rot, wo kein Platz – und wie viele Zombies drin warten) |
| **Rechts (immer)** | **Mehrere gewählt** (Box, Shift-Klick): was alle haben – Owner, Life, Infected/Veteran, Mark on the minimap, Delete all –, jede Änderung geht an alle (ein Undo-Schritt). **Object** (eines gewählt): Bild, Nummer, Größe, Kachel – Locate, Clone (seine Art in die Hand), Delete, **Owner** (Colony / **Neutral**: tut nichts, Zombies lassen es in Ruhe / **Joins**: tritt bei, sobald eine eigene Einheit auf eine Kachel herankommt), **Life** in %, Infected bzw. Veteran, **Name ID** (für Skripte), **Name** und **Text** (statt der Art in der Leiste), **Mark on the minimap**. Der gewählte **Spawner**: umbenennen, gerade/schräg, Periodic mit Sekunden und Anzahl / Off, entfernen |
| **Events** | Eine **Seite über die ganze Mitte** (`src\editor\events.rs`). Oben: Tage, Events und wie viele Zombies insgesamt, **+ Wave / + Notice / + Reveal**, „Days 1–30 / All“. **Zeitleiste** über die Runde: eine Zeile für Waves (Balkenhöhe = Zombies, Wurzelskala), Final, Notices (Punkt bei „Stunden vorher“), Reveal, Victory; Klick wählt, **Balken ziehen** = neue Stunde. Unten links **TABs Event**: Name, Tag und Stunde **+ bis zu** Stunden Zufall, **alle** so viele Stunden, so oft, **Wachstum** pro Mal (mit Obergrenze), **Announce**, **Spread**, **Final**, **Reveal**, **Wins**, **Wins when dead**, Send now, Delete; in der Mitte die **Gruppen**: woher (1 zufälliger Spawner, 1–4 zufällige Seiten, **ausgewählte Spawner** oder **ausgewählte Regionen** per Häkchen) und **mehrere Arten** mit Anzahl **von–bis**. Rechts **Plan from rules** (TABs Plan für die Tage, Bevölkerung einer Schwierigkeit; ersetzt alle Events nach Rückfrage) und **Send now** (Sofort-Wellen) | Dazu je Event ein **Skript, wenn es kommt**, und eines, **wenn es fertig ist** |
| **Regions** | Benannte Flächen und Punkte wie in SC2 (`src\editor\regions.rs`): Werkzeuge **Rectangle**, **Circle**, **Paint** (Kacheln malen) und **Point**; links die Listen (Special = das Spielfeld, Regions, Points, Spawners) mit Suche, rechts Name, Farbe, Formen mit Zahlen, Locate, Duplicate, Delete. Auf der Karte: anklicken, verschieben, Rechteck-Kante ziehen; mit gewählter Region **Strg-Ziehen** = Form dazu, **Alt-Ziehen** = Form ausschneiden. Sichtbar unter „Show → Regions and points“, nur im Editor |
| **Triggers** | Eine **Seite über die ganze Mitte** (`src\editor\triggers.rs`, `src\story_panel.rs`) wie SC2s Trigger-Editor mit TABs „Game Narrative“ (`src\sim\story.rs`): **links der Baum** (Goals, Command center button, Variables, Functions; die Trigger in Ordnern, mit Suche; „Object scripts“ = alle neutralen Objekte mit Name-ID und ihre Beitritts-Trigger – rot, wenn es das Objekt nicht gibt), **Mitte** der gewählte Trigger: Ereignis, Ordner, Notiz, Bedingungen und Aktionen als **Sätze** (blaue Wörter anklicken öffnet das Feld darunter; „...“ vor jeder Zeile: hoch, runter, Duplizieren, Bedingung, Code, Entfernen) oder als **Text** (so, wie die Kartendatei den Trigger hält – getippt und zurückgelesen), **rechts die Palette** der Aktionen mit Suche (Klick: ans Ende der zuletzt angeklickten Liste, Ziehen: zwischen die Schritte); Statuszeile: Bedingungen, Aktionen, Probleme (Event oder Objekt gibt es nicht, Ausdruck passt nicht mehr zu den Variablen). Inhalt: **Trigger** (wie SC2: Ereignis, Bedingungen als Ausdrücke – alle müssen stimmen –, Aktionen; an/aus, Ordner; Ereignisse: Spielstart, Sieg, Niederlage, Event kommt / ist fertig, Objekt tritt bei, Knopf, alle N Sekunden, Tag und Stunde, Gebäude fertig / fällt, Zombies getötet, Einheit betritt / verlässt Region, Einheit erreicht ein neutrales Objekt) – Aktionen nacheinander (**Block**, **Loop** / **Exit Loop**, **Warten** in Sekunden – ein laufendes Skript ist nur Daten): Code (nur der Code), Zombies (aus Seiten/Spawnern, stürmend oder nicht), Ressourcen geben/nehmen, Einheiten, Häuser infizieren, Meldung, Fenster (pausiert), **Zufall** (nach Gewicht), **Auswahl** (der Spieler wählt per Knopf; jede Option mit Bedingung, mit Loop und „Zurück“ ein Dialogmenü), ein neutrales Objekt beitreten lassen, Karte aufdecken, **Wetter** (Schnee, Regen, klar), **Kamera** und **Lärm** an einer Region oder einem Punkt, **Zug** (siehe unten), Sieg, Niederlage. **Ziele** (Sieg: alle erreicht – Niederlage: eines erreicht): Text, Bedingung und Fortschritt 0–100 als Ausdruck – im Spiel oben links mit Fortschritt. **Knopf an der Kommandozentrale** (V, Name, Tooltip, wie oft, sichtbar / aktiv / Zähler als Ausdrücke – was er tut, sind die Trigger mit Ereignis „Button“). **Variablen** (Zahl, Ja/Nein oder Text) mit Startwert. **Jede Aktion hat Bedingung und Code** wie in TAB (`Day >= 3 & Gold > 200`, `keys += 1; GiveGold(100)`): getippt, sofort geprüft (Fehler mit Stelle), die Liste aller Funktionen steht unter „Functions“ im Baum (`src\sim\funcs.rs`). Trigger zu einem Event oder zu einem neutralen Objekt (Beitreten, über die Name-ID) stehen auch in dessen Fenster |

### Für Entwickler

- **Gleiche Karte:** „Custom maps“ → Play, oder `LastBastion.exe --seed 12345` (ohne eigene Datei).
- **Determinismus:** mit festem Seed ist „Checksum tick 600“ (F3) bei jedem Start gleich; `cargo test` prüft das.
- **Balancing:** `cargo test --release balance_mini_round -- --ignored --nocapture` spielt die Runde (100 Tage) ohne Spieler
  durch – wie lange die Basis allein hält. `$env:ROAMERS = "0"` vorher: ohne herumstreifende Zombies – nur die Wirtschaft.
- **Kartenbilder:** `cargo test --release dump_previews -- --ignored` mit `DUMP_DIR` (und `DUMP_SEEDS`, sonst 2) schreibt
  jede Karte aus `data\game_maps.json` als PNG – gedreht wie die Minimap und TABs Kartenansicht.
- **Tempo der Horde:** `cargo test --release horde_tick_time -- --ignored --nocapture` misst auf einer 256er-Karte,
  wie lange ein Takt mit 22.000 bis 87.000 Zombies dauert (er hat 50 ms) – zu Beginn (fast alle schlafen) und wenn
  die Schlusswelle alle ruft. Standard (65.000, ≈ 2.200 wach): ≈ 0,4 ms / ≈ 10 ms.

## Fahrplan

- Die Schritte stehen in [DESIGN.md, Abschnitt 13](DESIGN.md#13-prototype-roadmap).
- Reihenfolge: Steuerung → Horden-Stresstest → **Mini-Runde** (macht es Spaß?) → dieselbe Runde im **Koop**.
