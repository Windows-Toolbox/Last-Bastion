# Trigger wie SC2, geprüft an „28 Years Later“

**Ziel:** Karten erzählen ihre Story mit **Triggern wie in SC2** (Ereignis → Bedingungen → Aktionen, als Daten
gespeichert) und einer kleinen **getypten Ausdruckssprache**. Geprüft wird das an TABs Workshop-Kampagne
**„28 Years Later“** (28YL, POBmaestro, 20 Karten): sie soll per Konverter bei uns laufen.

Jeder Schritt ist ein Startpunkt für eine neue Unterhaltung („Lies docs/trigger-und-28yl.md, Schritt N“). Das Warum
steht in `DESIGN.md` 12.9 Schritt 6 (Englisch), der Editor dazu in `docs\editor.md` Schritt 5.

## Für jeden Schritt

- README → „Regeln für Claude“: Code Stück für Stück erklären, am Ende eine kurze Zusammenfassung.
- **Deterministisch:** Trigger laufen in der Simulation, jeden Takt in fester Reihenfolge, nur ganze Zahlen – sonst
  bricht Koop (Lockstep). Eine Entscheidung des Spielers kommt als Order.
- **Kein Lua/Rhai:** eine laufende Koroutine lässt sich nicht speichern, und Luas `pairs`-Reihenfolge bricht Lockstep.
- Neues in der Karten-Datei: `VERSION` in `src\mapfile.rs` hoch, alte Dateien bleiben lesbar, mit Test.
- `DESIGN.md` 12.9, die README (Tabelle „Editor“) und `data\README.md` an Ort und Stelle nachziehen.
- Prüfen: `cargo test --release`, `cargo clippy --release --all-targets`, Editor und Spiel ansehen (Skill
  `run-last-bastion`).

## Was es schon gibt

`src\sim\story.rs` + `src\story_panel.rs` (Editor-Modul „Triggers“) – TABs „Game Narrative“ ohne Code:
- **Trigger** (Ereignis, Bedingungen als Ausdrücke, Aktionen mit Bedingung und Code) für Start, Sieg, Niederlage, Event kommt / fertig, Beitreten eines
  neutralen Objekts, Knopf der Kommandozentrale.
- **Aktionen** aus einer festen Liste: Zombies, Ressourcen, Einheiten, Häuser infizieren, Meldung, Fenster, Block, Loop,
  Exit Loop, Warten, Zufall, Auswahl, Code, Beitreten, Aufdecken, Wetter, Zug, Sieg, Niederlage.
- **Ziele** (Text, Bedingung, Fortschritt als Ausdruck), **Knopf** der Kommandozentrale (sichtbar, aktiv, Zähler als
  Ausdrücke), **Variablen**.

## Was TAB mehr hat (aus dem Decompile)

Quellen: `ZX.Editor\ZXAction*.cs`, `ZXScript.cs`, `ZXCompiler.cs`, `ZXCodeVariablesAndFunctions.cs`,
`ZX.Components\CScriptOnEnter.cs`; Notizen in `E:\Software\Decompiling_Project\games\TheyAreBillions_Decompiled\MECHANICS.md`
(„Karten-Editor & Skripte“).
- **Jede Aktion hat eine Bedingung und eine Code-Zeile** (ZXCode). Bedingung falsch → Aktion samt Kindern übersprungen;
  der Code läuft **nach** der Aktion (bei Block/Auswahl/Zufall nach allen Kindern).
- **Ablauf:** Block, Loop / Exit Loop (mit Zahl: mehrere Schleifen), Auswahl mit Bedingung je Option (keine sichtbar →
  ohne Fenster weiter), Zufall nach Gewicht. Loop + Auswahl + „Zurück“ = Dialogmenü.
- **Variablen** Zahl und Ja/Nein mit Startwert (statt nur Zählern).
- **Sieg/Niederlage** mit Text, Bedingung und Fortschritt 0–100 als Ausdruck; **Knopf der Kommandozentrale** mit
  Ausdrücken für sichtbar, aktiv und Zähler.
- **Betreten** (`CScriptOnEnter`) an jedem Objekt (Gebäude, Einheit, schlafender Zombie, Beute): alle 20 Frames
  geprüft, feuert einmal, wenn eine eigene Einheit auf seinen Kacheln steht und die Bedingung wahr ist; optional fährt
  die Kamera hin und es entsteht Lärm (0–10.000).
- **Fenster** mit Bild (PNG neben der Karte), Stimmungs-Sound (`Celebration`, `Tension`, `Sad` …), eigenem Sound,
  Animation; **Sound** abspielen (Standard, eigene Datei, Schleife, Einheitenstimme) und stoppen.
- **Weitere Aktionen:** Technologien schenken (bis 5), Zug schicken/entladen, nur Code ausführen.
- **ZXCode:** Zahl, Ja/Nein, Text; `X = …`, `+=`, `-=`, `;`. Eingebaut: `Day`, `Hour`, `TotalHours`, `Gold` … `Oil`,
  `…Production`, `Workers`/`Food`/`Energy`/`Colonists` (+ `…Available`), `D100`; `Random`, `Count(Typ)`,
  `CountDeads`, `CountBonus`, `IsDead(ID)`, `IsInfected(ID)`, `EndGameWon/Lost()`, `GiveEntities`, `Give<Rohstoff>`,
  `ApplyRules("…")`, `DamageAll`, `CureAll`. **Eigenheit:** geteilt wird auf oberster Ebene am Operator, der in
  `| & = != <= >= > < + - * /` zuerst kommt, am ersten Vorkommen → `a - b - c` = `a - (b - c)`.
- **Texte:** `{Ausdruck}` wird ersetzt; mit `Strings.csv` neben der Karte: Spalte der Spielsprache, sonst `EN`; alle
  Zeilen, deren ID mit dem Text beginnt, sind Varianten (eine zufällig).

**Was 28YL davon braucht:** 6.325 Bedingungen, 5.320 Code-Zeilen, 478 Betreten-Trigger, 1.270 Auswahlen – aber nur
einfache Ausdrücke (Vergleiche, `&`, `|`, `+=`, `;`, `IsDead`, `Count`, `Give…`; kein `*`, `/`, `{…}`). Workshop-Karten
speichern nichts füreinander: frühere Entscheidungen fragt die Karte am Start per Auswahl ab.

## Schritte

| # | Was | Status |
|---|---|---|
| 1 | **Karten auslesen:** `tools\tab_levels.py` öffnet `CustomLevel.TABLevel` (ZIP mit ZipCrypto, Passwort `crypted_zip_password("CustomLevel.TABLevel")` – für alle Karten gleich, siehe README des Decompiles), liest `Info` und `Data` (SharpSerializer-XML, Wurzel `ZXModFileCustomLevel`), dekodiert die Ebenen (`"<B>\|<H>\|<Base64>"`, 32 Bit je Zelle). Ausgabe nur lokal: je Karte JSON + eine Liste, welche Aktionen, Funktionen, Variablen und Ereignisse wirklich vorkommen. Klärt auch, welche Workshop-Ordner 28YL sind (lokal nennen die `Strings.csv` nur 29YL Phase 13–24 – 28YL evtl. erst abonnieren) | offen |
| 2 | **Trigger als Daten:** `Trigger { name, ordner, an, ereignis, bedingungen, aktionen }` in der Karten-Datei. Die heutigen Skripte werden Trigger mit festem Ereignis (Start, Sieg, Niederlage, Event kommt/fertig, Beitreten, Knopf); Ziele und Knopf bleiben eigene Teile. Alte Dateien werden beim Laden umgebaut (Format 8, `triggers_from_scripts` in `src\mapfile.rs`). Zähler → **Variablen** (Zahl, Ja/Nein, Startwert). Bedingungen sind vorerst die der Ziele (`Condition`) – Schritt 3 ersetzt sie durch Ausdrücke. Trigger zu Event / neutralem Objekt hängen an Name bzw. Name-ID (Umbenennen im Editor zieht sie mit) | fertig |
| 3 | **Ausdruckssprache:** Typen Zahl, Ja/Nein, Text (später Einheit, Region, Punkt). Als Text getippt, sofort geprüft (Typfehler mit Stelle), gespeichert als fester Baum (`src\sim\expr.rs`: Parser, Typprüfung, Ausgabe als Text; normale Bindung, die Klammern nach TABs Eigenheit setzt der Konverter). **Funktionen aus einer Liste in Rust** (`src\sim\funcs.rs`: Name, Parametertypen, Rückgabe, Satz für den Editor, Wirkung; der Editor zeigt die Liste). Erste Einträge aus TAB: `Day`, `Hour`, `TotalHours`, `D100`, `Random`, Rohstoffe, `Workers`, `WorkersAvailable`, `Colonists`, `Count(Typ)`, `IsDead(ID)`, `EndGameWon/Lost()`, `Give<Rohstoff>`; dazu `Kills`, `EventDone`, `FoodStock`. Entity-Typen und IDs sind vorerst Text. Jeder Schritt einer Aktionsliste hat **Bedingung + Code** (`Step`; falsch → Aktion und Code übersprungen, Code danach – bei „Choose“ nach der Wahl); Aktion „Variable“ ist weg (Code `x += 1`), neue Aktion „Code“. Trigger-Bedingungen sind Ausdrücke; Variablen auch Ja/Nein und Text. Karten-Datei Format 9, alte Dateien lesen sich weiter. **Noch nicht:** `{Ausdruck}` in Texten (6), `Energy`/`Food`/`…Production`, `CountDeads`, `CountBonus`, `IsInfected`, `GiveEntities`, `ApplyRules`, `DamageAll`, `CureAll` (7) | fertig |
| 4 | **Ablauf wie TAB:** Block, Loop / Exit Loop (n), Auswahl mit Bedingung je Option (keine sichtbar → ohne Fenster weiter), Zufall nach Gewicht (Bedingung je Option), **Warten** (Sekunden als Ausdruck). Ein laufendes Skript ist ein `Run` (`src\sim\script.rs`, serde): Trigger, Weg im Baum (`Level { step, child }`), nächster Schritt, worauf es wartet (Tick / Auswahl mit den gezeigten Optionen) – Speichern mitten im Skript, Replays und Wiedereinstieg im Koop gehen von selbst. Aktions- und Zug-Skripte sind `Place` (Trigger + Weg), nicht kopiert. Schritt-Regel wie TAB: Bedingung falsch → alles übersprungen, **Code nach allen Kindern**. Auswahl hält das Skript an (die Schritte danach laufen erst nach der Wahl), „Zurück“ = Option mit Exit Loop → Dialogmenü. Ziele (`when` + `progress`) und Knopf (`visible`, `enabled`, `counter`) sind Ausdrücke, einmal je Takt ausgewertet und zwischengespeichert (`goal_states`, `command_state`; die Oberfläche liest nur). Karten-Datei Format 10; `Condition` bleibt nur zum Lesen alter Ziele (`to_expr`, `progress_expr`) | fertig |
| 5 | **Ereignisse:** TABs **Betreten** an Objekten (alle 20 Frames = alle 10 Takte, einmal, Bedingung, Kamera, Lärm). Dazu SC2s: Zeit, gebaut/zerstört, Zombies getötet; mit Regionen: Einheit betritt/verlässt Region. Gebaut: `TriggerEvent::{Every, Clock, Built, Fell, Kills, Enters, Leaves, Reached}` (`src\sim\story.rs`), geprüft in `src\sim\watch.rs` an fester Stelle im Takt (nach `join_neutrals`; Gebäude feuern dort, wo sie fertig werden / fallen); Aktionen **Kamera** (`Notice::Camera`, die Oberfläche setzt die Kamera) und **Lärm** an einer Region oder einem Punkt (`Site`); `zone_uses` zählt Trigger-Ereignisse und -Aktionen mit, ein gelöschte Region fällt aus den Zombie-Gruppen der Aktionen. Karten-Datei Format 11. Abweichungen siehe unten | fertig |
| 6 | **Texte, Fenster, Sound:** Fenster mit Bild, Titel, Stimmung, eigenem Sound; Sound abspielen/stoppen; Meldung mit Farbe und Symbol; `{Ausdruck}` in Texten; **Strings.csv** (Sprachen, Varianten nach ID-Anfang). Baut auf dem Ton auf (`src\audio.rs`) | offen |
| 7 | **Restliche TAB-Aktionen:** Technologien schenken (`World::researched`, `src\sim\research.rs`), Zug entladen (schicken gibt es: Aktion „Train“ mit Ankunfts-Skript), `GiveEntities`, `ApplyRules`, `DamageAll`, `CureAll`, `CountDeads`, `CountBonus`, `IsInfected` | offen |
| 8 | **Konverter:** `.TABLevel` → unsere Karten-Datei: Gelände, Objekte (Name-ID, Besitzer, Betreten-Skripte), Zombie-Ebene, Events, Story. ZXCode → unsere Sprache mit **TABs Bindung** (der Konverter setzt die Klammern, unsere Sprache bindet normal). Was er nicht kennt, meldet er mit Karte und Stelle, statt es zu verschlucken | offen |
| 9 | **Test an 28YL:** alle 20 Karten ohne Meldung umwandeln, die ersten drei durchspielen (Dialogbäume, Betreten, Sieg). Danach: eine **Kampagne** bei uns trägt Variablen von Karte zu Karte (DESIGN 9, Story-Bogen) – die Abfragen am Kartenstart fallen dann weg | offen |

## Abweichungen von TAB (Schritt 4)

- **Exit Loop** verlässt die Schleife sofort (der Rest bis zur Schleife ist übersprungen, der Code jedes verlassenen
  Schritts läuft, von innen nach außen). TABs Exit Loop überspringt nur die Geschwister im selben Block, der Rest des
  äußeren Blocks läuft weiter. Eine Exit-Loop-Aktion mit falscher Bedingung bricht bei TAB den Block trotzdem ab (Fehler,
  nicht nachgebaut). Außerhalb jeder Schleife tut Exit Loop nichts. Ob 28YL sich auf TABs Eigenheit verlässt, zeigt Schritt 9.
- **Warten** gibt es bei TAB nicht (Aktion neu, Einheit Sekunden = 20 Takte). Eine Schleife, die in einem Zug mehr als
  10.000 Schritte macht, ohne zu warten, wird gestoppt.
- Die gezeigten Optionen einer **Auswahl** stehen im Lauf-Zustand fest; wie bei TAB wird die Bedingung der gewählten
  noch einmal geprüft – ist sie inzwischen falsch, wird die Option übersprungen (Code der Auswahl und die Schritte danach
  laufen). Ein falscher `Order::Choose { index }` ändert nichts, die Auswahl bleibt.
- **Fenster** pausieren weiter nur die Oberfläche (`Notice::Window`), nicht das Skript: es läuft im selben Takt weiter, die
  Zeit steht aber, solange das Fenster offen ist. Wer im Koop später einsteigt, sieht ungelesene Fenster nicht.
- Der Knopf hat TABs Ressourcenkosten nicht (`GiveGold(-100)` im Trigger, `Gold >= 100` als Bedingung). Alte Ziele „Event
  fertig“ und „Objekt zerstört“ zeigen keinen Fortschritt in Prozent mehr (nur erreicht / nicht).
- Ziele und Knopf werden mit den Würfeln der Welt ausgewertet (`Random`, `D100` in einem Ziel verbrauchen jeden Takt einen).
- **Umbenennen** im Editor zieht jetzt auch `EventDone("…")`, `IsDead("…")` und die Aktion „Join“ mit (alle Ausdrücke und
  Ziele, `Story::walk`). Texte in `Count("…")` (Typnamen) bleiben, wie sie sind.
- Ändert man die Art einer Variablen, zeigt die Seite Triggers unter „Variables“ rot die Trigger / Ziele, deren Ausdrücke
  nicht mehr passen (`Story::misfits`), und der Baum markiert den Trigger mit „!“; zur Laufzeit wird weiter nur
  umgewandelt.
- Die Oberfläche ist die Seite Triggers (`docs\editor.md` Schritt 5): Aktionen als Sätze, Block, Loop und Auswahl-Optionen
  eingerückt darunter. Die blauen Wörter öffnen ihr Feld unter der Zeile statt in einem Pop-up; „Text“ zeigt den Trigger
  als JSON (keine eigene Sprache für ganze Aktionen). Ein Trigger hat eine Notiz (`comment`, nur für den Autor), aber
  noch kein „Only once“.
- Für den Konverter (Schritt 8): `Count(Typ)` vergleicht mit dem Namen aus `stats().name` ohne Leerzeichen und
  Groß/Klein, TABs Symbole müssen darauf abgebildet werden; `TotalHours` ist noch nicht an TAB geprüft.

## Abweichungen von TAB / offen (Schritt 5)

- **Betreten** (`Reached`) gilt nur für **neutrale** Objekte mit Name-ID, nicht für eigene Gebäude, Einheiten oder Beute; geprüft wird wie bei „Beitreten“ im Umkreis von einer Kachel (TAB: „auf den Kacheln“), alle 10 Takte, einmal je Name-ID. Kamera und Lärm sind eigene Aktionen statt Felder am Ereignis.
- **Region betreten / verlassen** feuert einmal je Blick (alle 4 Takte) und Region, egal wie viele Einheiten; es gibt noch keine „auslösende Einheit“ im Ausdruck. Wer schon beim Start drin steht, „betritt“ nicht; stirbt eine Einheit in der Region, zählt das als Verlassen.
- **Zombies getötet** feuert einmal beim Erreichen der Zahl (auch wenn der Zähler darüber springt); **Tag und Stunde** einmal; **Alle N Sekunden** zum ersten Mal nach N Sekunden.
- **Gebäude fällt** nur bei Zerstörung (nicht Abriss, nicht Fass, das explodiert ist).
- **Offen:** Aktion „Spielfeld auf Region“ (das Spielfeld ist ein Rechteck in Bildschirmachsen, eine Region nicht – bräuchte eine Umrechnung), Lärm/Kamera beim Betreten als Felder, „Haus fällt“ der Dörfer des Untergangs, Ausdrücke `UnitsIn(Region)`.

## Reihenfolge

- 1 zuerst: es zeigt, was 28YL wirklich braucht – danach lieber kürzen als raten.
- 2 → 3 → 4 nacheinander; 5 geht ohne Regionen (Betreten, Zeit, Kills), der Regionen-Teil nach `docs\editor.md`
  Schritt 4.
- Der Editor dazu (`docs\editor.md` Schritt 5: Sätze / Text, Baum, Aktionen zum Hineinziehen) baut auf 2–5 auf.
- 7 nach den Zügen, 8–9 zum Schluss.
- Die Karten und alles daraus bleiben lokal, nie in git (wie TABs Bilder).
