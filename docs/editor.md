# Der Editor wie SC2, mit allem von TAB

**Ziel:** Der Editor bekommt den Aufbau von StarCraft 2s Editor – alle Funktionen aus TABs Editor bleiben drin. Vorlage
ist der Entwurf (6 Tafeln, vom Nutzer abgenommen): https://claude.ai/artifact/YChSdLLzGWb4TFfNd5wrn9

Jeder Schritt ist ein Startpunkt für eine neue Unterhaltung („Lies docs/editor.md, Schritt N“). Was dahinter steht,
plant DESIGN.md 12.9 (Englisch); die Trigger-Sprache und 28YL stehen in `docs\trigger-und-28yl.md`.

## Für jeden Schritt

- README → „Regeln für Claude“: am Ende eine kurze Zusammenfassung (Code erklären nur auf Wunsch).
- `DESIGN.md` 12.9, die README (Tabelle „Editor“ unter „Prototyp ausprobieren“) und `data\README.md` an Ort und Stelle
  nachziehen, keine datierten Einträge.
- **Nur zeigen, was es gibt:** Ein Feld aus dem Entwurf, dessen Logik noch fehlt (Regionen, Betreten-Trigger,
  Bedingungen als Ausdruck …), kommt mit dem Schritt, der die Logik baut – nicht als toter Knopf.
- Jede Änderung geht als Order durch die Simulation (Undo/Redo, Determinismus). Neues in der Karten-Datei: `VERSION`
  in `src\mapfile.rs` hoch, alte Dateien bleiben lesbar, mit Test.
- Prüfen: `cargo test --release`, `cargo clippy --release --all-targets`, den Editor starten und ansehen
  (Skill `run-last-bastion`).

## Aufbau (alle Tafeln)

- **Oben:** Menüleiste (File, Edit, View, Map, Test, Help; Dateiname mit ● bei Ungespeichertem), darunter die
  **Modulleiste** wie SC2: Terrain · Objects · Zombies · Regions | Triggers · Events · Map · (Data – später);
  rechts Undo, Redo, Save und **▶ Test (F10)**.
- **Links** das Werkzeug des Moduls, **rechts** Eigenschaften und Ansicht, **Mitte** die Karte mit schwebenden
  Leisten (Sicht All/Player/Zombies, Gitter, Nebel, Zoom; unten links: was die Maus gerade tut).
- **Unten** eine Statuszeile: Kachel unter der Maus, Gelände, Auswahl, was gesetzt wird, Undo-Stand, Tag und Uhrzeit.
- Dunkles Design, Abzeichen zeigen die Herkunft: **TAB** (gelb), **SC2** (blau), **neu** (grün).

## Schritte

| # | Was | Status |
|---|---|---|
| 1 | **Rahmen:** Menü- und Modulleiste, feste Panels links/rechts statt des schwebenden Fensters, Statuszeile, schwebende Leisten über der Karte, F10 = Test. Die heutigen Tabs ziehen um: Map → **Map**, Terrain → **Terrain**, Entities → **Objects** und **Zombies** (Pinsel, Drop, Riese, Spawner, Dorfhäuser), Events → **Events**, Story → **Triggers**. Die Karte nutzt nur die Fläche zwischen den Panels (Kamera, Klicks, Minimap) | **fertig** – Map, Events und Triggers zeigen ihre Felder vorerst links neben der Karte (Vollbild-Seiten kommen mit 5–7) |
| 2 | **Objects** (Tafel 1): links alle Arten **mit Bild** (Porträts/Gebäudebilder aus `assets\`), in Gruppen, mit Suche und Filtern (Colony, Defense, Units, Neutral, Decor, Loot); „Placing“: Besitzer, Drehen (R), Shift = weiter setzen, „frei“ (ohne Kosten, ohne Arbeiter). Unten die Liste von allem Platzierten (TABs Entity-Liste: Spalten Art, Name-ID, Kachel, Besitzer, Leben, Skripte; Filter, Suche; Klick wählt, Doppelklick springt hin). Rechts die Eigenschaften, **Mehrfachauswahl ändert alle**. Dazu alle neuen Arten (TAB-Gebäude, Wunder, Deko, Raben) – sie sind schon im Spiel | **fertig** (`src\editor\objects.rs`, `thumbs.rs`) – ohne „frei“ (im Editor kostet nie etwas) und ohne Raben (sie leben nur im Bild, nicht in der Karte: `critters.rs`); Skripte in der Liste = das Beitritts-Skript, mehr kommt mit 5 |
| 3 | **Terrain** (Tafel 2): Werkzeuge Pinsel, Linie, Rechteck, Füllen, Rauschen, Glätten, Bereich kopieren/einfügen, Generator nur in der Auswahl (Wald, See, Berge, Erzfeld, Village of Doom – mit Seed). Ebenen wie TABs Painter: Boden, Wald, Felsen, Erz, je Bild A/B/C oder zufällig, Dichte; Pinsel rund/eckig, „Gebäude und Wege frei lassen“. Landschaft umschalten (BR … VO). Rechts Minimap, SC2s **Ebenen-Schalter** (Boden, Wald/Felsen/Erz, Gebäude und Einheiten, Zombies, Regionen, Gitter, Nebel, Wege der Zombies, Lärm als Wärmebild) und „Unter der Maus“ (Kachel, Boden, was drauf ist, begehbar) | **fertig** (`src\editor\terrain.rs`, `src\sim\landscaping.rs`) – **Dichte** statt eines eigenen Rausch-Pinsels; Bild A/B/C = die Bilder der Landschaft in drei Gruppen, für Wald, Felsen, Erz (der Boden hat nur eine Textur); „Wege“ = die Gleise (Straßen gibt es nicht); Erzfeld ohne Öl; die Ebene „Regionen“ kommt mit Schritt 4; Lärm entsteht nur im Test |
| 4 | **Regions** (Tafel 3, DESIGN 12.9 Schritt 5): benannte Flächen (Rechteck, Kreis, gemalte Kacheln, zusammensetzbar) und Punkte – neu in der Karten-Datei. Das Spielfeld wird eine besondere Region. Rechts: wer die Region benutzt (damit Umbenennen nichts kaputt macht) | **gebaut, Rest offen** (`src\sim\regions.rs`, `src\editor\regions.rs`, Kartenformat 6) – siehe „Plan im Detail → Schritt 4, Rest“. „Benutzt von“ kommt mit dem ersten Nutzer (Schritt 6: Gruppen aus Regionen) |
| 5 | **Triggers** (Tafel 4, eigenes Vollbild-Modul): links der Baum – TABs feste Skripte (Start, Sieg, Niederlage, Knopf der Kommandozentrale, Ziele), Variablen, eigene Trigger in Ordnern, alle Objekt-Skripte zum Wiederfinden. Mitte: Ereignis → Bedingungen → Aktionen als **Sätze**, blaue Wörter anklicken zum Ändern; TABs Ablauf (Block, Loop/Exit Loop, Auswahl mit Bedingung je Option, Zufall nach Gewicht, Fenster, Sound). Umschalter **Sätze / Text** (unsere kleine Sprache). Rechts Aktionen zum Hineinziehen. Baut auf `docs\trigger-und-28yl.md` Schritte 2–5 auf | offen |
| 6 | **Events** (Tafel 5): TABs Events als **Zeitleiste** über alle Tage – Wellen, Endschwarm, Hinweise, Aufdecken, Sieg; Balkenhöhe = Anzahl Zombies. Unten die Werte des gewählten (Stunde, Zufall, Wiederholen, Wachstum, Seiten, Gruppen aus Spawnern oder aus Regionen – das braucht Schritt 4). „Neu planen“ rechnet aus TABs Regeln | **fertig** (`src\editor\events.rs`, Orders `PlanEvents`, `SendEvent`, `WaveFrom::Zones`, Kartenformat 7) – Vollbild-Seite; „Used by“ im Regions-Modul; ohne „Jump to day“ (die Simulation kennt kein Vorspulen), ohne „Constant spawn“ (TABs Stragglers sind Events); Balken ziehen nur von Hand geprüft |
| 7 | **Map** (Tafel 6): TABs Info (für die Liste eigener Level), Landschaft, Kartengröße, Spielfeld, Schwierigkeit und infizierte Bevölkerung mit TABs Stufen, Tage. **Spieler** wie SC2 (Kolonie, Verbündete mit KI, Neutral, Zombies – Farbe, Steuerung, Team, Startpunkt; die KIs selbst sind DESIGN 12.9 Schritt 7). **Sieg/Niederlage** wie TAB: Text, Bedingung, Fortschritt | **fertig** (`src\editor\map_page.rs`, Order `SetDifficulty`) – Vollbild-Seite (`Module::FULL`); **Difficulty** statt TABs „Infected population“ allein (unsere Stufen enthalten sie); Spieler nur zum Lesen, Wetter/Musik, Keep-out je Karte, Rules, Strings fehlen mit Absicht; Bild der Karte in der Vorschau erst, wenn das Spiel es irgendwo zeigt |
| – | **Data** (SC2s Data-Editor, leicht): Werte aus `data\` je Karte überschreiben (DESIGN 12.9 Schritt 8) | später |

## Reihenfolge

- Schritt 1 zuerst – alle anderen hängen an seinen Panels.
- Danach frei wählbar; 4 vor dem Regionen-Teil von 6, Triggers (5) zusammen mit `docs\trigger-und-28yl.md`.
- Gegenüber DESIGN.md 13 („Next“: erst Feinschliff, dann Features): der Editor ist ein
  Feature – wann er drankommt, entscheidet der Nutzer.

## Plan im Detail

Für jeden Schritt: was zu bauen ist, wo, in welcher Reihenfolge, und woran man merkt, dass er fertig ist. Empfohlene
Reihenfolge: **4 (Rest) → 7 → 6 → 5**. 7 und 6 brauchen nichts Neues in der Simulation außer dem hier Genannten; 5 hängt
an `docs\trigger-und-28yl.md` Schritte 2–5 und ist der größte.

### So ist der Editor gebaut (für alle Schritte)

- **Rahmen:** `src\editor.rs` – `Editor::show` baut jeden Frame alles neu (egui, immediate mode). `Module` (Enum) =
  die Knöpfe der Modulleiste: `ON_MAP` (Werkzeug links, Karte in der Mitte) und `PAGES` (Seiten mit Feldern). Ein
  Modul = eine Methode `xyz_module(ui, world, orders)` für links, plus ein Zweig in `properties_panel` für rechts.
  Große Module liegen in `src\editor\<name>.rs` als `impl Editor { … }` (Muster: `terrain.rs`, `regions.rs`).
- **Der Editor liest die Welt nur.** Jede Änderung ist ein `Order` (`src\sim\orders.rs`: Variante im Enum + Zweig in
  `apply`). Undo/Redo (`src\undo.rs`) spielt Orders nach – ein neuer Order braucht dafür nichts. Ein Schritt im Undo
  ist alles zwischen Maus-drücken und -loslassen.
- **Felder:** `settled(ui, wert, |v| DragValue::new(v)…)` und `settled_name` / `settled_text` geben den neuen Wert erst
  beim Loslassen zurück → ein Order statt einer pro Frame. Abschnitte: `section_from(ui, "Titel", Some(Origin::Tab|Sc2|New), "Hinweis", |ui| …)`.
- **Maus auf der Karte:** `src\ui.rs` – `use_tool` (Druck mit Werkzeug), `mouse_buttons` (Ziehen, Loslassen),
  `draw` (Vorschau). Werkzeuge sind `Tool`-Varianten in `editor.rs` (`name`, `hint`). Ziehen an Rechteck-Kanten:
  `RectDrag` + `Dragged` + `grab(…)`.
- **Karten-Datei:** `src\mapfile.rs` – Feld in `MapFile` (mit `#[serde(default, skip_serializing_if = …)]`), in
  `from_world`, `to_world`, `world_to_play`, `resized` (alles um `d` Kacheln verschieben) und in der Schlüssel-Liste
  von `pretty`. Bricht das Format für ältere Spiele: `VERSION` hoch, Zeile im Kommentar darüber, Test mit alter Datei.
- **Namen im Code:** Region = `Zone`, Punkt = `Spot` (weil `map::Region` schon das Kamera-Rechteck ist); in der Datei
  und im Editor heißen sie `regions` / `points`.
- **Prüfen:** `cargo test --release`, `cargo clippy --release --all-targets` (nur neue Warnungen zählen), dann Skill
  `run-last-bastion`: `launch editor; click 250 258; wait 800; click 1067 680; wait 3000; key F3` öffnet eine Karte;
  Screenshots mit `ss name` ansehen. Koordinaten = Pixel des Screenshots.
- **Danach nachziehen:** diese Tabelle, `DESIGN.md` 12.9 (*In the prototype:* …, Englisch), README-Tabelle „Editor“,
  `data\README.md` nur bei neuen Daten. Keine Datumseinträge.

### Schritt 4, Rest

Gebaut und im laufenden Editor gesehen: Modul „Regions“ (Werkzeuge + Rectangle / + Circle / + Paint / + Point; Listen
Special, Regions, Points, Spawners mit Suche; rechts Name, Farbe, Formen mit Zahlen, Locate / Duplicate / Delete),
auf der Karte auswählen, verschieben, Rechteck-Kanten ziehen, Ebene „Regions and points“ unter „Show“, Statuszeile,
Orders `AddZone` `SetZone` `PaintZone` `RemoveZone` `AddSpot` `SetSpot` `RemoveSpot`, Kartenformat 6, 3 Tests
(`sim::regions::tests`). Offen:

1. **Entf löscht nicht.** `picking.rs` → `clip_keys` schluckt die Taste auch ohne Auswahl. In `match action` vor
   `ClipAction::Delete => self.remove(…)` einfügen: `ClipAction::Delete if pieces.is_empty() => return false,`. Dann
   greift der Zweig in `ui.rs` (`handle_input`: `orders.extend(self.editor.remove_picked())`). Prüfen: Punkt anklicken,
   Entf → weg; Strg+Z → wieder da.
2. **Nicht im laufenden Editor geprüft:** Strg-Ziehen (Form dazu), Alt-Ziehen (ausschneiden), Kreis-Vorschau, Speichern
   und Laden mit Regionen, Duplicate, Farbe ändern, Regionen im Test-Modus. Der Treiber kann keine Taste beim Ziehen
   halten – Strg/Alt per Hand prüfen lassen oder im Treiber ergänzen.
3. **Namen überlappen** (Punkt in Region): den Punkt-Namen nur zeichnen, wenn er gewählt ist oder die Maus nah ist –
   oder ihn über den Punkt setzen. `regions.rs` → `draw_regions`.
4. **Spielfeld als Region:** steht nur in der Liste (Klick = Werkzeug „Drag the border“). Reicht für jetzt; die
   Trigger-Aktion „Spielfeld auf Region X“ kommt mit Schritt 5.
5. **Doku:** `DESIGN.md` 12.9 Schritt 5 (*In the prototype:* …), README-Tabelle „Editor“ (Zeile Regions),
   `src\mapfile.rs`-Kopfkommentar (regions, points), Memory `editor-egui-next-step`.

### Schritt 7 – Map: fertig

`src\editor\map_page.rs`; `Module::FULL` = Seiten, die die ganze Mitte füllen (für 5 und 6 wieder nutzbar: Eintrag in
`FULL` und ein Zweig in `Editor::show`). Offen mit Absicht: Keep-out je Karte (bräuchte ein eigenes Feld in der Datei),
das Bild der Karte in der Vorschau (das Spiel zeigt das Bild noch nirgends).

### Schritt 6 – Events (Tafel 5, `project/Events.dc.html`)

Heute: `events_module` + `event_box` in `editor.rs` (eine Liste aufklappbarer Kästen), `send_now` rechts.
`Event`-Felder in `src\sim\events.rs`; Orders `AddEvent`, `SetEvent { index, event }`, `RemoveEvent { index }`.

1. **Eigene Datei** `src\editor\events.rs`; `events_module`, `event_box`, `wave_group`, `from_box`, `spawner_checks`,
   `send_now` dorthin ziehen (erst nur verschieben, bauen, testen). Vollbild-Seite wie in Schritt 7.1.
2. **Kopfzeile:** „N days · M events · Z zombies in all“ (Summe über Gruppen × Wiederholungen mit Wachstum – eine
   Funktion `Event::total()` in der Sim, mit Test), Knöpfe + Wave / + Notice / + Reveal (ein Event mit passenden
   Voreinstellungen), Umschalter „Days 1–30 / All“ (nur der gezeigte Bereich).
3. **Zeitleiste** mit `egui::Painter` (`ui.allocate_exact_size(…, Sense::click())`, dann `painter.rect_filled`,
   `line_segment`, `text`): x = Stunde / (Tage · 24), Zeilen Waves (Balkenhöhe ∝ Zombies, √-Skala damit kleine
   sichtbar bleiben), Final, Notices (Punkt bei `hour − notify_hours`), Reveal, Victory. Wiederholte Events: ein
   Balken je Mal. Klick auf einen Balken wählt das Event (`self.event: Option<usize>`, nächster Balken zur Maus);
   der gewählte bekommt einen gelben Rahmen und „day D · Z“. Ziehen eines Balkens = neue Stunde (`SetEvent` beim
   Loslassen) – erst wenn Klick-Auswahl sitzt.
4. **Unten links** die Werte des gewählten (die Felder aus `event_box`, als Formular statt Klappkasten; Duplicate,
   Delete), **unten rechts** die Gruppen als Tabelle: Zeile = Gruppe, Spalten = From + je Art eine Zahl.
5. **Gruppen aus Regionen** (erster Nutzer der Regionen):
   - `WaveFrom::Zones(Vec<u32>)` in `src\sim\spawning.rs`. `wave_sides` gibt heute Spawner-Indizes je Seite zurück,
     `Request` (`events.rs`) hält `spawners: Vec<u32>` + `square`. Erweitern: `Request` bekommt
     `zone: Option<u32>`; beim Herauslassen (`stream…`) für eine Zone eine zufällige freie Kachel aus
     `Zone`-Kacheln nehmen (Hilfsfunktion `World::point_in_zone(id) -> Option<Vec2>`: `bounds()` würfeln, bis
     `contains` und begehbar, höchstens 20 Versuche; sonst Mitte). Nur `self.rng`, keine Gleitkomma-Tricks.
   - `RemoveZone` räumt die Nummer aus allen Gruppen (`forget_zone`, Muster `forget_spawner`).
   - Editor: in `from_box` „chosen regions“ + Häkchen-Liste (`spawner_checks` als Muster).
   - **„Used by“** im Regions-Modul: `World::zone_uses(id) -> Vec<String>` („Wave day 20, group 2“), rechts unter der
     Region zeigen; „Delete“ bei benutzter Region erst fragen (egui-Fenster mit der Liste).
   - Kartendatei: `WaveFrom` ist serde – alte Dateien bleiben lesbar; ein älteres Spiel kann die neue Variante nicht
     lesen → `VERSION` 7. Tests: Welle aus Region landet in der Region; Region gelöscht → Gruppe leer; Datei hin und
     zurück.
6. **Rechts:** „Plan from rules“: Difficulty, Days, Population + Knopf „Plan again (replaces all N)“ → neuer Order
   `PlanEvents { days, population }` = `world.events = plan_events(days, population)` + `roll_events()`; vorher
   fragen. „Send now“ wie heute, dazu „This event now“ (Order `FireEvent { index }` – prüfen, ob `events.rs` schon
   eine Funktion zum sofortigen Auslösen hat) und „Jump to day D“ **nur im Test** weglassen, wenn die Sim kein
   Vorspulen kennt. „Constant spawn“ gibt es nicht mehr (TABs Stragglers sind Events) → weglassen.
7. Fertig, wenn: Zeitleiste zeigt alle Events der Standardkarte, Klick wählt, Werte ändern = Balken ändert sich,
   Undo geht, Gruppe aus einer Region kommt im Test dort heraus.

### Schritt 5 – Triggers (Tafel 4, `project/Triggers.dc.html`)

Erst die Simulation, dann der Editor. Die Simulations-Schritte stehen in `docs\trigger-und-28yl.md` (2 Trigger als
Daten, 3 Ausdruckssprache, 4 Ablauf, 5 Ereignisse) – jeder ist eine eigene Unterhaltung. Der Editor wächst mit:

1. **Nach Trigger-Doku 2** (Trigger als Daten, Variablen): Vollbild-Seite, **links der Baum** (Story: On start / On
   victory / On defeat / Command center button / Goals; Variables; Triggers in Ordnern; Object scripts = alle
   `Neutral` mit Beitritts-Skript, Klick springt hin), Suche, „+ New“. **Mitte** vorerst die heutige Aktionsliste aus
   `story_panel.rs` für den gewählten Knoten. Kopf: Name, „On“, „Only once“, Kommentar.
2. **Sätze:** jede Aktion als Zeile mit anklickbaren blauen Wörtern (`ui.link`-artige Labels; Klick öffnet ein
   kleines Popup mit dem passenden Feld). Eine Funktion `sentence(action) -> Vec<Part>` (Text | Parameter mit Index),
   damit Zeichnen und Bearbeiten dieselbe Quelle haben. Einrückung für Kinder (Choose-Optionen, Random-Gewichte).
3. **Nach Trigger-Doku 3:** Blöcke „When“ (Ereignis) und „If“ (Bedingungen), Feld für Ausdrücke mit sofortiger
   Fehlermeldung (Stelle markieren), Umschalter **Sentences / Text** (Text = die Aktion in unserer Sprache, getippt
   und zurück in den Baum geparst).
4. **Nach Trigger-Doku 4:** Block, Loop / Exit loop, Wait, Bedingung je Option.
5. **Rechts die Palette** (Flow, Zombies, Colony, Show, Map, End) mit Suche; Klick hängt die Aktion ans Ende, Ziehen
   in die Liste (egui `dnd_drag_source` / `dnd_drop_zone`) an die Stelle. Strg+Leertaste: Suchfeld mit Tippen.
6. **Nach Trigger-Doku 5 + Regionen:** Ereignisse „unit enters / leaves region“, Aktionen „Spawn at region“,
   „Camera to region/point“, „Playing field to region“ – jede trägt sich in `zone_uses` ein (Schritt 6.5).
7. Statuszeile: „Trigger: X · N conditions · M actions · No errors“.

### Was bewusst fehlt (nicht als toten Knopf bauen)

- Wetter und Musik je Karte, Regeln je Karte (`ApplyRules` → Data-Modul), Strings-Tabelle, verbündete KIs, „Jump to
  day“, Constant spawn – jeweils erst mit der Logik dahinter.
