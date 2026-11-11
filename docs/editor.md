# Der Editor wie SC2, mit allem von TAB

**Ziel:** Der Editor bekommt den Aufbau von StarCraft 2s Editor – alle Funktionen aus TABs Editor bleiben drin. Vorlage
ist der Entwurf (6 Tafeln, vom Nutzer abgenommen): https://claude.ai/artifact/YChSdLLzGWb4TFfNd5wrn9

Jeder Schritt ist ein Startpunkt für eine neue Unterhaltung („Lies docs/editor.md, Schritt N“). Was dahinter steht,
plant DESIGN.md 12.9 (Englisch); die Trigger-Sprache und 28YL stehen in `docs\trigger-und-28yl.md`.

## Für jeden Schritt

- README → „Regeln für Claude“: Code Stück für Stück erklären, am Ende eine kurze Zusammenfassung.
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
| 2 | **Objects** (Tafel 1): links alle Arten **mit Bild** (Porträts/Gebäudebilder aus `assets\`), in Gruppen, mit Suche und Filtern (Colony, Defense, Units, Neutral, Decor, Loot); „Placing“: Besitzer, Drehen (R), Shift = weiter setzen, „frei“ (ohne Kosten, ohne Arbeiter). Unten die Liste von allem Platzierten (TABs Entity-Liste: Spalten Art, Name-ID, Kachel, Besitzer, Leben, Skripte; Filter, Suche; Klick wählt, Doppelklick springt hin). Rechts die Eigenschaften, **Mehrfachauswahl ändert alle**. Neue Arten (TAB-Gebäude, Wunder, Deko, Raben) erscheinen, sobald `docs\tab-assets.md` sie ins Spiel bringt | offen |
| 3 | **Terrain** (Tafel 2): Werkzeuge Pinsel, Linie, Rechteck, Füllen, Rauschen, Glätten, Bereich kopieren/einfügen, Generator nur in der Auswahl (Wald, See, Berge, Erzfeld, Village of Doom – mit Seed). Ebenen wie TABs Painter: Boden, Wald, Felsen, Erz, je Bild A/B/C oder zufällig, Dichte; Pinsel rund/eckig, „Gebäude und Wege frei lassen“. Landschaft umschalten (BR … VO). Rechts Minimap, SC2s **Ebenen-Schalter** (Boden, Wald/Felsen/Erz, Gebäude und Einheiten, Zombies, Regionen, Gitter, Nebel, Wege der Zombies, Lärm als Wärmebild) und „Unter der Maus“ (Kachel, Boden, was drauf ist, begehbar) | offen |
| 4 | **Regions** (Tafel 3, DESIGN 12.9 Schritt 5): benannte Flächen (Rechteck, Kreis, gemalte Kacheln, zusammensetzbar) und Punkte – neu in der Karten-Datei. Das Spielfeld wird eine besondere Region. Rechts: wer die Region benutzt (damit Umbenennen nichts kaputt macht) | offen |
| 5 | **Triggers** (Tafel 4, eigenes Vollbild-Modul): links der Baum – TABs feste Skripte (Start, Sieg, Niederlage, Knopf der Kommandozentrale, Ziele), Variablen, eigene Trigger in Ordnern, alle Objekt-Skripte zum Wiederfinden. Mitte: Ereignis → Bedingungen → Aktionen als **Sätze**, blaue Wörter anklicken zum Ändern; TABs Ablauf (Block, Loop/Exit Loop, Auswahl mit Bedingung je Option, Zufall nach Gewicht, Fenster, Sound). Umschalter **Sätze / Text** (unsere kleine Sprache). Rechts Aktionen zum Hineinziehen. Baut auf `docs\trigger-und-28yl.md` Schritte 2–5 auf | offen |
| 6 | **Events** (Tafel 5): TABs Events als **Zeitleiste** über alle Tage – Wellen, Endschwarm, Hinweise, Aufdecken, Sieg; Balkenhöhe = Anzahl Zombies. Unten die Werte des gewählten (Stunde, Zufall, Wiederholen, Wachstum, Seiten, Gruppen aus Spawnern oder aus Regionen – das braucht Schritt 4). „Neu planen“ rechnet aus TABs Regeln | offen |
| 7 | **Map** (Tafel 6): TABs Info (für die Liste eigener Level), Landschaft, Kartengröße, Spielfeld, Schwierigkeit und infizierte Bevölkerung mit TABs Stufen, Tage. **Spieler** wie SC2 (Kolonie, Verbündete mit KI, Neutral, Zombies – Farbe, Steuerung, Team, Startpunkt; die KIs selbst sind DESIGN 12.9 Schritt 7). **Sieg/Niederlage** wie TAB: Text, Bedingung, Fortschritt | offen |
| – | **Data** (SC2s Data-Editor, leicht): Werte aus `data\` je Karte überschreiben (DESIGN 12.9 Schritt 8) | später |

## Reihenfolge

- Schritt 1 zuerst – alle anderen hängen an seinen Panels.
- Danach frei wählbar; 4 vor dem Regionen-Teil von 6, Triggers (5) zusammen mit `docs\trigger-und-28yl.md`.
- Gegenüber `docs\tab-assets.md` (erst alle TAB-Bilder ins Spiel, dann Feinschliff, dann Features): der Editor ist ein
  Feature – wann er drankommt, entscheidet der Nutzer.
