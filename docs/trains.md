# Züge ausbauen: Gleisnetz, Haltestellen, Fahrpläne per Skript

Ziel: mehr als TABs eine Strecke. Mehrere Strecken mit **Weichen**, **Haltestellen** als Gebäude, **Einfahrten** von
außerhalb der Karte, Züge mit **ID**, die Skripte erscheinen lassen und fahren schicken. Jeder Schritt ist ein Startpunkt
für eine neue Unterhaltung („Lies docs/trains.md, Schritt N“). Grundlage: der einfache Zug, wie TABs
(`src\sim\rails.rs`, `trains.rs`, `src\train_sprites.rs`, `src\rail_planner.rs`).

**Entschieden:**
- **Weichen und Gleisnetz:** Gleise dürfen sich verzweigen, ein Zug sucht sich seinen Weg.
- **Haltestelle = Gebäude mit Leben:** Zombies greifen sie an, zerstört hält dort kein Zug mehr.
- **Skriptgesteuert:** Jeder Zug hat eine ID (`TA`, `TB` …). Skripte lassen ihn **erscheinen** (an einer Haltestelle oder
  einer benannten Einfahrt außerhalb der Karte), schicken ihn **zu einer Haltestelle** und **weiter**: woanders
  stehen bleiben oder die Karte über eine Ausfahrt verlassen. Haltestelle „Command Center“ = der Halt am Kommandozentrum.
- **Zusammenstöße:** Kommen sich zwei Züge zu nahe, bleibt der hintere stehen.

## Schritte

| # | Was | Status |
|---|---|---|
| 1 | **Editor:** Gleisplaner wie Factorio (offenes Ende oder freier Boden anklicken → Pfeile in jede mögliche Richtung, das Stück, auf das die Maus zeigt, als Ghost; Klick baut, Ziehen baut weiter zur Maus, Rechtsklick hört auf; Gleis anklicken zweigt davon ab, ein Prellbock wird weitergebaut; nichts doppelt, nichts außerhalb der Karte, kein Prellbock auf Gleis; trifft das neue Stück anderes Gleis, ist Schluss), Ghost-Bild auch für die Bahn-Deko, Richtung im Namen (`\` entlang x, `/` entlang y), Anzeige auf der Karte: wo der Zug reinkommt und wo er am CC hält (oder dass keine Strecke lang genug ist) | fertig |
| 2 | **Gleisnetz mit Weichen** wie Factorio: kein eigenes Weichenstück – zwei Stücke am selben Anschluss *sind* eine Weiche, ein gerades x- und y-Stück auf einer Kachel eine Kreuzung. Netz aus Anschlüssen, Wegsuche nur vorwärts (Dijkstra), der Zug fährt eine Route statt einer Strecke. Weichen im Editor markiert. Gebaut: `RailNet` in `src\sim\rails.rs` (Anschlüsse nach Kachelseite, Dijkstra nur vorwärts, Weichen = ein Anschluss mit mehr als einem Stück dahinter); `routes()` gibt von jedem Prellbock den kürzesten Weg zu jedem Ende, das er erreicht (Prellbock oder letztes Stück), als `RailLine` – Zug, Ton und Bilder lesen sie wie vorher. Der Editor markiert Weichen gelb mit „switch“ (`draw_train_plan`). Die heutige Aktion „Train“ nimmt die Route, die am nächsten am Kommandozentrum vorbeiführt. **Noch offen:** ein Zug mit festem Ziel (Haltestelle, Ausfahrt) – `RailNet::search` kann es schon, `route` fehlt ein Ziel-Parameter, kommt mit Schritt 3 und 5; mehrere Wege zum selben Ende (nur der kürzeste); das Kreuzungsstück zeichnet der Planer nicht besonders; den Editor mit einer Weiche noch nicht von Hand angesehen | teils fertig |
| 3 | **Haltestellen** als Gebäude (`TrainStation`, 2 × 2, vorerst das verkleinerte Bild des Kommandozentrums): Leben, Schild, infizierbar, nur im Editor setzbar, ihr Name = Name-ID. Gehalten wird auf dem Gleis direkt daneben (Editor zeigt es): **die Mitte der Lok am Punkt des Gleises, der der Mitte der Haltestelle am nächsten ist** (statt heute: Mitte des nächsten Gleisstücks, auf einer Kurve 8 Kacheln lang). Gilt nur, wenn das Gleis höchstens ~1 Kachel neben der Haltestelle vorbeiläuft. „Command Center“ ist eine normale Haltestelle, die man neben das CC stellt – sie sagt nur, wo die Lok hält; ohne sie hält der Zug am CC nicht. Liegt sie näher am Streckenanfang als der Zug lang ist, fährt er von der anderen Seite ein – sonst warnt der Editor. Gebaut: `BuildingKind::TrainStation` (`data\buildings\`, nur im Editor setzbar, in keinem Baumenü), `World::station_stop` (nächster Punkt des Gleises, `RailLine::nearest`, höchstens `STATION_REACH` = 1 Kachel neben dem Platz), `train_plan` hält an der Haltestelle „Command Center“ (nur auf Routen, die an ihr vorbeiführen; zerstört hält dort keiner). **Abweichung:** ohne eine Haltestelle „Command Center“ hält der Zug wie bisher am nächsten Gleisstück zum Kommandozentrum – so laufen alte Karten weiter. Der Editor zeigt je Haltestelle den Haltepunkt (oder „no track right next to it“) und warnt bei zu kurzem Anlauf. **Noch offen:** eigenes Bild (vorerst das CC-Bild verkleinert), Haltestellen anderer Namen als Ziel (kommt mit Schritt 5), von Hand im Editor noch nicht angesehen | teils fertig |
| 4 | **Einfahrten und Ausfahrten** außerhalb der Spielfläche: ein benanntes Gleisende im Rand hinter der Weltgrenze (der Rand ist dafür breiter geworden, Gleis bauen geht dort schon). Dort erscheint ein Zug ganz und fährt herein – oder fährt hinaus und verschwindet | offen |
| 5 | **Züge mit ID per Skript:** „Spawn train ‹ID› at ‹Haltestelle/Einfahrt›“ (welche Wagen), „Send train ‹ID› to ‹Haltestelle/Ausfahrt›“ (Wartezeit, Skript bei Ankunft), „Remove train“; Bedingung „Zug ‹ID› ist an ‹Haltestelle›“ für Ziele und Trigger. Die heutige Aktion „Train“ wird in alten Karten zu Spawn + Send | offen |
| 6 | **Abstand:** der hintere Zug bremst vor dem vorderen und wartet; Gegenverkehr auf einem Gleis: beide warten – der Editor warnt, wenn zwei Fahrten sich gegenüberstehen können | offen |

## Vorschläge (noch nicht entschieden)

- **Signale wie Factorio** (Blocksignale) als Schritt 7: Mit „der hintere bleibt stehen“ allein blockieren sich zwei
  Züge auf einem eingleisigen Stück für immer. Signale teilen das Netz in Blöcke, ein Zug fährt nur in einen freien.
- **Wenden nur an Haltestellen und Prellböcken** (Kopfbahnhof: die Lok setzt um, im Bild springt sie ans andere Ende).
  Ohne Wenden braucht jedes Ziel eine Wendeschleife; überall wenden sieht falsch aus.
- **Zuglärm wie TABs Lärm** (`sim/sound.rs`): ein fahrender Zug lockt Zombies an – ein Bahnhof wird so zum Risiko, und
  die Haltestelle als Gebäude mit Leben bekommt ihren Sinn.
- **Ankunft als Ereignis** für die geplanten SC2-Trigger (`docs\trigger-und-28yl.md`): „Wenn Zug ‹ID› an ‹Haltestelle›
  ankommt“ statt nur eines Skripts in der Fahrt.
- **Einfahrten als Regionen** (SC2-Editor-Plan, `docs\editor.md`): eine Einfahrt wäre dann eine benannte Region am Rand,
  die ein Gleisende enthält.
