# Last Bastion

Echtzeit-Strategie ums Überleben: Einheitensteuerung wie in **StarCraft**, Horden und Rundenaufbau wie in
**They Are Billions**, Produktionsketten und Stromnetz wie in **Factorio** – in Runden von 2–4 Stunden (wie in TAB
wählt man die Anzahl der Tage). Isometrisch wie TAB – der Kern ist die TAB-Seite: die Horde gegen eine Basis im Raster.

Stand: **Konzept**, das erste Fenster läuft. Das ganze Design steht in [DESIGN.md](DESIGN.md) (auf Englisch).

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
  passenden Kapitel im Rust-Buch verweisen. Neue Einträge kommen zusätzlich in [SPICKZETTEL.md](SPICKZETTEL.md).
- **Am Ende eine kurze Zusammenfassung** in ein paar Sätzen, und dazu ein Eintrag unter [Stand](#stand) in dieser
  README.
- Passt eine Idee nicht zum Design (`DESIGN.md`) oder hat sie einen Haken, sagt Claude das, bevor es loslegt.

---

## Dateien

| Datei | Was |
|---|---|
| `DESIGN.md` | Design-Dokument: Säulen, Ablauf einer Runde, Wirtschaft, Einheiten, Zombies, Karte, Technik, Fahrplan, offene Fragen |
| `SPICKZETTEL.md` | Rust-Spickzettel mit Python-Vergleich – alles, was im Code bisher neu war |
| `Cargo.toml` | Rust-Projekt: Name, Rust-Edition, Bibliotheken (`macroquad`, `serde` + `serde_json` für JSON, `libm` für `sin`/`cos`/`atan2`, die auf jedem PC gleich rechnen) |
| `Cargo.lock` | Von `cargo` erzeugt: die genauen Versionen aller Bibliotheken, damit jeder Build gleich ist (gehört in git) |
| `src\main.rs` | Start, Spielschleife mit festen Takten, Debug-Anzeige |
| `src\sim\` | **Die Simulation**, deterministisch, zeichnet nichts – eine Datei pro Teil eines Takts: `mod.rs` (die `World`, die Reihenfolge eines Takts in `step`, kleine Helfer für alle), `orders.rs` (Befehle der Spieler), `spawning.rs` (Walker beim Start, konstanter Spawn, Spawner, Wellen, Tote stehen auf), `zombies.rs` (was sie bemerken, wohin sie laufen, wen sie schlagen), `units.rs` (laufen, ankommen, Türme), `commands.rs` (Befehle → Befehlsketten, Formationen), `combat.rs` (Ziele suchen, drehen, zielen, nachladen, schießen), `buildings.rs` (Gebäude, Produktion, Einsturz), `terrain.rs` (wenn sich die Karte ändert), `tests.rs` |
| `src\placement.rs` | **Alle Bauregeln an einem Ort:** wo ein Gebäude hin darf, worauf es gebaut werden darf (Tor auf Mauern), wie viel Abstand es zu anderen halten muss (Türme), was es dort kostet, wie eine gezogene Reihe aussieht |
| `data\` | **Alle Spielwerte an einem Ort** (JSON): `units.json`, `buildings.json`, `zombies.json`, `colony.json`, `spawning.json`. Was jeder Wert heißt: `data\README.md` |
| `src\data.rs` | Liest `data\` (vor jeder neuen Runde, sonst die eingebauten Werte) |
| `src\kinds.rs` | Einheiten- und Gebäudetypen und was ihre Werte bedeuten (Soldat, Ranger, Scharfschütze, Medic; Kommandozentrum, Zelt, Mauer, Tor, Turm) |
| `src\aggro.rs` | Bedrohungskarte: welche Einheit (in Hörweite) bzw. welches Gebäude ist von dieser Kachel aus das nächste (Luftlinie, für Zombies) |
| `src\hunt.rs` | Jagdkarte: welche Einheit ist von hier **zu Fuß** die nächste – für die Lückensucher um Mauern herum, für den Ansturm der anderen um Wasser herum, aber durch Mauern hindurch |
| `src\flow.rs` | Flow Field mit Sichtlinie: Wegweiser zu einem Ziel für beliebig viele Figuren (für Zombies auch durch Gebäude) |
| `src\formation.rs` | Formationen wie in *Diplomacy is Not an Option*: Linie, Block, Kreis – ein Platz pro Einheit |
| `src\vision.rs` | Sicht und Nebel: was Einheiten und Gebäude gerade sehen und was schon erkundet ist |
| `src\grid.rs` | Räumliches Raster: wer steht in welcher Kachel (schnelle Nachbarsuche) |
| `src\map.rs` | Die Karte: Kacheln mit Gelände (Gras, Berg, Wald, Baum, Wasser), wo Gebäude und Tore stehen, die Weltgrenze, Sichtlinie |
| `src\mapgen\` | **Die Kartengeneratoren**, jeder mit eigenen Werten: `mod.rs` (Auswahl, Schieberegler, was alle teilen: Spawner, freie Basis, Wege in jede Ecke), `wild.rs` (Rauschen wie Factorio), `regions.rs` (Regionen mit Pässen wie TAB), `rings.rs` (Festungsringe), `canyons.rs` (Zellulärer Automat), `ores.rs` (Erzfelder wie Factorio), `tools.rs` (Flüsse, Bäume, Wege freischneiden) |
| `src\noise.rs` | Glattes Rauschen (Value Noise in mehreren Lagen) für die Generatoren |
| `src\mapfile.rs` | Karten speichern und laden (Editor „Save map“, Startbildschirm „Custom maps“) als JSON unter `%APPDATA%\Last Bastion\maps\` |
| `src\home.rs` | Der Startbildschirm: New game, Map editor, Custom maps, Settings – und die Seite zum Einstellen der Karte mit Vorschau |
| `src\picture.rs` | Ein Bild der Karte (ein Pixel pro Kachel) für Minimap, Vorschau und Hintergrund – nur neu gemalt, wenn sich etwas ändert |
| `src\widgets.rs` | Knöpfe, Schalter, Schieberegler, Textfelder für die Menüs (aus `menu.rs` herausgelöst) |
| `src\rng.rs` | Eigener Zufallsgenerator mit Seed |
| `src\iso.rs` | Isometrische Kamera: Welt ↔ Bildschirm |
| `src\ui.rs` | Maus und Tastatur → Auswahl, Kontrollgruppen, Kamera, Befehle |
| `src\cursor.rs` | Die echte Maus unter Windows: Fokus, Position, Maus im Fenster halten |
| `src\menu.rs` | Spielmenü (Esc) mit Einstellungen, „Main menu“ zurück zum Startbildschirm |
| `src\settings.rs` | Einstellungen als JSON-Datei unter `%APPDATA%\Last Bastion\settings.json` laden und speichern |
| `src\hud.rs` | Die Leiste unten wie in StarCraft: Minimap, Kontrollgruppen, Auswahl mit Werten, Befehlskarte |
| `src\editor.rs` | Oben rechts: Game View / Editor Mode (F10), der Editor mit Tabs wie in TAB: Map (Name, speichern), Terrain (malen), Entities (Einheiten, Gebäude, Zombies, Spawner), Waves (sofort und Zeitleiste) |
| `src\draw.rs` | Zeichnen (Platzhalter-Grafik) |
| `build.ps1` | Baut das Spiel (`cargo build --release`) und kopiert es nach `dist\LastBastion.exe` |
| `dist\LastBastion.exe` | Das fertig gebaute Spiel (wird gebaut, nicht in git) |

## Anbindung an die Windows-Toolbox

Das Spiel ist ein eigenes Repo und weiß nichts von der [Windows-Toolbox](https://github.com/ItsMaxxx/windows-toolbox).
Die Toolbox hat nur eine kleine Seite im Tab „Games“, die diesen Ordner kennt (Knopf „Spielordner wählen“, sonst sucht
sie `last-bastion` neben dem Toolbox-Ordner). Damit das klappt, gilt (DESIGN.md, Abschnitt 12.7):

- Das fertige Spiel liegt immer unter **`dist\LastBastion.exe`** – egal mit welcher Sprache gebaut.
- Das Design-Dokument heißt `DESIGN.md` und liegt im Hauptordner.
- Die Einstellungen des Spielers liegen an einer festen Stelle (DESIGN.md, Abschnitt 11.4).

## Rust lernen

Das Spiel wird in **Rust** geschrieben (DESIGN.md, Abschnitt 12.2) – Rust zu lernen ist ein eigenes Ziel.

1. **Einrichten:** `winget install Rustlang.Rustup` (Toolchain **MSVC**, der Standard). Dazu Visual Studio (Community
   oder nur die Build Tools) mit „Desktopentwicklung mit C++“ – daraus kommen der Linker `link.exe` und das Windows SDK,
   die Rust nicht selbst mitliefern darf. Im Editor **rust-analyzer** benutzen (hilft nur beim Schreiben, ersetzt
   Compiler und Linker nicht).
   - **Starten zum Ausprobieren:** `cargo run` (Debug-Build, schnell gebaut, mit Konsolenfenster für Ausgaben).
   - **Fertiges Spiel bauen:** `.\build.ps1` → `dist\LastBastion.exe` (Release-Build, optimiert, ohne Konsole).
2. **Das Rust-Buch** (kostenlos): [doc.rust-lang.org/book](https://doc.rust-lang.org/book/), auf Deutsch
   [rust-lang-de.github.io/rustbook-de](https://rust-lang-de.github.io/rustbook-de/). Wichtig fürs Spiel: Kapitel 1–10
   (Ownership, Borrowing, Structs, Enums, Traits), später 13 (Closures, Iteratoren) und 15 (Smart Pointers).
3. **Rustlings** ([github.com/rust-lang/rustlings](https://github.com/rust-lang/rustlings)): kleine Aufgaben passend
   zum Buch, gut zum Üben nebenher.
4. **Aufwärmen:** etwas Kleines mit der Grafik-Bibliothek, z. B. ein Quadrat, das der Maus folgt – dann der
   Wegwerf-Prototyp (DESIGN.md 12.2).

## Prototyp ausprobieren

Zurzeit ist `dist\LastBastion.exe` der **Wegwerf-Prototyp** (DESIGN.md 12.2): 8 Soldaten, 4 Scharfschützen und
4 Ranger gegen eine Horde, die zum blauen Kommandozentrum in der Mitte läuft. Die Zombies kommen nur aus **Spawnern** – zu
Beginn 3 auf jeder Seite der Karte, im Editor lassen sich weitere setzen, umbenennen, verschieben und vergrößern, und
für jeden einstellen, wie er Zombies schickt: dauernd (Getötete kommen wieder), alle X Sekunden N Stück, oder nur bei
Wellen.

**Startbildschirm:** New game, Map editor, Custom maps (im Editor gespeicherte Karten spielen oder bearbeiten),
Settings, Quit. „Continue“ und „Research“ sind als „kommt später“ zu sehen. **New game** und **Map editor** führen zur
**Karten-Einstellung**: links Generator, Seed (anklicken und tippen, oder „New seed“) und die Werte als Schieberegler
(Maus auf einem Regler = Erklärung), rechts sofort die **Vorschau** mit Spawnern (rosa) und Basis (blau). „Random values“
würfelt die Werte des Generators aus, „Defaults“ setzt sie zurück. Die letzten Werte merkt sich das Spiel
(`%APPDATA%\Last Bastion\map_setup.json`).

**Kartengeneratoren** – gleicher Seed + gleiche Werte = gleiche Karte (auch im Koop):

| Generator | Wie | Werte |
|---|---|---|
| **Wild** | Rauschen wie Factorio: Seen, Bergrücken, Wälder, Flüsse von Rand zu Rand, die um die Basis herumfließen | Größe der Formen, Verbiegen, Anteil Seen / Berge / Wald, Rücken statt Klumpen, Bäume, Anzahl / Breite / Kurven der Flüsse |
| **Regions** | Wie TAB: Regionen (Wiese, Wald, Felsen, See), zwischen ihnen Wände mit **Pässen** – die Engstellen zum Zumauern | Anzahl Regionen, wie gewunden, Wanddicke, Pässe pro Wand, Passbreite, Wände aus Berg / Wald / gemischt / Wasser, Anteil Wald-, Fels-, See-Regionen, Bäume |
| **Rings** | Festung: Ringe um die Basis, jeder mit ein paar Öffnungen, die nie übereinander liegen – der Weg hinein geht im Zickzack | Anzahl, Dicke, Öffnungen, Öffnungsbreite, Unebenheit, rund ↔ eckig, Material, Kram zwischen den Ringen, Bäume |
| **Canyons** | Zellulärer Automat wie bei Höhlen: Fels mit gewundenen Schluchten | Felsanteil, Glätten, Schluchtbreite, Material, Teiche, Bäume |
| **Blank** | Nur Gras (und Bäume, wenn man will) – zum Selberbauen im Editor | Bäume |

Für alle: **Kartengröße** (96–256, Standard 256 wie in TAB), freier Platz um die Basis, Spawner pro Seite, wie viele
**Zombies herumstreifen** und die **Erzfelder** (Anzahl, Größe, Reichtum). Danach sorgt jeder Generator dafür, dass
**jede offene Stelle die Basis erreicht** – für die Zombies überall, für deine Einheiten **innerhalb der Weltgrenze**: Ist
etwas abgeschnitten (hinter einem Fluss, in einem Ring, in einer Höhle), wird der Weg freigeschnitten, der am wenigsten
Hindernis durchbricht (2 Kacheln breit) – eine Furt, eine Lücke im Rücken. Winzige Löcher werden stattdessen Wald.

**Erz** (wie in Factorio): **Eisenerz** (rostbraun), **Kohle** (schwarz) und **Stein** (hellgrau) liegen als Felder auf
dem Gras – man kann darüber laufen und darauf bauen (Minen kommen mit der Wirtschaft). Direkt neben der Basis liegt immer
ein kleines Feld von jedem, weiter draußen werden sie größer und reicher. Holz kommt aus dem Wald.

**Zombies sehen** Einheiten nur ein kleines Stück weit wie in TAB (Walker und Fat 5 Kacheln, Runner 6, nicht durch Wald
und Bäume), **hören** sie auf 3 Kacheln (auch hinter einem Baum), und wer **angeschossen** wird, stürmt 5 Sekunden lang
los, auch ohne den Schützen zu sehen. Vor allem lockt aber der **Lärm**: Jeder Schuss ist ein Stück weit zu hören
(Ranger 1 Kachel, Soldat 3, Scharfschütze 10), und wer ihn hört, stürmt 20 s lang auf deine Einheiten los – durch Wald
und Mauern hindurch gehört. Ranger sind leise, Scharfschützen holen die halbe Gegend. Dann stürmen
sie auf die Einheit zu: um Wasser und Berge herum, aber **fast gerade durch Mauern** – die werden eingerissen, statt
dass die ganze Horde die Lücke sucht. Ohne Einheit in Sicht gehen sie auf Gebäude in 5 Kacheln los, sonst zum
Kommandozentrum (Gebäude im Weg umgehen sie nur, wenn der Umweg kurz ist). Einheiten können sterben, Gebäude
einstürzen. Wer **angeschossen** wird, sucht mit den 3 Zombies neben sich 10 s lang die **Lücke** in der Mauer, der Rest
haut weiter drauf. **Walker** (dunkelgrau, langsam) liegen von Anfang an verstreut auf der Karte, **Runner** (hellgrau,
schnell) kommen aus den Spawnern. **Fat** (gelblich braun, groß, 600 HP, Panzerung 2, haut mit 40 zu – TABs „Chubby“).
Zu Beginn **streifen** viele Zombies über die Karte (Standard etwa 3.900): nahe der Basis nur Walker, weiter draußen
auch Runner, am Rand auch Fat (`roaming` in `data\zombies.json`). Getötete Zombies kommen **nie wieder**; ab Tag 2 schickt der konstante Spawn neue
(`data\spawning.json`). Tote Einheiten stehen nach 2–5 s als Walker auf, Zelte mit gebrochenem Schild werden infiziert.
Einheiten, Zombies und Gebäude sind kleine isometrische Klötze (Zombies die kleinsten, Fat größer).

**Schießen wie in StarCraft** – jede Einheit hat dafür eigene Zeiten (`data\units.json`): Sie **dreht** sich erst zum
Ziel (Ranger 0,25 s für eine halbe Drehung, Soldat 0,35, Scharfschütze 0,5), **zielt** dann (Ranger spannt den Bogen
0,4 s, Scharfschütze 0,25, Soldat nur 0,1), schießt und **lädt nach**. Ein Schuss dauert Zielen + Nachladen, so bleibt
die Feuerrate wie in TAB (Ranger 1 pro s, Soldat 2, Scharfschütze 0,45). Wechselt das Ziel, fängt das Zielen von vorne
an – deshalb bleibt eine Einheit bei ihrem Ziel, solange es lebt und in Reichweite ist. Das kleine dunkle Stück an
jedem Klotz zeigt, wohin die Einheit schaut. Schüsse bis zum Tod: Walker Ranger 4 / Soldat 2 / Scharfschütze 1,
Runner 6 / 3 / 1. **Alle Werte** stehen in
`data\` (siehe [data\README.md](data/README.md)).

**Sicht und Nebel** (wie in StarCraft 2): Einheiten und Gebäude sehen nur ein Stück weit (Soldat 6 Kacheln,
Ranger 8, Scharfschütze 9, Medic 10, Kommandozentrum 11, Zelt 5, Mauer und Tor 3, Turm 6). Wo noch nie jemand war, ist die Karte
**schwarz**; was man schon gesehen hat, bleibt **abgedunkelt** sichtbar – nur die Zombies darin sieht man nicht. Ein
Zombie, der aus der Sicht verschwindet, **verblasst** in knapp einer Sekunde, statt plötzlich weg zu sein. Auf der
Minimap genauso (dort ohne Verblassen). Durch Wälder und Bäume kann niemand laufen, **sehen oder schießen** – eine
Einheit hinter einem Baum muss erst um ihn herum. Über Berge und Wasser kann man schauen.

Das Spielfeld (Standard 256 × 256) hat eine **Weltgrenze**: Ringsherum liegen noch 16 Kacheln, darin die Spawner.
Deine Einheiten können nicht darüber, die Kamera hört an der Grenze auf, und dahinter sieht man nichts – auch nicht die
Zombies, bis sie hereinkommen. Im **Editor Mode** (oben rechts) sieht man die ganze Karte (288 × 288), die Grenze ist
dort eine orange Linie. Oben rechts neben Game View / Editor Mode steht die **Uhr** der Runde (Tag und Stunde, 1 Tag = 3 Minuten) und wann die
nächste Welle der Zeitleiste kommt.

Alles im Spiel ist auf **Englisch** (auch die Kommentare im Code). Steuerung und Leiste unten wie in **StarCraft 2**:

- **Leiste unten:** links die Minimap (klicken/ziehen = Kamera, Rechtsklick = hinlaufen), in der Mitte oben die
  Kontrollgruppen, darunter die Auswahl (eine Einheit: alle Werte; mehrere: ein Symbol pro Einheit, Rand = Leben,
  der aktive Typ ist hell), rechts die Befehlskarte (Knöpfe mit Tooltip).
- Ausgewählte Einheiten zeigen ihre **ganze Befehlskette** als Linie. In den Einstellungen abschaltbar – dann wie in
  SC2 nur das aktuelle Ziel und die ganze Kette nur mit gedrückter Shift-Taste: grün = laufen/folgen, rot = Angriff, gelb = Patrouille. Oranger Ring = hält die Stellung.
  Ausgewählte und verletzte Einheiten haben einen Lebensbalken.
- Unter der Maus: Zombie mit rotem Ring (Rechtsklick greift an), eigene Einheit mit weißem Ring. Beim Ziehen des
  Auswahlrahmens sind die Einheiten darin schon hervorgehoben. Über einem Symbol unten steht Name und Leben.
- **Die Maus bleibt im Fenster** (wie SC2 „Mauszeiger im Fenster halten“), solange das Spiel vorne ist – nur so
  klappt das Scrollen am Rand, wenn rechts noch ein Bildschirm ist. Alt+Tab oder F12 geben sie frei.

| Taste | Was |
|---|---|
| Linke Maus (klicken / ziehen) | Einheit / Gebäude / alle im Rahmen auswählen, mit Shift dazu oder weg. Der Rahmen nimmt Gebäude nur, wenn keine Einheit drin ist. Trifft der Klick nichts, bleibt die Auswahl |
| Strg + Klick, Doppelklick | Alle Einheiten (oder Gebäude) dieses Typs auf dem Bildschirm (mit Shift: dazu) |
| Klick auf ein Symbol unten | Nur diese Einheit (dieses Gebäude). Shift: weg. Strg: nur dieser Typ. Strg+Shift: dieser Typ weg |
| F2 | Alle Einheiten |
| Tab / Shift+Tab | Nächster / voriger Typ in der Auswahl wird aktiv – **auch Gebäude** (z. B. Soldaten + Kommandozentrum: Tab, dann Q baut einen Soldaten). Die Befehlskarte zeigt immer den aktiven Typ |
| Rechte Maus | Alle laufen auf den angeklickten Punkt – auf einen Zombie: angreifen – auf eine eigene Einheit: ihr folgen – auf einen Turm: hineingehen. Ausgewählte Gebäude setzen dabei ihren Sammelpunkt |
| **Rechte Maus ziehen** | **Formation** (wie in *Diplomacy is Not an Option*): die Einheiten stellen sich an der gezogenen Linie auf. Beim Ziehen sieht man schon, wo jede hinkommt. Nur beim Ziehen – ein Klick ist keine Formation |
| X / C / V (oder Knöpfe unten auf der Befehlskarte) | Formation wählen: **Linie** (Reihen so breit wie gezogen) / **Block** (Quadrat, die Linie ist die Front) / **Kreis** (um den Startpunkt, gezogen = Radius). Soldaten vorne bzw. außen, dann Ranger, Scharfschützen und Medics dahinter |
| T + Linksklick | **Angriff / Attack-Move**: hinlaufen, unterwegs alles bekämpfen (auf einen Zombie: genau den). Auf eine eigene Einheit: Medics richten ihren grünen Strahl auf sie, alle anderen laufen per Attack-Move hin. **Ziehen** = in Formation |
| P + Linksklick | **Patrouille**: hin und her, unterwegs wie Attack-Move. Ziehen = in Formation |
| M + Linksklick | Hinlaufen ohne zu schießen (auf eine Einheit: folgen). Ziehen = in Formation |
| S / H | Stopp (Kette löschen) / Halten (stehen bleiben, nur schießen, nicht wegschubsen lassen) |
| **Shift** + Befehl | Befehl **hinten an die Kette hängen**. Mit Shift bleibt A/P/M aktiv, so setzt man mehrere Wegpunkte |
| Rechtsklick / Esc bei A, P, M | Wartenden Befehl abbrechen |
| Strg + 1–0 | Kontrollgruppe setzen – mit Einheiten **und Gebäuden** (z. B. Strg+5 aufs Kommandozentrum) |
| Strg + Shift + 1–0 | Kontrollgruppe setzen und die Einheiten / Gebäude aus allen anderen Gruppen nehmen („stehlen“) |
| Shift + 1–0 | Auswahl zur Gruppe dazu |
| 1–0 (oder Klick auf die Gruppe) | Gruppe auswählen, zweimal schnell: Kamera springt hin |
| Strg / Shift / Strg+Shift + Klick auf eine Gruppe | wie mit der Zahlentaste: Gruppe durch die Auswahl ersetzen / Auswahl dazu / stehlen |
| **WASD**, Pfeiltasten, Maus am Fensterrand, Mausrad | Kamera schieben, zoomen. **WASD nur, solange nichts ausgewählt ist** – mit Einheiten oder einem Gebäude gehören die Tasten der Befehlskarte (S = Stopp, A = Ranger …). Pfeiltasten, Rand und Mausrad gehen immer |
| Mittlere Maustaste ziehen | Karte anfassen und verschieben |
| Strg + F5–F8 / F5–F8 | Kamera-Position merken / hinspringen |
| Rücktaste | Kamera zur Basis |
| Leertaste | Pause: Befehle gehen weiter und **sind sofort zu sehen** (Linien, Bauschlange, Sammelpunkte, neue Gebäude) – ausgeführt wird erst, wenn es weitergeht |
| `+` / `-` | Tempo ×1 / ×2 / ×4 / ×8 |
| Esc | **Spielmenü** (wenn kein Befehl, kein Baumenü und kein Editor-Werkzeug wartet und kein Spawner ausgewählt ist): Resume, Settings, Restart, New map, Quit. Hält alles an |

**Kommandozentrum** – Tasten im **Raster wie in StarCraft** (die Taste ist da, wo der Knopf sitzt):

```
Q Soldat    W Ranger    E Energie ▸   R Scharfschütze   T Medic
A Spezial ▸ S Zelt      D Verteidigung ▸  F –           G Sammelpunkt
```

| Taste | Was |
|---|---|
| Q / W / R / T | Soldat (5 s) / Ranger (4 s) / Scharfschütze (8 s) / Medic (6 s) in die Bauschlange, mit Shift gleich 5. Höchstens 5 in der Schlange. Mehrere Kommandozentren ausgewählt: jede Einheit ins kürzeste |
| Klick auf ein Symbol der Bauschlange | Abbrechen (wenn nur das Kommandozentrum ausgewählt ist) |
| G + Linksklick, oder Rechtsklick | **Sammelpunkt**: neue Einheiten laufen dorthin – auf eine Einheit: sie folgen ihr |
| S | **Zelt** (2 × 2) aufs Raster stellen: grün = Platz frei, rot = besetzt |
| D | **Verteidigung** (Baumenü): **Q** Mauer, **W** Tor, **E** Turm. **Esc** zurück |
| E / A | **Energie** / **Spezial** (Baumenüs) – noch leer, die Gebäude kommen mit der Wirtschaft |
| D, Q | **Mauer** (1 × 1) |
| D, W | **Tor** (3 × 1): durch die Mitte gehen deine Einheiten, Zombies nicht. **R** dreht es beim Platzieren. Darf **auf Mauern** gesetzt werden – die Mauern darunter verschwinden, das Tor wird billiger (lohnt sich trotzdem nicht: Mauern + Tor kosten mehr als das Tor allein) |
| D, E | **Turm** (1 × 1). Zwischen zwei Türmen muss **1 Kachel frei** bleiben (auch über Eck) |
| Beim Bauen: Linke Maus **ziehen** | Eine ganze Reihe von der Start-Kachel bis zur Maus. Mauern schräg gezogen wie in TAB **1 breit**, Ecke an Ecke (dicht – niemand kommt durch die Ecke); Türme genauso, aber mit einer Lücke dazwischen; Zelte als Treppe; ein Tor immer einzeln. Rote Plätze werden ausgelassen |
| Beim Bauen | Das Gebäude **bleibt an der Maus** – gleich die nächste Reihe. **Rechtsklick oder Esc** legt es weg |

**Mauern verbinden sich** (Test-Skin): Eine Mauer allein ist ein Block; sonst ein Pfosten in der Mitte und ein Stück
Mauer zu jedem Nachbarn – gerade oder schräg zur Ecke. Die Pfosten eines Tores und Türme verbinden sich genauso.
Die Tür des Tores steht offen, solange eine deiner Einheiten nah dran ist.

**Turm** (600 HP, 50 % mehr als die Mauer): **Rechtsklick** mit ausgewählten Einheiten auf den Turm – bis zu **4**
Soldaten, Ranger oder Scharfschützen gehen hinein (Medics bleiben draußen, sie haben nichts zu schießen). Drinnen schießen und
sehen sie **2 Kacheln weiter**, können nicht getroffen werden (die Zombies hauen auf den Turm) und sind nicht
auswählbar, bleiben aber in ihren Kontrollgruppen. Auf dem Dach sitzt ein kleiner Block pro Einheit. Turm anklicken,
**Q** = alle raus, **G** oder Rechtsklick = Sammelpunkt: Beim Rauslassen kommen sie auf dieser Seite heraus und laufen
dorthin (ohne Sammelpunkt: Seite zur Basis). Fällt der Turm, stehen sie wieder auf dem Boden.

**Nicht in der Nähe von Zombies bauen:** Auf dem Bauplatz und 4 Kacheln rundherum darf kein Zombie stehen – bei einer
Mauer (1 × 1) also 9 × 9 Kacheln, bei einem Zelt (2 × 2) 10 × 10. Der Rahmen um das Gebäude an der Maus zeigt dieses
Feld, rot heißt: ein Zombie ist drin. Auf eigene Einheiten kann man auch nicht bauen, und nicht hinter der Weltgrenze.

Zelte zahlen 12 Gold pro Spielstunde und haben 150 HP + 133 % davon als Schild (Schild weg = infiziert). **Leben und
Schild sind ein Balken**: vorne die HP in ihrer Farbe, dahinter der Schild in Graublau; unten im Panel steht der Schild in
Prozent. Mauern und Tore haben 400 HP, Türme 600. Das
Kommandozentrum hat 1.500 HP.
Kosten und Bauzeit für Gebäude gibt es noch nicht – sie stehen sofort.

**Medic** (weiß, wie in StarCraft): greift nie an. Steht er still oder läuft per Attack-Move, richtet er seinen grünen
Strahl von selbst auf verletzte Einheiten in 4 Kacheln Umkreis; mit T auf eine Einheit auf genau die. **Heilen tut der
Strahl noch nicht** – das kommt als Nächstes.

**Formationen** statt Magic Box: Nur wer **zieht** (rechte Maus, oder T/P/M + linke Maus), stellt die Einheiten in der
gewählten Formation auf (Linie, Block oder Kreis, Standard: Linie) – Ziehen bestimmt Breite und Richtung (beim Kreis den
Radius). Ein einfacher **Klick** schickt alle auf den angeklickten Punkt, bei jedem Befehl (Laufen, Angriff, Patrouille,
auch auf der Minimap). Steht ein Baum auf einem Platz, nimmt die Einheit den nächsten freien.

**Wege:** Einheiten und Zombies laufen in **jedem Winkel gerade** und um Hindernisse straff von Ecke zu Ecke – nicht mehr
erst schräg, bis eine Achse passt, und dann geradeaus (Sichtlinien-Flow-Field, DESIGN.md 12.1).

**Einstellungen** (Esc → Settings) gelten sofort und landen in `%APPDATA%\Last Bastion\settings.json`:
Rand-Scrollen, Scroll-Tempo, Maus im Fenster halten, Fenster oder randloses Vollbild,
Lebensbalken immer zeigen, ganze Befehlskette immer zeigen, Messwerte. Der Knopf „Open settings folder“ zeigt die Datei im Explorer.

**Game View und Editor Mode** (DESIGN.md 12.6) – zwei Knöpfe oben rechts. Im **Game View** sieht man das Spiel wie ein
Spieler, ohne Entwickler-Werkzeuge. Im **Editor Mode** sieht man die ganze Karte ohne Nebel und darunter alle Werkzeuge
als Knöpfe, damit die Tasten fürs Spiel frei bleiben – aufgeteilt in **Tabs wie im Editor von TAB**: **Map** (Name,
speichern, neue Karte), **Terrain** (malen), **Entities** (Einheiten, Gebäude, Zombies, Spawner), **Waves** (sofort und
Zeitleiste). Darunter immer „View“. Die Abschnitte lassen sich mit einem Klick auf die Überschrift auf- und zuklappen
([-] / [+]); ist das Panel zu lang, scrollt das Mausrad darauf. **Gebäude** (Tab Entities oder über das
Kommandozentrum) gehen im Editor **überall hin** – auf Wasser, hinter die Grenze, neben Zombies, auf Einheiten (die werden weggeschoben); nur
nicht auf ein anderes Gebäude. Das Gelände darunter bleibt, wie es ist (Wasser bleibt Wasser) und ist wieder zu sehen,
wenn das Gebäude fällt. Durch die Tür eines Tores auf Wasser kommen Einheiten trotzdem nicht.

| Taste / Knopf | Was |
|---|---|
| Game View / Editor Mode, oder F10 | Zwischen beiden wechseln |
| Tab **Map**: Name (rename) | Name der Karte anklicken und tippen – er wird der Dateiname |
| Save map | Speichert Gelände, Spawner, Gebäude, Einheiten, gesetzte Zombies und Zeitleiste nach `%APPDATA%\Last Bastion\maps\<Name>.json`. Auf dem Startbildschirm unter „Custom maps“ spielen oder bearbeiten |
| New map... | Zur Karten-Einstellung, mit den Werten dieser Karte – „Open in the editor“ macht die neue Karte |
| View: All / Player / Zombies | **Durch wessen Augen**: alles ohne Nebel / der Nebel des Spielers wie im Game View / was die Zombies sehen (dunkelrot = kein Zombie schaut hin). Im Koop teilen sich beide Spieler die Sicht, darum gibt es nur einen „Player“ |
| Mountain / Forest / Tree / Water / Erase | **Gelände malen**: Werkzeug wählen, dann mit gedrückter linker Maus übers Feld ziehen – sofort, auch während das Spiel läuft. Erase macht wieder Gras. Gebäude werden nie übermalt |
| Iron ore / Coal / Stone / No ore | **Erz malen** (nur auf Gras), genauso mit dem Pinsel. „No ore“ nimmt es weg. Wird gespeichert |
| 1 / 3 / 5 / 9 / 15 / 25 / 41 | Pinselgröße in Kacheln – fürs Gelände und den Zombie-Pinsel |
| + Spawner | Werkzeug: ein Klick setzt einen **Spawner** (6 × 6) und wählt ihn gleich aus |
| Klick auf einen Spawner (ohne Werkzeug) | Auswählen: der **Rand leuchtet**. Rand (oder Ecke) ziehen = Größe ändern, Innenfläche ziehen = verschieben |
| Name des Spawners (Feld im Panel) | Anklicken und tippen, Enter übernimmt, Esc bricht ab. Solange man tippt, tun die Tasten nichts anderes |
| Constant / Periodic / Off (Spawner ausgewählt) | Wie der Spawner von selbst Zombies schickt: **constant** (macht beim konstanten Spawn der Runde mit, `data\spawning.json`) / **alle X Sekunden N Runner** / **gar nicht** (nur Wellen) |
| − every … s + / − … zombies + | Bei „Periodic“: Abstand (1–120 s) und Anzahl (1–500) in Stufen. Ein Klick darauf stellt den Spawner auf „Periodic“ |
| Remove spawner, Entf | Ausgewählten Spawner löschen. Esc oder Klick daneben: Auswahl weg |
| Zombies: **Brush** | Werkzeug: Solange die linke Maus gedrückt ist, landen **jeden Takt** so viele Zombies auf der Pinselfläche (1–20 pro Takt, mit − / +). In der Pause kommt keiner – es ist pro Takt |
| Zombies: **Drop** | Werkzeug: jeder Linksklick setzt auf einmal so viele Zombies (1–1.000, mit − / +) – auf der Pinselfläche, bei vielen etwas größer |
| Walker / Runner / Fat | Welche Zombies Pinsel und Drop setzen |
| Storm the CC / Wander | Ob sie zum Kommandozentrum stürmen oder **herumwandern**: in der Nähe stehen bleiben und langsam umherschlendern (höchstens 4 Kacheln weit). Sobald sie eine Einheit sehen, hören, angeschossen werden, stürmen sie für immer los |
| Waves: Small / One side / Two sides | **Wellen** aus den Spawnern: 150 Runner aus einem zufälligen Spawner / 500 aus den Spawnern einer Seite / je 500 von zwei zufälligen Seiten |
| +1000 from all spawners | 1.000 Zombies verteilt auf alle Spawner (Belastungstest) |
| Remove all | Alle Zombies weg. Getötete Zombies kommen nie wieder |
| Soldier / Sniper / Medic | Werkzeug: jeder Linksklick setzt eine Einheit an die Maus (nur innerhalb der Grenze) |
| CC / Tent / Wall / Gate / Tower | Gebäude an die Maus hängen – ohne Spielregeln, Ziehen baut eine Reihe, R dreht das Tor |
| Tab **Waves**: Zeitleiste | Wellen, die **von selbst** kommen, wie die Ereignisse in TAB („Tag 12, 6 Uhr“). „+ Add wave“: einen Tag nach der letzten. Pro Welle: − Zeit + (6 Stunden, mit Shift ein Tag), − Anzahl + (10–2.000), woher (1 Spawner / 1 / 2 / 3 / alle Seiten), x löscht. Vergangene Zeiten sind grau |
| Rechtsklick, Esc, Knopf nochmal | Werkzeug weglegen |
| F3 | Messwerte und Tastenhilfe ein/aus (FPS, ms pro Takt, Prüfsumme) |
| F12 | Maus im Fenster halten an/aus (Notausgang, steht auch im Menü) |

Berg, Wald, Baum und Wasser versperren vorerst alle gleich den Weg – für Einheiten und Zombies, und einreißen können
Zombies sie nicht. Wald und Bäume nehmen zusätzlich die Sicht. Wer auf neuem Gelände steht, wird an die nächste freie Stelle geschoben; Einheiten unterwegs suchen sich
sofort einen neuen Weg (auch bei neuen Gebäuden – aus denen muss aber niemand geschoben werden, weil man nicht auf
Einheiten und nicht neben Zombies bauen kann). Ist die Basis ganz abgeschnitten (z. B. ein Wasserring),
kommen einfach keine neuen Zombies – das Spiel hängt nicht.

**Gleiche Karte wiederholen:** Der Seed steht in den Messwerten (F3) und auf der Karten-Einstellung – dort eintippen.
`LastBastion.exe --seed 12345` (bzw. `cargo run -- --seed 12345`) überspringt den Startbildschirm und startet diese
Karte mit den zuletzt eingestellten Werten; „Restart“ im Menü behält Seed und Werte, „New map“ würfelt nur den Seed neu,
„Main menu“ geht zurück zum Startbildschirm.

**Determinismus selbst prüfen:** Mit festem Seed starten (`--seed 12345`), nichts anklicken, 30 Sekunden warten. Die
Zeile „Prüfsumme Takt 600“ ist bei jedem Start gleich (solange die Werte der Karte gleich bleiben) (ebenso nach „Restart“). Der automatische Test dafür:
`cargo test`.

## Fahrplan

Die Prototyp-Schritte stehen in [DESIGN.md, Abschnitt 13](DESIGN.md#13-prototype-roadmap) – davor die Liste
„Before step 1“ (Sprache, Koop, Speichern, wer baut, Wirtschaft in einer Tabelle durchrechnen …). Dann zuerst die
Einheitensteuerung, dann der Horden-Stresstest, nach Schritt 3 eine Mini-Runde: Macht das Spiel Spaß? Danach dieselbe
Mini-Runde im Koop über das Netzwerk (Schritt 5).

## Stand

- **2026-10-15:** Konzept als `DESIGN.md` angelegt (zuerst in der Windows-Toolbox unter `programs\last_bastion\`).
- **2026-10-15:** Einheiten überarbeitet (TAB-Infanterie, SC-Medic, Siege Tank, MG-Schütze); Blight heißt jetzt Creep;
  keine Bosse (nur Riesen wie in TAB); keine Förderbänder; Meta-Fortschritt wie in TAB ohne Kampagne; Endlos-Modus
  statt Kampagne/Daily Challenge; Sprache Rust oder C-Familie.
- **2026-10-15:** DESIGN.md ergänzt: „Before step 1“-Liste und Mini-Runde (neuer Schritt 4) im Fahrplan, Simulation
  in festen Takten, Entwickler-Werkzeuge, Einstellungen in einer JSON-Datei (11.4), neue offene Fragen 13–16.
  Hausstufe 1 heißt „Tent“ (Zelt).
- **2026-10-15:** Eigenes Repo, getrennt von der Windows-Toolbox. Die Toolbox startet `dist\LastBastion.exe`.
- **2026-10-15:** Sprache: **Rust**. Bibliothek noch offen, Tendenz macroquad (DESIGN.md 12.2, Frage 17).
- **2026-10-15:** Rundenlänge **2–4 Stunden** statt 30 Minuten: Anzahl der Tage wählbar wie in TAB, aber längere
  Tage und dafür weniger (z. B. 40–80, 1 Tag ≈ 3 min), Phasen und Wellen als Anteil der Runde (DESIGN.md 4). Speichern mitten in der Runde ist damit
  praktisch Pflicht (Frage 9), neue Frage 18 (Länge eines Tages).
- **2026-10-15:** **Koop** ist gewünscht: Ein Spieler hostet im Spiel, keine eigenen Server. Technik: Lockstep (nur
  Befehle werden verschickt), dafür muss die Simulation streng deterministisch sein (DESIGN.md 12.8). Neuer Schritt 5
  „Co-op mini round“ im Fahrplan, neue Fragen 19 (gemeinsame Kolonie?) und 20 (wie finden sich die Spieler?).
- **2026-10-15:** Erster Code: Rust-Projekt mit **macroquad** angelegt (MSVC-Toolchain). `src\main.rs` öffnet ein
  1280×720-Fenster „Last Bastion“ mit Titel und FPS-Anzeige, Esc beendet. `build.ps1` baut nach
  `dist\LastBastion.exe`.
- **2026-10-15:** Neue Regel für Claude: Neuen Code Stück für Stück erklären, mit Python-Vergleich und Spickzettel.
- **2026-10-15:** „Before step 1“: *Build auf frischem PC* abgehakt (rustup + Visual Studio mit C++ + `build.ps1`,
  DESIGN.md 12.7). Bei der *Library* steht jetzt: macroquad läuft, Bestätigung durch den Wegwerf-Prototyp fehlt noch.
- **2026-10-15:** Offene Fragen beantwortet (DESIGN.md 14):
  - **Bauen wie TAB** (F5): Gebäude bauen sich selbst, keine Arbeiter-Einheiten. **Kolonisten sind nur Deko**, fliehen
    bei Zombies ins nächste Haus in der Nähe – ohne TABs Hin-und-her-Rennen quer über die Karte (5.6).
  - **Speichern wie Factorio** (F9): frei, Autosave, Schnellspeichern, Lade-Bildschirm mit Datum, Spielzeit, Tag und
    Screenshot (neu: 11.5). Einstellungen und Spielstände unter `%APPDATA%\Last Bastion` (F16).
  - **Daten als JSON-Dateien** in `data\`, keine Datenbank (F14): werden einmal in Rust-Structs geladen, also genauso
    schnell; Änderungen ohne Neustart per Hot Reload. Die Wirtschaft wird später mit einer kleinen Rechnung über
    dieselben Dateien geprüft statt in einer Tabelle (12.4).
  - **Isometrisch wie TAB** (F2). Wichtigste Säule (F13): die TAB-Seite – Horde gegen eine Basis im Raster.
  - Munition vorerst gratis (F6), Held auf den Parkplatz (F8), *Last Bastion* gilt als endgültiger Name (F11).
  - **Koop** (F19, F20): erst nur eine gemeinsame Kolonie, beide steuern alles – aber was der eine benutzt, kann der
    andere nicht steuern. 2 Spieler, keine Pause im Koop (F7), Esc-Menü pausiert nicht.
  - Kleinere: Straßen machen Einheiten **und Zombies** schneller; Zelte kosten Holz und brauchen Nahrung + Strom,
    Steinhäuser zusätzlich Stein und Werkzeug; genau eine Fähigkeit pro Einheit; Sammelpunkt auch auf eine Einheit;
    Formationen wie SC2; **jeder Flächenschaden trifft auch die eigenen Leute** (auch bei Zombies); keine
    Lufteinheiten und kein Crawler.
  - Neue offene Fragen: 21 (Zeitalter), 22 (woher die Bilder), 23 (Strom für die ersten Zelte), 24 (brauchen Gebäude
    Arbeiter?), 25 (Koop: wann ist eine gesperrte Einheit wieder frei?).
- **2026-10-15:** Widersprüche geklärt:
  - **Kolonisten** sind eine Ressource wie in TAB: Häuser liefern sie, Gebäude brauchen sie (sonst abgeschaltet),
    Einheiten verbrauchen sie. Nur die herumlaufenden Figuren sind Deko (F24, DESIGN.md 5.6).
  - **Erster Strom:** etwas vom Kommandozentrum, dazu kleine, ineffiziente Holzgeneratoren (F23).
  - **Tasten wie SC2:** F2 = ganze Armee, Kamera-Lesezeichen auf F5–F8; alle Tasten in den Einstellungen änderbar.
  - **Esc** pausiert im Einzelspieler alles, nur noch das Menü geht; die Leertaste-Pause erlaubt weiter Befehle. Im
    Koop pausiert nichts.
  - **Koop:** Eine Einheit gehört dir, solange sie in deiner Kontrollgruppe ist oder deinen Befehl ausführt (F25).
    Speichern darf jeder (und allein weiterspielen), laden mitten in der Runde nur der Host.
  - Fahrplan-Reihenfolge bleibt; Zeitalter (F21) bleibt vorerst offen.
- **2026-10-15:** Fehlen Kolonisten, werden **zuerst Sammelgebäude** abgeschaltet, reihum über die Rohstoffe verteilt.
  Eine Einheit kostet 1 Kolonisten (Infanterie), Fahrzeuge mehr. „Grundlagen lernen“ abgehakt: Rust lernen wir beim
  Bauen, jeder Code wird erklärt. Feste Takte, Seed und Entwickler-Werkzeuge kommen in den Wegwerf-Prototyp.
- **2026-10-15:** Farmen und Stromerzeuger werden als Letzte abgeschaltet (sonst Abwärtsspirale).
- **2026-10-15:** **Wegwerf-Prototyp** läuft: isometrisch, 10 steuerbare Einheiten, 2.000 Zombies im Flow Field
  (mit Sichtlinie), räumliches Raster, feste Takte mit weichem Zeichnen, eigener Zufallsgenerator, Prüfsumme und
  Determinismus-Test. Gemessen: 10.000 Zombies bei 59 FPS, 2,1 ms Simulation pro Takt. macroquad ist schnell genug;
  offen ist noch, wie sich die Steuerung anfühlt. Gefunden: macroquad liest Tasten nach US-Position (DESIGN.md 12.2),
  und WASD für die Kamera beißt sich mit A/S (11.2).
- **2026-10-15:** `SPICKZETTEL.md` angelegt: alle bisher erklärten Rust-Begriffe mit Python-Vergleich, nach Themen
  sortiert, dazu die Spiel-Technik-Begriffe. Wächst mit jedem neuen Code mit.
- **2026-10-16:** **Steuerung wie StarCraft** im Prototyp:
  - **Befehle:** Attack-Move (A), Patrouille (P), Laufen (M, Rechtsklick), Angriff auf einen bestimmten Zombie
    (Rechtsklick oder A-Klick auf ihn), Stopp (S), Halten (H).
  - **Befehlsketten:** Mit Shift wird jeder Befehl hinten angehängt, auch Attack-Move, Patrouille und Halten. Die Kette
    der ausgewählten Einheiten ist als farbige Linie zu sehen.
  - **Kontrollgruppen** 1–0 (Strg = setzen, Shift = dazu, zweimal = Kamera hin), F2 = alle.
  - **Kamera** auch mit der Maus am Fensterrand (F11 schaltet es aus). Haken: macroquad meldet nicht, wenn die Maus das
    Fenster verlässt. Wer sie langsam über den Rand schiebt, dem scrollt die Kamera weiter (DESIGN.md 12.2).
  - **Zombie-Werkzeuge:** an der Maus (F5/F6), Welle von einer Seite (F7), alle weg (F8), Nachschub an/aus (F9).
  - Zombies haben jetzt eine feste Nummer (`id`), damit sich ein Angriffsbefehl den richtigen merkt. Tote Zombies
    werden erst am Ende des Takts entfernt. Neue Tests für Befehlskette, Patrouille und Angriff (5 Tests).
  - Haken für später: F5–F8 sind im Design die Kamera-Lesezeichen (11.2). Die Entwickler-Tasten ziehen um, sobald es
    die Lesezeichen gibt.
- **2026-10-16:** **Alle SC2-Steuerbefehle und eine Leiste unten wie in SC2:**
  - **Zwei Einheitentypen** mit eigenen Werten: Soldat (HP 100, Panzerung 1, Schaden 10, Reichweite 6) und
    Scharfschütze (HP 60, Schaden 35, Reichweite 10, langsam). Start: 8 Soldaten + 4 Scharfschützen, 500 Zombies.
  - **Zombies schlagen zu:** Ist eine Einheit näher als 5 Kacheln, gehen sie auf sie los (6 Schaden pro Sekunde
    minus Panzerung). Einheiten können sterben und werden aus Auswahl und Gruppen gestrichen. Zombies und Einheiten
    schubsen sich jetzt gegenseitig.
  - **Neue Befehle:** Folgen (Rechtsklick oder M auf eine eigene Einheit), Rechtsklick auf die Minimap,
    Befehle per Klick auf die Befehlskarte. **Magic Box**: Gruppen behalten beim Laufen ihre Aufstellung.
  - **Auswahl:** Strg-Klick und Doppelklick (alle dieses Typs auf dem Bildschirm), Klicks auf die Symbole unten, Tab
    für den aktiven Typ. Ein Klick ins Leere behält die Auswahl (wie in SC2).
  - **Gruppen und Kamera:** Gruppen stehlen (Strg+Shift+Zahl – SC2 nimmt dafür Alt, das piept aber vermutlich unter
    Windows mit macroquad), Kamera-Lesezeichen (Strg+F5–F8 / F5–F8), Rücktaste = Basis, Minimap zum Klicken und Ziehen.
  - **Leiste unten:** Minimap, Kontrollgruppen mit Anzahl, Auswahl (eine Einheit: Werte, Kills, aktueller Befehl;
    mehrere: Symbole mit Lebensbalken), Befehlskarte mit Tooltips. Lebensbalken über verletzten Einheiten.
  - Die Entwickler-Tasten sind von F5–F9 auf den Block **Einfg/Entf/Pos1/Ende/Bild** umgezogen (F5–F8 sind jetzt die
    Lesezeichen). Neu: Einheiten an der Maus erzeugen.
  - Einheiten haben jetzt wie Zombies eine feste `id`. 7 Tests (neu: Zombies töten eine Einheit, Magic Box).
  - Noch nicht drin: Fähigkeiten (bewusst später), Smart Targeting, Seiten in der Auswahl bei sehr vielen Einheiten.
- **2026-10-16:** **Rand-Scrollen repariert, mehr SC2-Kleinigkeiten:**
  - **Maus am Rand / zweiter Bildschirm:** macroquad merkt nicht, wann die Maus das Fenster verlässt oder das Fenster
    den Fokus verliert. Neu `src\cursor.rs`: fragt Windows direkt nach Fokus und echter Mausposition und hält die Maus
    wie SC2 im Fenster fest, solange das Spiel vorne ist. Am Rand stößt sie jetzt an, statt auf den rechten Bildschirm
    zu rutschen, und ohne Fokus scrollt nichts mehr. Alt+Tab gibt die Maus frei, F12 schaltet das Festhalten ab.
  - **Klick auf eine Kontrollgruppe mit Strg / Shift / Strg+Shift:** ersetzt die Gruppe durch die Auswahl / fügt sie
    hinzu / stiehlt – genau wie Strg/Shift + Zahl. Ein Klick ohne Zusatztaste wählt die Gruppe weiter aus.
  - **Mittlere Maustaste** ziehen verschiebt die Karte.
  - **Hervorheben wie in SC2:** Zombie unter der Maus rot, Einheit weiß, Vorschau beim Auswahlrahmen, Tooltip über den
    Symbolen der Auswahl.
  - **Befehlskette nur mit Shift ganz** (sonst nur das aktuelle Ziel), Lebensbalken auch über ausgewählten Einheiten.
- **2026-10-16:** **Spielmenü und Einstellungen, Magic Box gezähmt:**
  - **Esc öffnet das Spielmenü** (statt das Spiel zu beenden): Weiter, Einstellungen, Neu starten, Neue Karte, Spiel
    beenden; Speichern/Laden ist als „kommt später“ zu sehen. Solange es offen ist, steht alles (DESIGN.md 7.1).
  - **Einstellungen** wie in DESIGN.md 11.4 als JSON-Datei unter `%APPDATA%\Last Bastion\settings.json`: Rand-Scrollen,
    Scroll-Tempo, Maus festhalten, Magic Box für Laufen und für Angriff/Patrouille, Fenster/randloses Vollbild,
    Lebensbalken immer, Messwerte. Fehlende oder kaputte Werte → Standardwerte, nie ein Absturz. F3/F11/F12 ändern
    dieselben Einstellungen. Neue Bibliotheken: `serde` und `serde_json`.
  - **Magic Box:** In SC2/Brood War gibt es eine feste Höchstgröße (BW: 195 × 195 Pixel für Bodeneinheiten). Ist die
    Gruppe breiter, laufen alle zum Punkt. Bei uns jetzt 6 Kacheln statt 12. Wie SC2 es bei Attack-Move genau
    macht, war nicht herauszufinden – deshalb ist die Magic Box für Angriff/Patrouille im Menü getrennt abschaltbar.
    Die Einstellung reist mit dem Befehl, damit sie im Koop für beide PCs gleich ist.
  - 10 Tests (neu: Einstellungsdatei unvollständig / kaputt / speichern und laden; Magic Box zu verstreut).
- **2026-10-16:** **Kommandozentrum, Zelte, WASD:**
  - **Angriff liegt auf T** statt A. **WASD bewegt die Kamera** – außer die Auswahl braucht die Taste selbst
    (bei Einheiten ist S = Stopp, beim Kommandozentrum W = Scharfschütze). Mit Strg nie Kamera.
  - **Das Kommandozentrum ist ein echtes Gebäude** (3 × 3, 1.500 HP) und anklickbar. Es baut testweise Soldaten (Q)
    und Scharfschützen (W) mit Bauschlange (5 Plätze, Klick bricht ab), Sammelpunkt per R oder Rechtsklick (auch auf
    eine Einheit), und stellt **Zelte** (B) aufs Raster – mit grün/roter Vorschau, Shift für mehrere.
  - **Gebäude versperren den Weg** für Einheiten und Zombies. Ein Klick auf ein Gebäude oder einen Felsen schickt die
    Einheiten an die nächste freie Stelle daneben.
  - **Zombies schlagen auf Gebäude ein**, die sie berühren. Zerstörte Gebäude verschwinden und geben den Platz frei.
    Die Horde läuft jetzt aufs Kommandozentrum zu und greift es an. Verlieren kann man dadurch noch nicht.
  - **R und N** (Neustart, neue Karte) gibt es nur noch im Menü – R ist jetzt der Sammelpunkt.
  - 15 Tests (neu: Kommandozentrum versperrt die Basis, Zelte nur auf freien Platz, Zombies zerstören ein Zelt,
    Bauen + Sammelpunkt, Bauschlange voll / abbrechen).
- **2026-10-16:** **Mauern, Bauen durch Ziehen, Zombies greifen Gebäude an:**
  - **Zombies zielen auf Gebäude wie auf Einheiten:** Das Nähere in 5 Kacheln wird angegriffen. Steht eine Einheit
    hinter einer Mauer, ist die Mauer näher – sie wird eingerissen. Vorher liefen Zombies um alles herum, was nicht
    direkt vor ihnen stand.
  - **Der Weg der Horde geht durch Gebäude**, wenn außen herum mehr als \10 Kacheln Umweg wären (Flow Field jetzt mit
    Dijkstra und „Umweg-Preis“ statt Breitensuche). Nebenbei behoben: War das Kommandozentrum ganz zugebaut, galt die
    Basis als unerreichbar – dann wäre das Spiel beim nächsten Zombie-Nachschub hängen geblieben.
  - **Jäger:** Jeder 4. Zombie wittert Einheiten bis 12 Schritte zu Fuß (um Mauern herum, neu `src\hunt.rs`) und geht
    durch Lücken auf sie los. Nur ein Teil, damit die Mauern nicht egal werden. Später könnte das der Runner übernehmen
    (DESIGN.md 8.1).
  - **Mauer** (1 × 1, 400 HP, Taste V) beim Kommandozentrum. **Bauen durch Ziehen** für Mauern und Zelte: gerade oder
    schräg als lückenlose Treppe, mit grün/roter Vorschau für jedes Stück. Gebaut wird beim Loslassen.
  - Viele Gebäude in einem Takt (eine gezogene Reihe) rechnen den Zombie-Weg nur einmal neu.
  - 18 Tests (neu: Zombies brechen durch einen Mauerring, Jäger finden die Lücke und die anderen hauen auf die Mauer,
    gezogene Reihe ohne Lücke).
- **2026-10-16:** **Gelände-Editor, Entwickler-Werkzeuge als Knöpfe, alles auf Englisch:**
  - **Editor-Panel oben rechts (F10):** Berg, Wald, Wasser und Radierer mit Pinsel (1/3/5/9) live aufs Feld malen –
    zum Testen von Verhalten und Wegfindung, ohne dass Zombies die Hindernisse einreißen. Gebäude werden nie übermalt.
  - **Wegschieben:** Wer auf neuem Gelände oder einem neuen Gebäude steht, landet an der nächsten freien Stelle.
    Gebäude dürfen deshalb jetzt auch auf Zombies und Einheiten gebaut werden (wie in SC2, wo Einheiten Platz machen).
  - **Neue Wege:** Ändert sich die Karte, rechnen Einheiten unterwegs ihre Wege neu (eine Gruppe teilt sich weiter
    einen Weg). Vorher wären sie geradewegs gegen den neuen Berg gelaufen.
  - **Kein Hänger bei abgeschnittener Basis:** Für neue Zombies am Rand wird höchstens 1.000-mal eine Stelle gesucht;
    führt von keiner ein Weg zur Basis, kommt eben keiner.
  - **Entwickler-Tasten sind jetzt Knöpfe** im Panel: Einfg, Pos1, Bild auf/ab, Entf, Ende, F4 und F11 sind frei.
    Geblieben: F3 (Messwerte), F10 (Editor), F12 (Maus freigeben).
  - **Alles auf Englisch:** Texte im Spiel und alle Kommentare im Code (`src\`). README, Spickzettel und Erklärungen
    bleiben deutsch. Neue Datei `src\editor.rs`, die Karte kennt jetzt Geländearten statt nur Felsen.
  - 24 Tests (neu: Malen schiebt alle heraus, Bauen auf Zombies, Malen lässt Gebäude stehen, abgeschnittene Basis
    hängt nicht, Einheiten laufen um neues Gelände herum, gemalte Linie ohne Lücke).
- **2026-10-16:** **Weltgrenze, Game View / Editor Mode, Medic, Einheiten als Klötze:**
  - **Weltgrenze:** Das Spielfeld bleibt 128 × 128, die ganze Karte ist jetzt 160 × 160 – im Ring außen herum starten
    die Zombies. Einheiten kommen nicht darüber (für sie ist alles dahinter versperrt, Befehle nach draußen enden an
    der Grenze), die Kamera hört dort auf, und im Game View wird außerhalb nichts gezeichnet, auch nicht auf der Minimap.
  - **Game View / Editor Mode** oben rechts (F10): Der Editor zeigt die ganze Karte mit der Grenze als oranger Linie
    und hat alle Entwickler-Werkzeuge; der Game View hat keine.
  - **Wellen im Editor:** klein (150 Zombies aus einem Viertel einer Seite), eine ganze Seite (500), zwei zufällige
    Seiten (je 500).
  - **Bauen nur mit Abstand zu Zombies:** Bauplatz plus 4 Kacheln rundherum müssen frei von Zombies sein; der Rahmen
    an der Maus zeigt das Feld (rot = Zombie drin). Auf Einheiten kann man wieder nicht bauen – Wegschieben gibt es
    nur noch bei Berg, Wald und Wasser.
  - **Medic** (E beim Kommandozentrum): greift nie an, T auf eine eigene Einheit richtet den grünen Strahl auf sie,
    sonst von selbst auf verletzte Einheiten in der Nähe. Heilen kommt später.
  - **Einheiten sind kleine 3D-Klötze** wie die Gebäude (gleiche Zeichen-Funktion), der Medic ist weiß.
  - 29 Tests (neu: Grenze hält Einheiten drin und lässt Zombies herein, Wellen von den richtigen Seiten, kein Bau
    neben Zombies, kein Bau auf Einheiten, Medic-Strahl auf Befehl und von selbst).
- **2026-10-16:** **Formationen wie in DINO, Sicht und Nebel, Wälder und Bäume, Spawner:**
  - **Formationen statt Magic Box** (neu `src\formation.rs`): Linie, Block oder Kreis auf der Befehlskarte (X / C / V)
    wählen, dann mit der **rechten** Maustaste ziehen – die Einheiten stellen sich an der Linie auf, Soldaten vorne,
    Scharfschützen und Medics dahinter. Mit T/P/M wartend geht dasselbe mit der linken Maustaste. Gewünscht war
    Linksklick + Ziehen – das ist bei uns (und in DINO) aber der Auswahlrahmen; DINO zieht Formationen auch mit rechts.
    Die beiden Magic-Box-Einstellungen sind weg.
  - **Sicht und Nebel** (neu `src\vision.rs`): Sichtweite für jede Einheit und jedes Gebäude, unerkundet schwarz,
    erkundet abgedunkelt, Zombies nur in Sicht – auf dem Spielfeld, auf der Minimap und beim Anklicken. Der Editor
    zeigt alles.
  - **Wälder und Bäume** in der Kartenerzeugung (nicht in 14 Kacheln um die Basis), Baum auch als Pinsel im Editor.
    Niemand sieht oder schießt hindurch (Sichtlinie Kachel für Kachel, `Map::clear_sight`); wer ein Ziel hinter einem
    Baum angreifen soll, läuft erst herum.
  - **Spawner**: Zombies kommen nur noch aus Spawnern – Start-Horde, Wellen und Nachschub. Zu Beginn 3 pro Seite im
    Ring außerhalb der Grenze (benannt NE/SE/SW/NW 1–3, so wie die Seiten auf dem Bildschirm liegen). Im Editor:
    setzen, anklicken (leuchtender Rand), am Rand ziehen = Größe, innen ziehen = verschieben, Namen tippen, löschen.
  - Haken: Die Formationstasten sind X/C/V statt Z/X/C, weil macroquad Tasten nach US-Position liest (Z läge auf Y).
  - 41 Tests (neu: Formationen Linie/Block/Kreis/Soldaten vorne, Einheiten stellen sich in Linie auf, Sicht endet am
    Wald und Erkundetes bleibt, kein Schuss durch Bäume, Wälder und Bäume in der Karte, Baum versperrt nur dazwischen,
    Wellen aus Spawnern, ohne Spawner keine Zombies, Spawner setzen/vergrößern/umbenennen).
- **2026-10-16:** **Zombies sehen, Tor und Turm, diagonale Mauern:**
  - **Zombie-Sicht:** Zombies sehen Einheiten auf 10 Kacheln (Wald und Bäume nehmen die Sicht), hören sie auf 3, und
    wer angeschossen wird, stürmt 5 s lang los. Vorher bemerkten sie nur, was näher als 5 Kacheln war – Scharfschützen
    hinter einer Mauer am Wasser schossen sie ab, ohne dass sie reagierten. Gerechnet wird umgekehrt (von den wenigen
    Einheiten aus: „wer hier steht, sieht eine Einheit“), sonst wären es Tausende Sichtlinien pro Takt.
  - **Ansturm:** Wer eine Einheit bemerkt hat, folgt einer zweiten Jagdkarte (`src\hunt.rs`): um Wasser und Berge
    herum, aber eine Gebäude-Kachel kostet nur 2 Schritte – so stürmen sie die Mauer, statt die Lücke zu suchen (das
    bleibt den Jägern, DESIGN.md 8.1). Gerechnet wird sie nur, wenn ein Zombie überhaupt jemanden bemerkt hat.
  - **Editor „View“:** All / Player / Zombies – durch wessen Augen man schaut. Die Zombie-Sicht wird nur gerechnet,
    solange man sie ansieht (einmal pro Takt, pro Kachel statt pro Zombie).
  - **Bauen bleibt aktiv** bis Rechtsklick oder Esc (vorher war das Gebäude nach dem Bauen weg, außer mit Shift).
  - **Diagonale Mauern 1 breit** wie in TAB (Linie wie beim Bresenham-Algorithmus). Damit niemand durch die Ecke
    zwischen zwei schrägen Stücken schlüpft, prüfen „freie Linie“ und die Sichtlinie im Flow Field jetzt Kachel für
    Kachel ohne Eckensprung – sonst wären Zombies und Einheiten in der Ecke hängen geblieben.
  - **Mauer-Skin, der sich verbindet:** Alles Stehende ist jetzt ein „Prisma“ (beliebiger Viereck-Grundriss nach oben
    gezogen), so gehen auch schräge Mauerstücke. Allein ein Block, sonst Pfosten + Stück zu jedem Nachbarn.
  - **Tor** (G, 3 × 1, R dreht): Die Karte kennt jetzt „Tür“-Kacheln – für Einheiten offen, für Zombies zu.
  - **Turm** (T, 1 × 1, 600 HP): 4 Plätze, Rechtsklick zum Hineingehen, U zum Rauslassen, drinnen +2 Reichweite und
    Sicht und nicht treffbar. Die +2 sind meine Wahl (DESIGN.md sagte nichts dazu) – eine Zahl in `kinds.rs`.
  - Gebäude haben jetzt Breite × Tiefe statt einer Kantenlänge (das Tor ist nicht quadratisch).
  - Geändert: Der Test „Zombies zerstören ein Zelt“ läuft ohne Einheiten – sonst stürmen die Zombies jetzt auf die
    Start-Einheiten und gehen ums Zelt herum (so gewollt).
  - Gemessen: 3.500 Zombies, 1,5 ms Simulation pro Takt.
  - 46 Tests (neu: Zombies sehen den Scharfschützen und stürmen die Mauer am Wasser vorbei, ein angeschossener Zombie
    kommt zum Turm – ohne Schuss nicht, diagonale Mauern sind dicht, Tor lässt Einheiten durch und Zombies nicht,
    Turm fasst 4 und schützt sie, diagonale Baulinien).
- **2026-10-16:** **Spawner-Einstellungen, Turm-Sammelpunkt, Karten aus Rauschen, zufälliger Seed:**
  - **Spawner** (im Editor anklicken): schickt Zombies **dauernd** (Getötete kommen dort wieder), **alle X Sekunden N
    Stück** (1–120 s, 1–500) oder **gar nicht** von selbst. Neuer Befehl `SetSpawnMode` – auch Editor-Werkzeuge sind
    Befehle, damit es deterministisch bleibt.
  - **Turm-Sammelpunkt:** R oder Rechtsklick bei ausgewähltem Turm; „Unload“ lässt die Einheiten auf dieser Seite heraus
    und schickt sie hin (wie beim Kommandozentrum, auch auf eine Einheit: folgen).
  - **Formation nur beim Ziehen:** Ein Klick (rechte Maus, T/P/M, Minimap) schickt alle auf den Punkt; erst Ziehen
    stellt sie in Formation auf. Die Vorschau erscheint auch erst beim Ziehen.
  - **Zombies verblassen** in 0,8 s, wenn sie aus der Sicht verschwinden. Nur Darstellung – darum darf dafür eine
    `HashMap` benutzt werden (in der Simulation verboten, DESIGN.md 12.8).
  - **Sicht:** Einheiten +2 (Soldat 10, Scharfschütze 13, Medic 10), Zombies +3 (13). Haken: Zombies sehen jetzt weiter,
    als jede Einheit schießt – der Alarm beim Angeschossen-werden greift fast nur noch, wenn ein Schütze aus der Sicht
    verschwindet. Der Ansturm sucht dafür jetzt bis 36 Schritte weit.
  - **Zufälliger Seed** bei jedem Start und bei „New map“ (aus dem Zufallsgenerator des Betriebssystems, ohne neue
    Bibliothek). `--seed 12345` startet eine bestimmte Karte – zum Wiederholen von Fehlern und für die Prüfsumme.
  - **Karten aus Rauschen** (neu `src\noise.rs`, DESIGN.md 9.1): Höhe → Seen und Gebirge, Feuchte → Wälder, bis zu zwei
    Flüsse, Bäume mehr in Waldnähe. Die Mengen werden pro Karte ausgewürfelt. Ist ein Spawner abgeschnitten, wird eine
    Furt zur Basis freigeräumt.
  - 50 Tests (neu: Rauschen wiederholbar und glatt, jede Karte hat Wasser/Berge/Wald/Bäume und nichts an der Basis,
    alarmierter Zombie stürmt zu unsichtbaren Einheiten, Spawner dauernd/periodisch/aus, jeder Spawner erreicht die
    Basis bei 15 Seeds, Turm lässt Richtung Sammelpunkt heraus). Der alte Alarm-Test („Turm schießt weiter als Zombies
    sehen“) ging mit Sicht 13 nicht mehr und ist ersetzt.
- **2026-10-16:** **Pathing neu, Pause zeigt Befehle, Zombie-Pinsel, Zombies als Klötze:**
  - **Pathing:** Einheiten liefen erst schräg, bis eine Achse passte, dann gerade. Ursache: Das Flow Field rechnete
    Entfernungen mit 4 Nachbarn (alle Treppen gleich lang) und hatte Pfeile in nur 8 Richtungen. Jetzt ein
    **Sichtlinien-Flow-Field** nach dem Prinzip von **Theta\***: Entfernungen mit 8 Nachbarn (gerade 10, schräg 14),
    und statt Pfeilen merkt sich jede Kachel einen **Ankerpunkt** – den weitesten Punkt auf ihrem Weg, den sie noch
    gerade sieht. Ergebnis: gerade in jedem Winkel, straff um Ecken. Kostet 2–4 ms pro Feld (pro Befehl, nicht pro Takt).
    Gilt auch für den Weg der Horde zur Basis. Die Jagd- und Sturmkarten der Zombies (`hunt.rs`) laufen noch in 8
    Richtungen – auf freier Fläche gehen sie ohnehin gerade auf ihr Ziel zu.
  - **Pause zeigt Befehle:** In der Pause werden Befehle sofort übernommen (`World::apply_orders`) – Linien, Bauschlange,
    Sammelpunkte, Gebäude sind zu sehen –, aber erst mit dem nächsten Takt ausgeführt. Ein Test zeigt: genau dasselbe,
    als hätte man sie mit dem nächsten Takt gegeben.
  - **Editor baut überall** (außer auf andere Gebäude), das Gelände darunter wird Gras.
  - **Zombie-Pinsel** (pro Takt 1–20 auf der Pinselfläche) und **Drop** (1–1.000 auf einmal), dazu Art (gemischt /
    Walker / Jäger) und Verhalten (stürmen / **herumwandern**, bis sie eine Einheit bemerken). Zombies haben dafür jetzt
    eine echte Art (`Breed`) statt „jede 4. Nummer“. Das alte „+1 / +100 zombies“ ist weg.
  - **Editor-Abschnitte auf- und zuklappbar**, damit das Panel auch bei 720 px Höhe passt.
  - **Zombies sind kleine 3D-Klötze** wie die Einheiten (verblassen weiter). Die Punkt-Textur ist weg.
  - 54 Tests (neu: gerade in jedem Winkel und straff um eine Mauer, Pause = nächster Takt, Editor baut überall,
    Pinsel-Art und Wanderer bleiben – bis sie einen Soldaten sehen).
- **2026-10-16:** **Gruppen laufen durch Wegpunkte, Zombies quetschen sich nicht mehr, ganze Befehlskette sichtbar:**
  - **Gruppe auf einen Punkt mit Shift-Kette:** Nur einer kam jeweils weiter. Ursache: Angekommen war nur, wer genau
    auf dem Punkt stand oder eine *stillstehende* Einheit berührte – der Erste lief aber sofort zum nächsten Punkt
    weiter, die anderen drückten sich ewig um den Punkt. Im Test blieben 8 von 12 Einheiten für immer am ersten Punkt
    stehen. Jetzt gilt auch als angekommen, wer am **Rand der Menge** steht: Ist der Kreis um den Punkt, der näher
    liegt als man selbst, schon voll, kommt man nicht näher. Der Rand wächst mit der Gruppe (wie in StarCraft).
    Formationen waren nicht betroffen – dort hat jeder seinen eigenen Platz.
  - **Ganze Befehlskette immer sichtbar** (neue Einstellung, an). Bisher wie in SC2 nur mit Shift; abgeschaltet
    ist es wieder so.
  - **Zombies quetschen sich nicht mehr ineinander:** Das Schubsen war nur eine sanfte Kraft, die gegen das Laufen
    verlor – wer hinten drückte, rutschte in den Vorderen hinein (im Test über 100 Paare mehr als halb ineinander,
    107 Zombies auf 1,7 Kacheln vor einer Lücke). Jetzt werden überlappende Zombies zweimal pro Takt **direkt
    auseinandergesetzt**. Die Horde staut sich vor Engstellen und drängt zur Seite wie Wasser. Kostet bei 10.000
    Zombies in dichten Haufen etwa 3 ms mehr pro Takt (3,6 → 6,7 ms, Budget 50 ms).
  - 56 Tests (neu: eine Gruppe arbeitet eine Kette von 3 Punkten ganz ab, Zombies stauen sich vor einer Lücke statt
    ineinanderzurutschen).
- **2026-10-16:** **Tor auf Mauern, Abstand zwischen Türmen, Bauregeln in einer Datei, kein WASD bei Gebäuden:**
  - **Neue Datei `src\placement.rs`** mit allen Bauregeln: `can_place` (Spiel), `can_place_anywhere` (Editor), die
    zombiefreie Zone (`BUILD_CLEARANCE`), die Abstandsregeln (`gap`), was worauf gebaut werden darf (`replaces`), der
    Preis (`build_cost`) und die gezogenen Reihen (vorher in `sim.rs` und `ui.rs` verteilt). Eine neue Regel ist dort
    eine Zeile, z. B. `(Tent, Tent) => 1,`.
  - **Tor auf Mauern:** Das Tor darf auf Kacheln mit Mauern stehen (ganz oder teilweise); die Mauern darunter werden
    abgerissen. Darum war es im Bild rot: bisher musste jede Kachel leer sein.
  - **Preise** (erste Schätzung, `kinds.rs`): Zelt 50, Mauer 10, Tor 40, Turm 30. Noch wird nichts bezahlt – es gibt
    noch keine Wirtschaft –, der Preis steht nur an der Maus. Jede ersetzte Mauer zieht **50 %** ihres Preises ab
    (`REPLACE_REFUND`): Tor auf 3 Mauern kostet 25, aber 3 Mauern + Tor = 55 > 40. Ein Test prüft, dass das so bleibt.
  - **Türme halten 1 Kachel Abstand** zueinander, auch über Eck. Gezogene Turmreihen lassen die Lücke von selbst.
  - **Gebäude ausgewählt → WASD bewegt die Kamera nicht** (Pfeiltasten, Rand und Mausrad gehen weiter).
  - 60 Tests (neu: Tor ersetzt Mauern, Türme halten Abstand, Tor auf Mauern lohnt sich nicht, Abstand in Kacheln).
- **2026-10-16:** **Fünf Kartengeneratoren mit eigenen Werten, Startbildschirm, Editor mit Tabs wie TAB, Karten speichern:**
  - **Was an der alten Karte kaputt war** (6 Seeds als Bild angeschaut): ein eckiges Loch um die Basis, an dem Seen und
    Flüsse wie abgeschnitten endeten; Flüsse als dünne Rausch-Linie, die in Stücke zerfiel, Schleifen machte und mitten
    im Feld aufhörte; formlose Kleckse ohne Engstellen. Statt eines Generators gibt es jetzt fünf (neu `src\mapgen\`):
    **Wild** (das Rauschen repariert: verbogene Formen, Bergrücken, echte Flüsse von Rand zu Rand um die Basis herum,
    weiches Ausblenden vor der Basis), **Regions** (Regionen mit Wänden und Pässen wie TAB), **Rings** (Festungsringe
    mit versetzten Öffnungen), **Canyons** (Zellulärer Automat) und **Blank**. Jeder hat eigene Schieberegler.
  - **Jede offene Stelle erreicht die Basis** (`connect_pockets`): ersetzt die alte gerade Furt – jetzt der kürzeste
    Weg durchs Hindernis, für jede abgeschnittene Stelle, nicht nur für Spawner.
  - Generatoren ohne `sin`/`cos`/`powf` (nur `+ − × ÷` und `sqrt`), damit die Karte im Koop auf jedem PC bit-genau
    gleich ist; `BTreeMap` statt `HashMap`, weil ein `HashMap` bei jedem Start in anderer Reihenfolge läuft.
  - **Karte 256 × 256** wie in TAB (wählbar 96–256). Gemessen: Karte erzeugen 2–30 ms, ganze Welt 57 ms, 5.000 Zombies
    3,9 ms pro Takt. Die Minimap ist jetzt ein Bild statt einer Raute pro Kachel (`src\picture.rs`) – sonst wären es auf
    256er-Karten bis zu 80.000 Rauten pro Bild gewesen.
  - **Startbildschirm** (`src\home.rs`, DESIGN.md 10.1 erste Fassung): New game, Map editor, Custom maps, Settings, Quit;
    Karten-Einstellung mit Seed, Reglern, Hilfetext und Vorschau. Esc-Menü: „Main menu“.
  - **Editor mit Tabs** wie TAB: Map / Terrain / Entities / Waves. Neu: Gebäude ohne Regeln setzen, größere Pinsel,
    Panel scrollt mit dem Mausrad, **Zeitleiste** für Wellen (Tag + Stunde, 1 Tag = 3 min, DESIGN.md 4.1) mit **Uhr**
    oben, **Save map** als JSON (`src\mapfile.rs`), unter „Custom maps“ spielen oder bearbeiten.
  - Vom eingefügten TAB-Editor-Text nicht umgesetzt: Story/Script-Tab und `ApplyRules()` (wie gewünscht), Übersetzungen
    (`Strings.csv`), Prüfsummen-Datei, Themes (es gibt nur eine Grafik), Sieg/Niederlage (gibt es im Spiel noch nicht).
  - 67 Tests (neu: jeder Generator hält die Basis frei und verbindet alles, wiederholbar und hört auf seine Werte,
    Kartengröße und Zufallswerte, abgeschlossener Raum bekommt einen Weg, Karte speichern und laden, kaputte Datei,
    Welle der Zeitleiste kommt pünktlich). Der Stau-Test nimmt 250 statt 200 Zombies – mit 200 lag er bei 1,9997
    Kacheln genau an der Grenze.
- **2026-10-16:** **Vorschlag „ECS + Bitmasken-Raster“ geprüft** (eingefügter Text, DESIGN.md 12.2): keine
  ECS-Bibliothek – es gibt kein Tempo-Problem, und Bevys paralleler Ablauf und `delta_time` vertragen sich nicht mit
  Lockstep. Übernommen wurden zwei Ideen:
  - **`sim.rs` aufgeteilt** in `src\sim\` – eine Datei pro Teil eines Takts (Befehle, Spawner, Zombies, Einheiten,
    Befehlsketten, Kampf, Gebäude, Gelände, Tests). Nur verschoben, keine Logik geändert: dieselbe Prüfsumme wie vorher.
    `World::step` zeigt jetzt bei jedem Schritt, in welcher Datei er steht.
  - **Bitmaske pro Kachel** in `map.rs`: statt `occupied` und `door` (je eine Liste `bool`) ein Byte pro Kachel mit
    `SOLID`, `BLOCKS_SIGHT`, `BUILT`, `DOOR`, `OUTSIDE` – eine Abfrage statt drei Listen. Gleiche Prüfsumme, gemessen
    gleich schnell (3,7 ms pro Takt mit 5.000 Zombies, vorher wie nachher). Der Gewinn ist Übersicht und Platz für
    weitere Bits (Stromnetz).
  - `SPICKZETTEL.md` fehlte (war nie in git) – neu angelegt, beginnt mit Modulen und Sichtbarkeit.
  - 68 Tests (neu: die Bits einer Kachel folgen Gelände und Gebäuden).
- **2026-10-16:** **Floats im Koop geklärt** (DESIGN.md 12.8, Fragen an Gemini geprüft): Die Simulation bleibt bei
  `f32`. `+ - * /` und `sqrt` rechnen auf jedem PC bitgleich, `sin`/`cos`/`atan2` aber nicht – die kommen unter Windows
  aus `ucrtbase.dll`, und die ändert sich mit Windows-Updates und der CPU. Deshalb:
  - **`libm`** neu in `Cargo.toml`: die drei Stellen mit Winkeln (Kreisformation in `formation.rs`, `PlaceZombies` in
    `sim/orders.rs`) rechnen jetzt mit `libm::sinf`, `cosf`, `atan2f` – reines Rust, überall gleich.
  - Die Regel steht oben in `sim/mod.rs` und in DESIGN.md 12.8 (das ❓ dort ist aufgelöst).
  - **Test mit festgehaltener Prüfsumme:** `scripted_game` spielt 600 Takte (jetzt auch mit Kreisformation und
    `PlaceZombies`), die Prüfsumme steht als Zahl im Test. Läuft er auf einem anderen PC durch, rechnet der bitgleich.
    Debug- und Release-Build kommen schon auf dieselbe Zahl. Wer die Simulation absichtlich ändert, trägt die neue Zahl
    aus der Fehlermeldung ein.
  - 69 Tests.
- **2026-10-17:** **Walker und Runner, Infektion, Gold:**
  - **Zwei Zombie-Typen:** **Walker** (dunkelgrau, 0,7 Kacheln/s, 15 HP, 4 Schaden) – neu, der schwächste und
    langsamste. Die bisherigen Zombies heißen jetzt **Runner** (hellgrau) und sind schneller geworden: 2,0 statt
    1,2 Kacheln/s (Soldaten laufen 3,0, Wegrennen geht also noch). Werte in `Breed::speed/hp/damage` (`sim/zombies.rs`).
  - **Walker liegen von Anfang an auf der Karte verstreut** und wandern, bis sie eine Einheit bemerken: wenige nahe
    am Kommandozentrum (keiner näher als 16 Kacheln), die meisten am Rand. Neuer Regler „Walkers“ in der
    Karteneinstellung (pro 1.000 Kacheln, Standard 20 ≈ 1.300 auf 256er-Karten). Die alte Start-Horde (500 aus den
    Spawnern) ist weg. **Alles aus Spawnern sind Runner** – Wellen und der Nachschub, auch für getötete Walker.
  - **Kein Jäger mehr:** Statt jeden 4. Zombie zum Jäger zu machen, suchen **der angeschossene Zombie und die 3
    nächsten um ihn herum** (bis 3 Kacheln) 10 s lang die Lücke zu deinen Einheiten. Der Rest haut weiter auf die
    Mauer. Ist der Getroffene schon auf der Suche, holt er keine weiteren dazu – sonst schickte ein Soldat, der in die
    Menge schießt, bald die ganze Horde zur Lücke. Gerechnet wird der Weg durch Lücken nur, solange einer sucht.
  - **Tote Einheiten** bleiben liegen (flach, dunkel) und **stehen nach 2–5 s als Walker auf**.
  - **Infektion wie in TAB:** Zelte haben einen **Schild** (200, blauer Balken über dem Lebensbalken) und 150 HP.
    Zombies schlagen zuerst auf den Schild. Bricht er, ist das Zelt **infiziert** (wird kränklich grün) und seine
    **6 Kolonisten kommen als Zombies heraus: 2–4 Runner, der Rest Walker**. Erst danach verliert es HP.
    Mauern, Tore und Türme haben keine Kolonisten und damit keinen Schild.
  - **Gold:** Jeder Kolonist zahlt 2 Gold pro Spielstunde (7,5 s), ein Zelt also 12. Oben neben der Uhr:
    „Gold 120 (+12/h)“. Infizierte Zelte zahlen nichts. Ausgegeben wird noch nichts.
  - Haken: Die Preise (Zelt 50, Mauer 10 …) passen nicht zu 12 Gold alle 7,5 s – ein Zelt hätte sich nach 30 s
    bezahlt gemacht. Bevor Bauen Gold kostet, müssen Preise und Unterhalt hochskaliert werden (DESIGN.md 5.3).
  - Alte Kartendateien laden weiter: „Hunter“ wird zum Runner, ein alter „Walker“ ist jetzt der neue, langsame.
  - Gemessen: 256er-Karte mit 1.310 Walkern + 3.000 Runnern: 2,2 ms pro Takt, Welt erzeugen 51 ms.
  - 74 Tests (neu: Walker langsamer als Runner und Spawner schicken Runner, Walker verstreut und meist am Rand, tote
    Einheit steht nach 2–5 s als Walker auf, Zelt-Schild bricht → genau 6 Zombies mit 2–4 Runnern, Zelt zahlt
    12 Gold pro Stunde und infiziert nichts mehr). Der Jäger-Test prüft jetzt: Mit Schuss finden ein paar die Lücke,
    ohne Schuss sucht keiner. Die festgehaltene Prüfsumme ist neu (Simulation absichtlich geändert).
- **2026-10-17:** **Kein Respawn mehr, konstanter Spawn nach Zeitplan, alle Werte in `data\`:**
  - **Getötete Zombies bleiben tot.** Das Wiederbeleben war ein Überbleibsel aus dem Horden-Stresstest (damit immer
    gleich viele da waren) – dann hätte Freiräumen zum Ausbreiten keinen Sinn. Weg sind: Schalter „Respawn“ im Editor,
    `Order::SetRespawn`, das Feld in der Kartendatei (alte Dateien laden trotzdem) und dass Zombies an der Basis
    „sterben“ und neu kommen.
  - **Konstanter Spawn** (`data\spawning.json`) aus den Spawnern auf „Constant“: Tag 1 nichts, ab Tag 2 einzelne
    Walker alle 30 s, bis Ende Tag 3 gleitend mehr (1–3 alle 10 s), ab Tag 4 Gruppen aus 4–5 Runnern alle 60 s, keine
    Walker mehr, bis Tag 8 gleitend 8–10 alle 40 s, danach +1 Zombie pro Gruppe und Tag. Jede Gruppe kommt aus einem
    zufälligen Spawner. F3 zeigt, was gerade kommt.
  - **Ein Ordner für alle Werte: `data\`** (wie in DESIGN.md 12.4 geplant) – `units.json`, `buildings.json`,
    `zombies.json`, `colony.json` (Startgold, Gold pro Kolonist, Start-Einheiten), `spawning.json`. Was jeder Wert
    heißt, steht in `data\README.md`. Zeiten in Sekunden. Neu `src\data.rs` lädt sie: eingebaut in die Exe, und vor
    jeder neuen Runde (Start, Restart, New map) frisch aus `data\`, falls der Ordner da ist – Wert ändern, Restart,
    fertig. Eine kaputte Datei zeigt eine rote Zeile, die alten Werte bleiben.
  - Im Code geblieben: Tageslänge, Größe der Figuren, wie die Menge sich schiebt, die Abstands-Tabelle der Bauregeln,
    die Regler der Kartengeneratoren (die stehen in den Einstellungen).
  - 78 Tests (neu: konstanter Spawn folgt den Abschnitten, der erste Walker kommt genau zu Beginn von Tag 2; die
    eingebauten Daten sind vollständig; `data\` wird gefunden und gelesen; eine kaputte Datei nennt ihren Namen).
    Der Spawner-Test prüft jetzt, dass Getötete tot bleiben. Neue Prüfsumme (im Testspiel kamen vorher Getötete wieder).
- **2026-10-17:** **DESIGN.md gekürzt** (1166 → 859 Zeilen, ≈ 89 → 51 KB): nur noch der aktuelle und der geplante
  Stand, kein Verlauf mehr (der steht hier unter „Stand“ und in git).
  - Weg sind Datumsangaben, „ersetzt/vorher war …“, verworfene Varianten (Magic Box, Jäger-Zombies, Sichtweite 10)
    und der Sprachvergleich Rust/C++/C#/C. Lange Sätze sind jetzt Stichpunkte.
  - Genaue Prototyp-Zahlen (HP, Schaden, Tempo) stehen nicht mehr doppelt drin – nur noch der Verweis auf `data\`.
  - Was schon gebaut ist, steht pro Abschnitt kurz unter *Prototype:*. Veraltetes korrigiert (Einstellungen ohne
    `magic_box_*`, Medic gibt es schon, Angriffsbefehl auf T).
  - Offene Fragen (14) enthalten nur noch die offenen (14, 17, 18, 21, 22, alte Nummern behalten); die Entscheidungen
    stehen in ihren Abschnitten. Der Fahrplan (13) hat eine Spalte „Status“.
  - **Abschnittsnummern unverändert**, weil der Code darauf verweist (z. B. `DESIGN.md 12.8`).
- **2026-10-17:** **Ranger, Fat-Zombie, Drehen und Zielen wie in StarCraft, Werte aus TAB:**
  - **Ranger** (grün): schnell (4 Kacheln/s), schwach (60 HP), Reichweite 6, Schaden 10 – wie TABs Ranger. Im
    Kommandozentrum auf **A** (4 s), im Editor unter Units. Zu Beginn 4 dabei (`start_rangers` in `colony.json`).
  - **Jede Einheit dreht und zielt** (`combat.rs`): erst zum Ziel drehen (`turn_around_seconds` = halbe Drehung), dann
    nachladen abwarten (`cooldown_seconds`), dann zielen (`aim_seconds`: Ranger 0,4 s, Scharfschütze 0,25, Soldat
    0,1), dann der Schuss. Neues Ziel → Zielen von vorn; darum behält eine Einheit ihr Ziel, solange es lebt und in
    Reichweite ist (wie in SC2), statt jeden Takt zum nächstbesten zu springen. Beim Laufen dreht sie sich in
    Laufrichtung, ohne darauf zu warten. Gezeichnet: ein dunkles Stück (Gewehr, Bogen) zeigt die Blickrichtung.
  - **Werte aus TAB** (`data\units.json`, `data\zombies.json`): HP, Tempo, Reichweite, Sicht, Feuerrate der drei
    Schützen; Walker 35 HP / 0,4 Kacheln/s / 6 Schaden, Runner 1,75 Kacheln/s / 10 Schaden. Abweichungen, damit deine
    Schusszahlen stimmen (Ranger 4 / Soldat 2 / Scharfschütze 1 für Walker, 6 / 3 / 1 für Runner): Soldat 20 statt
    15 Schaden, Runner 50 statt 45 HP. Ein Test prüft die Schusszahlen, falls jemand Werte ändert.
  - **Fat** (TABs „Chubby“): 600 HP, 40 Schaden, 1,75 Kacheln/s, **größer** (Radius 0,45 statt 0,25 – er drängt
    die anderen weiter weg und erreicht mehr). Erstmal nur im Editor (Pinsel/Drop), nicht im konstanten Spawn.
  - **Zombies haben jetzt Panzerung** wie die Einheiten (Walker 0, Runner 1, Fat 2) und jeder eine eigene Größe
    (`radius` in `zombies.json`; die feste Konstante `ZOMBIE_RADIUS` ist weg).
  - Haken: Einheiten sehen mit den TAB-Werten weniger weit (Soldat 6, Ranger 8, Scharfschütze 9) als Zombies (13) –
    Zombies bemerken dich also, bevor du sie siehst. Die Zombie-Sicht ist (noch) für alle gleich; in TAB liegt sie
    bei 5–8, dort lockt vor allem der Lärm der Schüsse.
  - „Ich kann auf Wasser bauen“: Im **Spiel** geht das nicht (geprüft: auf keiner der 1.502 Wasserkacheln einer Karte
    passt ein Zelt). Im **Editor Mode** geht es absichtlich – das Gebäude kommt ohne Regeln hin und das Wasser darunter
    wird Gras (dafür gibt es einen Test). Nicht geändert.
  - 81 Tests (neu: Schusszahlen, Ranger dreht sich um und zielt vor dem ersten Pfeil und schießt dann genau alle
    Zielen + Nachladen, Fat-Zombies halten mehr Abstand). Angepasst: Turm-Test mit 10 statt 20 Zombies (die stärkeren
    Runner rissen den Turm sonst ein), Medic und Scharfschütze brauchen erst ein paar Takte zum Drehen. Neue Prüfsumme.
- **2026-10-17:** **Zombie-Sicht wie TAB, Lärm, WASD, Editor ohne Gras:**
  - **Zombie-Sicht pro Art** (`sight` in `zombies.json`): Walker 5, Runner 6, Fat 5 statt 13 für alle. Jede Art hat ihre
    eigene „sieht hier jemand eine Einheit?“-Karte (`zombie_sight`, eine pro Art) – gerechnet von den wenigen Einheiten
    aus, nicht von den tausenden Zombies.
  - **Lärm wie in TAB** (`noise` in `units.json`): Jeder Schuss auf einen Zombie weckt alle Zombies im Umkreis (Ranger 1,
    Soldat 3, Scharfschütze 10 Kacheln, Medic 0). Sie stürmen 20 s (`noise_seconds`) auf deine Einheiten los, wie ein
    angeschossener Zombie. Wald und Mauern halten Lärm nicht auf. Das ist nur der lokale Teil von DESIGN.md 8.3 – der
    Lärm, der sich wie Factorios Verschmutzung über die Karte ausbreitet, kommt mit Schritt 9 des Fahrplans.
  - **WASD bewegt die Kamera nur noch, wenn nichts ausgewählt ist** – vorher ging das mit Einheiten für die Tasten, die
    keine Befehle sind (W, A, D). Pfeiltasten, Rand und Mausrad gehen immer.
  - **Editor: kein Gras mehr unter Gebäuden.** Das Gelände bleibt (Wasser bleibt Wasser). Damit ein Tor auf Wasser
    keine Brücke wird, öffnet seine Tür nur auf Boden (`Map::blocks_unit`).
  - Tests: 83 (neu: Runner sieht 5,2 Kacheln weit, Walker nicht 5,8; Scharfschützen-Schuss weckt einen Zombie 9 Kacheln
    hinter ihm, ein Pfeil nicht). Angepasst: Der Mauer-Test („Zombies stürmen die Mauer am Wasser vorbei“) klappt jetzt
    über den Lärm des Scharfschützen statt über die Sicht; Wanderer bemerken nur noch den Soldaten, den sie auch sehen.
    Neue Prüfsumme.
  - **Offen:** Soldaten-Schaden steht jetzt auf 15 (TAB). Damit braucht der Soldat 3 statt 2 Schuss für einen Walker –
    der Test `shots_to_kill_walkers_and_runners` schlägt deshalb fehl. Beides zusammen (Ranger 4 Schuss, Soldat 2) geht
    mit 15 Schaden nicht: Der Walker bräuchte 31–40 HP für den Ranger, aber höchstens 30 für den Soldaten.
- **2026-10-17:** **Gebäude in der Auswahl, Baumenüs, Ein-Balken-Leben, Erz, bessere Karten, kein Ruckeln:**
  - **Soldat bleibt bei 15 Schaden** (TAB): 3 Schuss für einen Walker, 4 für einen Runner. Der Test hält das fest.
  - **Ruckeln bei Befehlen behoben:** Jeder Lauf-/Angriffsbefehl rechnete ein Flow Field über die ganze Karte (288 × 288)
    – 95 ms im Debug-Build, 8 ms im Release. Jetzt: die Sichtlinien-„Anker“ werden erst gerechnet, wenn jemand auf der
    Kachel steht (statt für alle 83.000), der kürzeste Weg mit einer Eimer-Warteschlange statt eines Heaps, Patrouille
    teilt sich ein Feld für den Rückweg (vorher eins pro Einheit), und der Debug-Build ist leicht optimiert
    (`Cargo.toml`). Ergebnis: 5,7 ms im Debug-Build, 2,2 ms im Release. Nebenbei laufen alle Tests in 2 statt 14 s.
  - **Gebäude in Auswahl und Kontrollgruppen** wie in StarCraft: Shift-Klick nimmt ein Gebäude zur Auswahl dazu, der
    Rahmen nimmt Gebäude, wenn keine Einheit drin ist, Strg+Zahl speichert auch Gebäude. **Tab** wechselt auch zu
    Gebäuden – die Befehlskarte zeigt den aktiven Typ. Rechtsklick: Einheiten laufen, Gebäude setzen ihren Sammelpunkt.
  - **Kommandozentrum im StarCraft-Raster:** Q Soldat, W Ranger, E Energie ▸, R Scharfschütze, T Medic, A Spezial ▸,
    S Zelt, D Verteidigung ▸ (Q Mauer, W Tor, E Turm), G Sammelpunkt; Esc zurück. Energie und Spezial sind noch leer.
    Der Turm genauso: Q raus, G Sammelpunkt. Einheiten behalten ihre Tasten (M, S, H, P, T).
  - **Leben + Schild = ein Balken** (über dem Gebäude, im Panel, auf den Symbolen): HP in Farbe, Schild graublau dahinter.
    Der Schild steht in `buildings.json` als `shield_percent` (Prozent der HP; Zelt 133 % = 199 statt 200), im Panel in %.
  - **Herumstreifende Zombies** (`roaming` in `zombies.json`): näher an der Basis (ab 10 statt 16 Kacheln), gleichmäßiger
    verteilt, deutlich mehr (Regler heißt jetzt „Roaming zombies“, Standard 60 statt 20 pro 1.000 Kacheln ≈ 3.900),
    ab 35 % des Wegs zum Rand auch Runner, ab 80 % auch Fat.
  - **Wege für die Einheiten repariert:** Die Generatoren verbanden jede offene Stelle mit der Basis – aber oft nur
    **durch den Ring hinter der Weltgrenze**, wo deine Einheiten nicht hinkönnen (Tausende Kacheln pro Karte, darum
    brauchte es den Erase-Pinsel). Jetzt zwei Durchgänge: erst die ganze Karte (Spawner), dann innerhalb der Grenze. Und
    der Weg schneidet so wenig Hindernis wie möglich durch („0-1-BFS“) statt einfach den kürzesten.
  - **Erz** (`mapgen/ores.rs`, wie Factorios „Spot Noise“): Eisenerz, Kohle, Stein als Felder auf Gras. Neben der Basis
    immer eins von jedem, weiter draußen größer und reicher. Regler: Anzahl, Größe, Reichtum. Im Editor malbar, wird mit
    der Karte gespeichert (alte Karten laden ohne Erz).
  - **Generatoren dichter und natürlicher:** Wild mit mehr Wald (30 %) und Fels (13 %) bis nah an die Basis; Regionen und
    Ringe mit dickeren Wänden aus Fels, Wald und etwas Wasser im Wechsel, lange Wände mit mehr Pässen, kleine Haine auch
    in Wiesen; Canyons mit großen Felsmassen statt gleichmäßigem Brei; überall verschwinden winzige Krümel.
  - Neues Werkzeug für mich (und dich): `cargo test dump_previews -- --ignored` mit `DUMP_DIR` schreibt von jedem
    Generator ein Bild als PNG – so habe ich die Karten angeschaut.
  - 85 Tests (neu: Einheiten und Zombies erreichen die Basis von überall – 30 Karten mit Standard- und Zufallswerten;
    Erz liegt neben der Basis, nur auf Gras, draußen reicher, der Regler wirkt; herumstreifende Zombies nach Entfernung).
    Angepasst: Alarm-Test (12 statt 16 Kacheln, Zombies sehen nur noch 6), Speichern prüft das Erz. Neue Prüfsumme.
