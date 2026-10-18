# Last Bastion

- **Echtzeit-Strategie ums Überleben:** Einheiten steuern wie in **StarCraft**, Horden und Basisbau wie in **They Are
  Billions (TAB)**, Produktionsketten und Strom wie in **Factorio**.
- **Runden von 2–4 Stunden** – wie in TAB wählt man die Anzahl der Tage. Isometrisch wie TAB.
- **Kern:** die TAB-Seite – die Horde gegen eine Basis im Raster.
- **Stand:** spielbare Mini-Runde + Karten-Editor. Das ganze Design steht in [DESIGN.md](DESIGN.md) (Englisch).

---

## Regeln für Claude (bitte zuerst lesen)

`CLAUDE.md` und Claudes Memory liegen nur lokal – deshalb stehen die Regeln hier:

- **Ich (Maximilian) gebe die Idee, Claude schreibt den Code.** Das gilt auch für `DESIGN.md`, diese README und
  Build-/Einrichtungs-Skripte.
- **Nicht einfach nur coden:** Zu jeder Änderung sagt Claude, **was** geändert wurde und **warum** (welche Dateien,
  welcher Ansatz, welche Alternative verworfen wurde). Ich will Rust und das Spiel dabei verstehen, nicht nur Code
  bekommen – Rust-Besonderheiten, die neu sind (Ownership, Borrowing, Traits …), kurz erklären.
- **Neuen Code Stück für Stück erklären** (so wie beim ersten Fenster, Stand 2026-10-15): jeden Abschnitt zeigen und
  erklären, was er tut und was die Syntax bedeutet – mit Vergleich zu Python/pygame, wo es passt. Neue Zeichen und
  Begriffe am Ende in einem kleinen Spickzettel (Rust ↔ Python). Was schon erklärt wurde, nur kurz erwähnen statt
  jedes Mal neu – außer es wird anders benutzt. Auch Hilfsdateien (`Cargo.toml`, Skripte) kurz erklären und zum
  passenden Kapitel im Rust-Buch verweisen.
- **Am Ende eine kurze Zusammenfassung** in ein paar Sätzen, und dazu ein Eintrag unter [Stand](#stand) in dieser
  README – **kurz, in Stichpunkten** (die Einzelheiten stehen im Code, in DESIGN.md und in git).
- Passt eine Idee nicht zum Design (`DESIGN.md`) oder hat sie einen Haken, sagt Claude das, bevor es loslegt.

---

## Dateien

| Datei | Was |
|---|---|
| `DESIGN.md` | Design-Dokument: Säulen, Runde, Wirtschaft, Einheiten, Zombies, Karte, Technik, Fahrplan, offene Fragen |
| `Cargo.toml` / `Cargo.lock` | Rust-Projekt und Bibliotheken (`macroquad`, `serde`/`serde_json`, `libm`) / ihre genauen Versionen |
| `build.ps1` | Baut beide Programme nach `dist\LastBastion.exe` und `dist\LastBastionEditor.exe` |
| `data\` | **Alle Spielwerte** als JSON (Einheiten, Gebäude, Zombies, Kolonie, Spawn) – erklärt in `data\README.md` |
| `src\main.rs`, `src\bin\editor.rs` | Die zwei Programme (je ein paar Zeilen) |
| `src\lib.rs`, `src\app.rs` | Liste aller Module / Fenster, Bildschirm-Wechsel, Kommandozeile |
| `src\game.rs` | Eine Karte auf dem Bildschirm: Takte, Zeichnen, Endbildschirm, Edit/Test im Editor |
| `src\sim\` | **Die Simulation** (deterministisch, zeichnet nichts): Befehle, Spawn, Zombies, Einheiten, Kampf, Gebäude, Runde, Tests |
| `src\placement.rs` | Alle Bauregeln an einem Ort |
| `src\data.rs`, `src\kinds.rs` | `data\` lesen / Einheiten- und Gebäudetypen |
| `src\aggro.rs`, `hunt.rs`, `flow.rs` | Wegfindung der Zombies und Einheiten (Bedrohungs-, Jagdkarte, Flow Field mit Sichtlinie) |
| `src\formation.rs`, `vision.rs`, `grid.rs` | Formationen / Sicht und Nebel / räumliches Raster |
| `src\map.rs` | Die Karte: Gelände, Erz/Öl, Gebäude, Weltgrenze, Sichtlinie |
| `src\mapgen\` | Kartengeneratoren (Wild, Regions, Rings, Canyons, Blank), Wege hinein, Erz und Öl, Werkzeuge |
| `src\mapfile.rs` | Eine Runde = eine Datei (`%APPDATA%\Last Bastion\maps\`) |
| `src\home.rs`, `menu.rs`, `widgets.rs`, `settings.rs` | Startbildschirm und Karten-Einstellung / Esc-Menü / Knöpfe und Regler / Einstellungen |
| `src\ui.rs`, `cursor.rs` | Maus und Tastatur → Auswahl, Gruppen, Kamera, Befehle / echte Windows-Maus |
| `src\hud.rs` | Die Leiste unten wie in TAB (Minimap, Uhr, Gruppen, Auswahl, Befehlskarte, Vorräte) |
| `src\editor.rs` | Das Panel des Editors |
| `src\draw.rs`, `picture.rs`, `iso.rs` | Zeichnen (Platzhalter) / Kartenbild für Minimap und Vorschau / isometrische Kamera |
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

- **Start:** 8 Soldaten, 4 Scharfschützen, 4 Ranger, ein Kommandozentrum, 300 Gold.
- **Länge:** 10 Tage ≈ 30 Minuten (3–80 wählbar, 1 Tag = 3 Minuten).
- **Ziel:** Zelte bauen (bringen Gold), Kaserne bauen (nur sie bildet aus), Einheiten, Mauern, Türme kaufen.
- **Gewonnen**, wenn die **Schlusswelle** am letzten Tag tot ist; **verloren**, wenn das Kommandozentrum fällt →
  Endbildschirm (Restart, „Look at the map“, Main menu).
- **Eine Runde = eine Datei** (wie TAB) unter `%APPDATA%\Last Bastion\maps\`. **Restart** liest sie neu (Änderungen aus
  dem Editor gelten sofort). Speichern mitten in der Runde kommt später (DESIGN.md 11.5).

### Startbildschirm und Karten-Einstellung

- New game, Map editor, **Custom maps** (Play / Edit / Delete), Settings, Quit.
- **New game** → Generator, Seed, Regler (Maus drauf = Erklärung), Vorschau, **Days** und **Waves** (25–300 %).
  „Random values“ würfelt, „Defaults“ setzt zurück. Die letzten Werte merkt sich das Spiel.

| Generator | Wie |
|---|---|
| **Wild** | Rauschen wie Factorio: Seen, Bergrücken, Wälder, Flüsse |
| **Regions** | Wie TAB: Regionen mit Wänden und **Pässen** dazwischen |
| **Rings** | Festungsringe mit versetzten Öffnungen |
| **Canyons** | Fels mit gewundenen Schluchten |
| **Blank** | Nur Gras – zum Selberbauen |

- **Für alle Generatoren:** Größe (96–256), freier Platz um die Basis, Spawner pro Seite, Wege hinein, herumstreifende
  Zombies, Erzfelder, Ölgruppen.
- **Wege hinein:** gewundene offene Wege von jeder Seite zur Basis, die sich wie Äste vereinigen – die Stellen zum
  Zumauern. Flüsse bekommen dort Furten, Regionswände Pässe. Ein Sicherheitsnetz verbindet alles Abgeschnittene.
- **Weltgrenze:** außen ein Ring von 16 Kacheln offenem Gelände mit den **Spawnern** (3 pro Seite, fast die ganze
  Seite breit). Dahinter sieht und läuft man nicht.
- **Erz:** Eisenerz (rostbraun), Kohle (schwarz), Stein (hellgrau) als Felder; neben der Basis je ein kleines, draußen
  größer und reicher.
- **Öl:** Quellen von genau **2 × 2 Kacheln** (lila mit schwarzer Lache), berühren sich nie. Neben der Basis eine
  einzelne, draußen Gruppen von 2 bis 6.

### Zombies und Wellen

- **Wellen** (`data\spawning.json`): ab Tag 4 alle 3 Tage (200 → 600 Runner pro Seite); am letzten Tag 6:00 die
  **Schlusswelle** von allen 4 Seiten mit Fat. Ab Tag 2 ein **konstanter Spawn**. Getötete Zombies kommen nie wieder.
- **Arten:** **Walker** (langsam, streifen von Anfang an herum, ≈ 3.900), **Runner** (schnell), **Fat** (groß, 600 HP).
- **Sehen** nur 5–6 Kacheln (nicht durch Wald), **hören** 3 Kacheln, angeschossen stürmen sie 5 s los.
- **Lärm** lockt am meisten: jeder Schuss weckt alle im Umkreis (Ranger 1, Soldat 3, Scharfschütze 10 Kacheln) für
  20 s. Sie laufen um Wasser und Berge, aber **durch Mauern** (einreißen); ein paar suchen die Lücke.
- Tote Einheiten stehen als Walker auf. Ein Zelt mit gebrochenem Schild ist **infiziert** und gehört den Zombies –
  Zombies und Einheiten laufen hindurch. **Reparieren** heilt es.

### Einheiten und Kampf

- **Soldat, Ranger, Scharfschütze, Medic** – alle Werte in `data\` (siehe `data\README.md`).
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
- **Vorräte:** Kolonisten, Gold (+ pro Stunde); Arbeiter, Nahrung, Energie, Holz, Stein, Eisen, Öl noch grau.

### Steuerung (wie StarCraft 2)

| Taste | Was |
|---|---|
| Linke Maus (klicken / ziehen) | Auswählen (Rahmen nimmt Gebäude nur ohne Einheiten). Auf nichts = Auswahl weg; Shift: dazu/weg |
| Strg + Klick, Doppelklick | Alle dieses Typs auf dem Bildschirm |
| Klick auf ein Symbol unten | Nur dieses (Shift: weg, Strg: nur dieser Typ) |
| Tab / Shift+Tab | Nächster / voriger Typ in der Auswahl (auch Gebäude) – die Befehlskarte zeigt ihn |
| F2 | Alle Einheiten |
| Rechte Maus | Laufen / Zombie angreifen / Einheit folgen / in den Turm. Gebäude: Sammelpunkt |
| Rechte Maus **ziehen** | Formation (X Linie, C Block, V Kreis) |
| T / P / M + Linksklick | Angriff (Attack-Move) / Patrouille / Laufen – ziehen = in Formation |
| S / H | Stopp / Halten |
| F / X | Ausgewählte Gebäude reparieren / alle reparieren (X nur ohne ausgewählte Einheiten) |
| **Shift** + Befehl | Hinten an die Befehlskette hängen. Shift loslassen = Befehl weg von der Maus, wieder drücken = zurück |
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

| Menü | Q | W | E | R | T | A | S |
|---|---|---|---|---|---|---|---|
| **Q** Kolonisten | **Zelt** | Steinhaus | | | | | |
| **W** Ressourcen | Jägerhütte | Fischerhütte | Sägewerk | Steinbruch | Ölpumpe | | |
| **E** Energie | Tesla-Turm | Windmühle | Generator | Ölgenerator | | | |
| **A** Industrie | Holzwerkstatt | Steinwerkstatt | Gießerei | The Inn | | | |
| **S** Militär | **Kaserne** | Munitionsfabrik | Belagerungsfabrik | | | | |
| **D** Verteidigung | **Mauer** | **Tor** | **Turm** | Minen | Stolperdraht | Balliste | MG-Turm |
| **Y** Wunder | – | | | | | | |

- **Kaserne:** Q Soldat (100 Gold), W Ranger (60), R Scharfschütze (200), T Medic (120), G Sammelpunkt. Shift = 5 auf
  einmal, höchstens 5 in der Schlange, Klick auf die Schlange bricht ab (Gold zurück).
- **Preise:** Zelt 100, Kaserne 150, Mauer 10, Tor 40, Turm 100. Gebäude stehen sofort (noch keine Bauzeit).
- **Ziehen** baut eine Reihe (Mauern schräg 1 breit wie TAB, Türme mit Lücke). Das Gebäude bleibt an der Maus –
  Rechtsklick/Esc legt es weg.
- **Regeln:** nicht auf Einheiten, nicht hinter der Grenze, kein Zombie auf dem Platz + 4 Kacheln drumherum.
- **Tor** (3 × 1, R dreht): Einheiten gehen durch, Zombies nicht. Darf auf Mauern (wird billiger).
- **Turm:** bis zu 4 Einheiten per Rechtsklick hinein – schießen und sehen 2 Kacheln weiter, unverwundbar. Q = raus,
  G = Sammelpunkt. 1 Kachel Abstand zum nächsten Turm.
- **Zelt:** 6 Kolonisten, 6 Gold pro Spielstunde, Schild weg = infiziert.
- **Reparieren (F)** bei jedem Gebäude, für alle ausgewählten: kostet den fehlenden Anteil von `repair_cost`
  (`data\buildings.json`, z. B. Mauer 5, Zelt 50, Kommandozentrum 300 für eine volle Reparatur), sofort bezahlt; die
  HP kommen über bis zu 10 s zurück. Ein infiziertes Gebäude ist danach geheilt (zu, zahlt wieder Gold).
- **Alles reparieren (X** oder Knopf „Repair all“ unten rechts unter „Energy“): alle Gebäude. X nur, wenn keine
  Einheiten ausgewählt sind (sonst ist X die Linien-Formation). Reicht das Gold nicht: die billigsten zuerst.
- **Nicht reparierbar**, solange ein Zombie in der Bauzone steht (4 Kacheln, wie beim Bauen) – später auch nicht auf
  Creep.

### Karten-Editor (`dist\LastBastionEditor.exe`)

- **Starten:** direkt, über „Map editor“ im Spiel, über „Edit“ bei einer Karte, oder Kartendatei auf die Exe ziehen.
- **Edit / Test** (F10): Edit = keine Zeit vergeht, alles wird so gespeichert. Test = die Runde läuft; zurück in Edit
  ist die Karte wieder wie vorher.
- **Speichern:** Save / Strg+S. Ungespeichertes → Frage Save / Don't save / Cancel. „Revert to saved“ im Esc-Menü.
- **Gebäude** gehen im Editor überall hin und kosten nichts.

| Tab | Was |
|---|---|
| **Map** | Name (= Dateiname), Save, Close, New map, Tage, Startgold |
| **Terrain** | Gelände malen (Berg, Wald, Baum, Wasser, Erase), Erz malen, **Öl stempeln** (2 × 2), Pinselgröße |
| **Entities** | Einheiten, Gebäude, **Radierer**, Zombies (Pinsel / Drop, Walker / Runner / Fat, stürmen / wandern), Spawner (setzen, ziehen, umbenennen, Constant / Periodic / Off), Sofort-Wellen |
| **Waves** | Zeitleiste der Wellen (Zeit, Final, Anzahl, Art, woher) und der konstante Spawn |
| **View** (immer) | All / Player / Zombies – durch wessen Augen man schaut |

### Für Entwickler

- **Gleiche Karte:** „Custom maps“ → Play, oder `LastBastion.exe --seed 12345` (ohne eigene Datei).
- **Determinismus:** mit festem Seed ist „Checksum tick 600“ (F3) bei jedem Start gleich; `cargo test` prüft das.
- **Balancing:** `cargo test --release balance_mini_round -- --ignored --nocapture` spielt die Mini-Runde zweimal
  automatisch durch (untätig / 12 Zelte + Kaserne + Soldaten).
- **Kartenbilder:** `cargo test dump_previews -- --ignored` mit `DUMP_DIR` schreibt jeden Generator als PNG.

## Fahrplan

- Die Schritte stehen in [DESIGN.md, Abschnitt 13](DESIGN.md#13-prototype-roadmap).
- Reihenfolge: Steuerung → Horden-Stresstest → **Mini-Runde** (macht es Spaß?) → dieselbe Runde im **Koop**.

## Stand

Die ausführliche alte Fassung steht in git (`git show a4e7199:README.md` – die Einträge bis zum letzten Commit).

- **2026-10-15 – Konzept und erster Code**
  - DESIGN.md angelegt, eigenes Repo, Sprache **Rust** mit **macroquad**.
  - Entscheidungen: Runden 2–4 Stunden (Tage wählbar), **Koop** per Lockstep (Simulation streng deterministisch),
    Bauen wie TAB (keine Arbeiter-Einheiten), Kolonisten als Ressource, Speichern wie Factorio, Daten als JSON.
  - Wegwerf-Prototyp: 10.000 Zombies bei 59 FPS – macroquad ist schnell genug.
- **2026-10-16 – Steuerung und Basis**
  - Steuerung wie StarCraft 2: alle Befehle, Befehlsketten, Kontrollgruppen, Kamera-Lesezeichen, Leiste unten.
  - Formationen wie DINO, Sicht und Nebel, Wald und Bäume, Spawner, Weltgrenze.
  - Kommandozentrum, Zelte, Mauern, Tor, Turm; Bauregeln in einer Datei.
  - Fünf Kartengeneratoren, Startbildschirm, Editor mit Tabs wie TAB, Karten speichern.
- **2026-10-17 – Mini-Runde spielbar**
  - Walker, Runner, Fat; Infektion; Lärm und Sicht wie TAB; Schießen mit Drehen/Zielen wie StarCraft, Werte aus TAB.
  - Alle Werte in `data\`; Gold für Einheiten und Gebäude; Wellen aus den Tagen, Schlusswelle, gewonnen/verloren.
  - Eine Runde = eine Datei; Karten-Editor als eigenes Programm; Kaserne als einziges Ausbildungsgebäude.
  - Erzfelder; Wege hinein von jeder Seite; offener Ring mit breiten Spawnern; Feuer verteilen; kein Ruckeln mehr.
- **2026-10-18 – Leiste wie TAB, Baumenüs, Öl**
  - Leiste unten neu aufgeteilt wie TAB (Uhr mit Pause-Knopf, Vorräte rechts); die Uhr oben ist weg.
  - Kommandozentrum: Baumenüs Q/W/E/A/S/D/Y, geplante Gebäude stehen grau an ihrem Platz. Kaserne jetzt S, dann Q;
    Zelt Q, dann Q.
  - Ölquellen (2 × 2, in Gruppen), Regler „Oil groups“, Öl-Stempel im Editor.
  - **Offen** (DESIGN.md 26–29): Munition doch als Ware? Zwei Tesla-Türme? Was holt Eisenerz und Kohle? Was tun Inn
    und Wunder?
  - README gekürzt (≈ 1.000 → ≈ 240 Zeilen); DESIGN.md ab Abschnitt 6 gekürzt (1.152 → ≈ 680 Zeilen): die
    Prototyp-Logs als kurze „In the prototype“-Stichpunkte, erledigte Fragen als Liste. Abschnitte 1–5 unverändert.
- **2026-10-18 – Overlay, Shift-Ketten, Kleinkram**
  - F3-Overlay in Abschnitten (Messwerte, Schalter, Tastenhilfe), Klick auf den Titel klappt ein/aus (bleibt
    gespeichert); Schalter klickbar, Seed per Klick kopieren. Neue Datei `src\overlay.rs`.
  - Shift-Ketten: Shift loslassen nimmt den Befehl von der Maus, Shift wieder drücken holt ihn zurück.
  - Kommandozentrum: **B**, Esc oder neu auswählen → zurück zu seinen Hauptknöpfen.
  - Bäume am Kartenrand nicht mehr im Nebel sichtbar (werden nach dem Nebel gezeichnet, abgedunkelt wie ihre Kachel).
  - Borderless: Fenster 1 Pixel höher als der Bildschirm, damit Screenshot-Tools es nicht „rausspringen“ lassen.
- **2026-10-18 – Infektion offen, Reparieren**
  - Infizierte Gebäude: Zombies und Einheiten laufen hindurch (vorher drumherum, DESIGN.md angepasst).
  - Reparieren (F) für alle ausgewählten Gebäude: Gold für den fehlenden Anteil, HP kommen über Zeit zurück;
    heilt infizierte Gebäude. Neue Werte `repair_cost` und `repair_seconds` in `data\buildings.json`.
  - Drei neue Tests (durchlaufen, Preis und Dauer, Heilen).
- **2026-10-18 – Alles reparieren**
  - X / Knopf „Repair all“: alle Gebäude, bei zu wenig Gold die billigsten zuerst. F nimmt dieselbe Regel.
  - Neu: keine Reparatur mit einem Zombie in der Bauzone (`can_repair` in `placement.rs`, Platz für Creep schon da).
