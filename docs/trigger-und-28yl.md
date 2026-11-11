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
- **Skripte** beim Start, Sieg, Niederlage, wenn ein Event kommt / fertig ist, beim Beitreten eines neutralen Objekts,
  vom Knopf der Kommandozentrale.
- **Aktionen** aus einer festen Liste: Zombies, Ressourcen, Einheiten, Häuser infizieren, Meldung, Fenster, Zufall,
  Auswahl, Zähler, Beitreten, Aufdecken, Sieg, Niederlage.
- **Ziele** aus einer festen Liste (Kolonisten, Tag, Kills, Gebäude, Objekt zerstört, Zähler, Event fertig), **Zähler**.

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
| 2 | **Trigger als Daten:** `Trigger { name, ordner, an, ereignis, bedingungen, aktionen }` in der Karten-Datei. Die heutigen Skripte werden Trigger mit festem Ereignis (Start, Sieg, Niederlage, Event kommt/fertig, Beitreten, Knopf); Ziele und Knopf bleiben eigene Teile. Alte Dateien werden beim Laden umgebaut. Zähler → **Variablen** (Zahl, Ja/Nein, Startwert) | offen |
| 3 | **Ausdruckssprache:** Typen Zahl, Ja/Nein, Text (später Einheit, Region, Punkt). Als Text getippt, sofort geprüft (Typfehler mit Stelle), gespeichert als fester Baum. **Funktionen aus einer Liste in Rust** (Name, Parametertypen, Rückgabe, Satz für den Editor, Wirkung) – die Sprache wächst, ohne den Interpreter anzufassen. TABs eingebaute Variablen und Funktionen von oben als erste Einträge. Jede Aktion bekommt **Bedingung + Code** wie in TAB | offen |
| 4 | **Ablauf wie TAB:** Block, Loop / Exit Loop (n), Auswahl mit Bedingung je Option, Zufall nach Gewicht, **Warten** als Aktion. Ein laufendes Skript ist nur „welcher Trigger, wo im Baum, welche lokalen Werte“ (serde) → Speichern mitten im Skript, Replays und Wiedereinstieg im Koop gehen von selbst. Sieg/Niederlage und Knopf (sichtbar, aktiv, Zähler) über Ausdrücke | offen |
| 5 | **Ereignisse:** TABs **Betreten** an Objekten (alle 20 Frames = alle 10 Takte, einmal, Bedingung, Kamera, Lärm). Dazu SC2s: Zeit, gebaut/zerstört, Zombies getötet, Welle kommt, Haus fällt; mit Regionen (`docs\editor.md` Schritt 4): Einheit betritt/verlässt Region, Lärm in Region. Feste Reihenfolge je Takt | offen |
| 6 | **Texte, Fenster, Sound:** Fenster mit Bild, Titel, Stimmung, eigenem Sound; Sound abspielen/stoppen; Meldung mit Farbe und Symbol; `{Ausdruck}` in Texten; **Strings.csv** (Sprachen, Varianten nach ID-Anfang). Braucht den Ton (`docs\tab-assets.md` Schritt 12) | offen |
| 7 | **Restliche TAB-Aktionen:** Technologien schenken (braucht die Forschung), Zug schicken/entladen (`docs\tab-assets.md` Schritt 13), `GiveEntities`, `ApplyRules`, `DamageAll`, `CureAll`, `CountDeads`, `CountBonus`, `IsInfected` | offen |
| 8 | **Konverter:** `.TABLevel` → unsere Karten-Datei: Gelände, Objekte (Name-ID, Besitzer, Betreten-Skripte), Zombie-Ebene, Events, Story. ZXCode → unsere Sprache mit **TABs Bindung** (der Konverter setzt die Klammern, unsere Sprache bindet normal). Was er nicht kennt, meldet er mit Karte und Stelle, statt es zu verschlucken | offen |
| 9 | **Test an 28YL:** alle 20 Karten ohne Meldung umwandeln, die ersten drei durchspielen (Dialogbäume, Betreten, Sieg). Danach: eine **Kampagne** bei uns trägt Variablen von Karte zu Karte (DESIGN 9, Story-Bogen) – die Abfragen am Kartenstart fallen dann weg | offen |

## Reihenfolge

- 1 zuerst: es zeigt, was 28YL wirklich braucht – danach lieber kürzen als raten.
- 2 → 3 → 4 nacheinander; 5 geht ohne Regionen (Betreten, Zeit, Kills), der Regionen-Teil nach `docs\editor.md`
  Schritt 4.
- Der Editor dazu (`docs\editor.md` Schritt 5: Sätze / Text, Baum, Aktionen zum Hineinziehen) baut auf 2–5 auf.
- 6 nach dem Ton, 7 nach Forschung und Zügen, 8–9 zum Schluss.
- Die Karten und alles daraus bleiben lokal, nie in git (wie TABs Bilder).
