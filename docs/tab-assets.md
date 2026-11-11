# TABs restliche Bilder einbauen

Ziel: alles, was TAB an Bildern hat, kommt ins Spiel – danach Feinschliff, Logik, weitere Features, und später eigene
Bilder statt TABs. Jeder Schritt ist ein Startpunkt für eine neue Unterhaltung („Lies docs/tab-assets.md, Schritt N“).

**Quellen:** Bilder in `E:\Decoded_Software\TheyAreBillions_Decompiled\_assets\` (`Sprites\` = Figuren, Gebäude,
Effekte als Einzelbilder; `Images\` = Icons, Porträts, Interface, Geschosse, Effektbilder; `Cursors\`, `Animations\`),
Werte in `_assets\Data\ZXRules\*.tsv` (`Entities`, `Commands`), Vorlagen in `ZXGame.dxprj` (TAB-Ordner), wann welcher
Effekt kommt in `ZX\VisualEffects.cs`. Alles bleibt lokal, nie in git.

**Entschieden:**
- Neue Einheiten mit TABs Werten: **Lucifer** = unser Flammenwerfer, **Thanatos**, **Titan** (erstmal wie in TAB).
  Unsere Mechaniken aus DESIGN.md bleiben (Veteranen, Regeneration …); weicht TAB davon ab: auflisten, nicht bauen.
- Neue Gebäude: Holzhaus (Wohnstufe zwischen Zelt und Steinhaus), Kraftwerk, Ölplattform, Steinbruch, Shocking Tower,
  Executor, Klingenfalle, Mine, Radar, Aussichtsturm, Lagerhaus, Ingenieurszentrum, Wunder – und was es dazu gibt
  (großer Steinbruch, große Farm, große Mühle).
- Raben und Ratten kommen rein (Deko).
- Sanitäter = silberner Soldat, Speerträger = Marine, beide ohne Angriffsbild; der Soldat nur noch normal.
- Nicht geplant: Mutant, Helden, Kampagnen-Gebäude. **Züge** doch (Bilder da, Schritt 13).

## Schritte

**Alle Bilder und Töne liegen schon in `assets\`** (mit den Skripten in `tools\` gemacht, Namen in `assets\README.md`) –
offen ist nur noch der Einbau von Schritt 12 (Töne) und 13 (Züge).

| # | Was | Status |
|---|---|---|
| 1 | **Figuren:** Skins für Sanitäter und Speerträger, Bilder von Lucifer, Thanatos (mit Nahkampf `fight`), Titan | fertig |
| 2 | **Leiste und Menüs:** TABs Rahmen, Knöpfe, Rohstoff- und Befehlsicons, Porträts der Auswahl, Ereignis-Icons, Mauszeiger (`tools\tab_ui.py` → `assets\ui\`, `src\hud.rs`, `src\ui_art.rs`) | fertig |
| 3 | **Effekte zu dem, was es schon gibt:** Feuer und Rauch an beschädigten Gebäuden, Tesla-Kugel, MG-Schüsse, Reichweitenkreis, Bau-Raster, Lichter, Wasser, Wetter (Regen, Schnee, Blätter) – wann, wie in `VisualEffects.cs` | fertig |
| 4 | **Neue Einheiten:** Lucifer (Feuerkegel, brennt, trifft auch eigene), Thanatos (Rakete 10 Kacheln, ab 3, Fläche 1,8; Nahkampf-Schlag), Titan (Schuss 9 Kacheln, Fläche 0,8) – Werte aus `Entities`/`Commands`, Rakete `Missile_A` | fertig |
| 5 | **Neue Wirtschaftsgebäude:** Holzhaus, Kraftwerk, Ölplattform, Steinbruch (+ großer), große Farm, große Mühle, Lagerhaus, Ingenieurszentrum (`tools\tab_buildings.py`) | fertig |
| 6 | **Neue Abwehr:** Shocking Tower, Executor, Klingenfalle, Mine – mit ihren Effekten (`ShockRay`, `ElectricSphere` …) | fertig |
| 7 | **Radar, Aussichtsturm, Wunder** (Spire, Academy, Victorious, Transmutator, Crystal Palace) mit TABs Wirkung | fertig |
| 8 | **Raben und Ratten** als Deko (Raben fliegen auf, wenn jemand kommt) | fertig |
| – | **Pflaster unter Gebäuden** (TABs `CFloorUndeBuilding`: `Floor A` mit weichem Rand, nicht unter Farmen) | fertig |
| 9 | **Weizen auf den Feldern** der Farm (`Vegetation\Wheat_A`, TABs `CFarmer`) statt unserer grünen Rauten | fertig |
| 10 | **Beute und Ruinen als Bilder:** `Survival\Pickable*` auf der Karte (jetzt Klötzchen), Schatz-Ruinen `RuinTreasureA` | fertig |
| 11 | **Deko auf der Karte:** Strommasten `TensionTowerHigh` im Wald (TABs `ZXMapTheme.Decorate`), Menü-Hintergründe (`Images\Backgrounds`) | fertig |
| 12 | **Töne:** ~300 Geräusche, Musik, Umgebung, Stimmen, Meldungen (`_assets\Sounds\`) – das Spiel hat noch gar keinen Ton | Töne da (`assets\sounds\`) |
| 13 | **Züge:** Lok, drei Wagen, Räder (`tools\tab_trains.py` → `assets\trains\`), Schienen, Prellböcke, Bahnbrücke, Lampe, Loren (`decor\rail_*`), Icons `IconFTrain`, `EventTrainArrival`, Töne `Train_*` – fahren wie TABs `Locomotive.cs` (auf Schienen, hält an Stopps, überfährt Einheiten), in Kampagnen per Skript (`ZXAction_SendTrain`) | Bilder da |

## Abweichungen von TAB (Schritte 3–11)

Beim Einbau bewusst anders oder vereinfacht – zum Entscheiden beim Feinschliff:
- **Kein Forschen:** TAB schaltet viele späte Sachen erst per Forschung frei (`Tech…` in `Commands`); bei uns reicht das
  Gebäude, das sie voraussetzt (`requires`).
- **Feuer breitet sich nicht aus:** nur wen Lucifer trifft, brennt (TAB: alles im Feuer heizt sich auf und fängt Feuer,
  auch eigene Einheiten).
- **Akademie:** wer Veteran wurde, bleibt es, wenn sie fällt (TAB: nur solange sie steht).
- **Brand an beschädigten Gebäuden** ist unsere Idee (TABs Gebäude brennen nur durch Feuerschaden).
- **Kraftwerk** verbrennt Kohle statt TABs Holz (DESIGN.md 5.2: Kohlekraftwerk).
- **Shocking Tower:** lädt 4,5 s (TABs 4 s Laden + 0,5 s Schuss); kein Ladebalken, seine Bild-Lichter laufen fest.
- **Ratten** sitzen an Ruinen und Dorfhäusern (TAB hat sie nur in der Kampagne); Raben und Ratten nur im Bild.
- **Mine** ohne TABs einstellbare Verzögerung (0–3 × 0,5 s, `DelayActivation`) – sie geht sofort hoch; TAB hat kein
  Porträt für sie (Knopf mit Buchstaben).

## Danach

- **Feinschliff und Logik:** Balance (der einfache Spieler verliert an Tag 2–3), geschätzte Werte prüfen.
- **Features:** Entscheidung über die alten Kartengeneratoren; Editor wie SC2 (Regionen, Trigger als Sätze, KI-Spieler –
  `docs\trigger-und-28yl.md`); Konsole/Chat wie Factorio mit Pings; Koop; Fähigkeiten der Einheiten; Leute, die
  herumlaufen (DESIGN 5.6, TABs Arbeiter-Bilder liegen bereit); Creep (DESIGN 8.4).
- **Eigene Bilder** statt TABs (offene Fragen 21, 22).
