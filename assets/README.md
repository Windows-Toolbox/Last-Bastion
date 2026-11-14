# Bilder (`assets\`)

Alle Bilder des Spiels – vorgerenderte **Sprite-Sheets** wie in TAB (DESIGN.md 12.3), je Art ein Ordner:

| Ordner | Was | Gemacht von | Gelesen von |
|---|---|---|---|
| `units\` | Unsere Einheiten (Ranger, Soldat, Scharfschütze, Medic, Speerträger) und TABs Lucifer, Thanatos, Titan | `tools\tab_skins.py` | `src\sprites.rs` |
| `zombies\` | Die Zombies (15 Skins mit den Einheiten zusammen) | `tools\tab_skins.py` | `src\sprites.rs` |
| `buildings\` | Gebäude, Mauerstücke, Dorfhäuser, Fässer | `tools\tab_buildings.py` | `src\building_sprites.rs` |
| `terrain\<BR\|TM\|AL\|DS\|FA\|VO>\` | Boden, Wasser, Bäume, Felsen, Kräuter, Erz, Klippen am Wasser je Landschaft | `tools\tab_terrain.py` | `src\ground.rs` |
| `effects\` | Blut, Mündungsfeuer, Explosionen, Trümmer, Flecken am Boden | `tools\tab_effects.py` | `src\effects.rs` |
| `projectiles\` | Pfeil und Ballista-Bolzen | `tools\tab_effects.py` | `src\effects.rs` |
| `ui\` | Die Leiste: ihr Bild, Knöpfe, Zahnrad, `icons\`, `portraits\`, `events\`, `cursors\`; `backgrounds\` (Menü) | `tools\tab_ui.py` | `src\ui_art.rs`, `src\hud.rs` |
| `logo\` | **Unser eigenes Logo** (`last_bastion.png`) – nicht von TAB, darum in git | `tools\make_logo.py` | `src\home.rs` (Startbildschirm) |
| `animals\` | Raben und Ratten (Deko) | `tools\tab_skins.py` | `src\critters.rs`, `src\sprites.rs` |
| `decor\` | Weizen, Beute, Schatz-Ruinen, Strommasten, brennender Lastwagen, Zelt- und Brunnenruinen, Schienen | `tools\tab_buildings.py` | `src\draw.rs` (alles außer Zelt- und Brunnenruinen) |
| `trains\` | Lok, Panzer-, Kolonisten-, Güterwagen, Räder | `tools\tab_trains.py` | `src\train_sprites.rs` |
| `sounds\` | `Effects\`, `UnitVoices\`, `Notifications\`, `Interface\`, `Ambient\`, `Music\` (alles `.ogg`/`.wav`) | `tools\tab_sounds.py` | `src\audio.rs` (welcher zu wem: `data\audio.json`) |
| `test\` | Unser graues Testbild (in git) | `examples\test_sprites.rs` | `src\sprites.rs` |

- **TABs Bilder** (alle Ordner außer `test\`): urheberrechtlich geschützt – nur auf diesem PC, **nie in git**
  (`.gitignore`). Die Skripte machen sie aus deinem entpackten TAB und seiner `ZXGame.dxprj` neu. Fehlt ein Bild, wird es
  übersprungen (Kästchen, Klötze, Rauten, Linien wie ohne TAB).
- **Welche Art welches Aussehen hat**, steht in `data\`: `"skins": ["zombies/walker_aged", …]` →
  `assets\zombies\walker_aged.json`. Jede Figur trägt eins davon ihr Leben lang (Einheiten nach ihrer Nummer, Zombies
  nach ihrem `look`, auch im Schlaf). Sind alle weg, nimmt sie `"sprite"` (das graue `test/figure`). Ohne beides (oder
  wenn die Dateien kaputt sind) bleibt sie ein Kästchen, oben links steht dann eine rote Zeile.
- **Färbung** (alle Werkzeuge, `colour_matrix` in `tab_skins.py`): TABs Farbmatrix Zahl für Zahl – Helligkeit wird
  addiert, Kontrast multipliziert (nicht um Mittelgrau gespreizt), Farbton gedreht wie TAB.
- **Figuren** (`tab_skins.py`): zugeschnitten, verkleinert (80 Pixel pro Kachel), eingefärbt wie TAB, höchstens 16
  Bilder je Animation (Stehen, Knurren, Sterben 12), die Geschwindigkeiten aus TABs `TimeAni…`. Etwa 290 MB
  Grafikspeicher.
- **Eigene Skins** (`paint` in `tab_skins.py`): eine TAB-Figur, ein Material nach Helligkeit neu eingefärbt (Schatten
  und Falten bleiben). `soldier_silver` = Soldat mit polierten Stahlplatten; `soldier_marine` = TABs Lucifer-Anzug als
  Marine (blaugraue Platten, Amber-Visier, 0,8× so groß, Schuss-Takt des Soldaten). Nur einzelne bauen:
  `python tools\tab_skins.py <_assets> soldier_silver soldier_marine`.
- **Geliehen** (`ALIASES` in `tab_skins.py`): `medic.json` = das Bild von `soldier_silver`, `spearman.json` = das von
  `soldier_marine` – nur eine eigene JSON, ohne `attack`/`load`/`fight` (ein Schuss sähe falsch aus): sie stehen beim
  Arbeiten.
- **Lucifer, Thanatos, Titan:** die Einheiten aus dem Ingenieurszentrum (`data\units.json`).
  Titan hat kein Gehen (nur Rennen) und ist kleiner gerechnet (sonst über 4096 Pixel).
- **Landschaften** (`tab_terrain.py`): je Landschaft `earth.png`, `grass.png`, `water.png` (wiederholen sich alle
  `tiles` Kacheln), `objects.png` (Bäume, Felsstücke, TABs **große Felsen** 3×3/4×4/5×5 für den Editor – gefärbt wie TABs Gebirge der Landschaft –, Kräuter, Erz, Kohle, Ölquelle), `floor.png` (TABs Pflaster unter Gebäuden, in der Farbe der Landschaft) mit `floor_masks.png` (seine drei weichen Ränder für 1, 2, 3+ Kacheln breite Gebäude) und `terrain.json`. Welche eine
  Karte nimmt, sagt `look` in `data\game_maps.json`.
- **Gebäude** (`tab_buildings.py`): aus TABs Clip, Ebene für Ebene, in drei Looks: **arbeitet**, **aus** (blasser, kein
  Licht, alles steht) und **im Bau** (TABs grüner Geist). Ein Look besteht aus **Teilen** wie TABs Ebenen: was still
  steht, ist ein Bild; jede Maschine, Fahne, Rauchfahne ein Teil mit **allen** TAB-Bildern in ihrem eigenen Takt; jedes
  Licht ein Teil, das **addiert** wird (es leuchtet). So brauchen nur die kleinen bewegten Stücke viele Bilder. Endet
  ein TAB-Loop mit (fast) seinem ersten Bild (Radar, Fahnen, MG), fällt das letzte weg – sonst ruckelt es jede Runde. Ballista
  und MG: ihr Geschütz geht einmal im Kreis (das Spiel nimmt das zum Ziel), die Ballista hat dazu ihren Schuss je 18°.
  Die Größe nach **unserer** Breite (`size` in `data\buildings.json`). Mauern und Tor als Stücke wie TABs `CWall`,
  Türme als Körper + Vorderwand. Köhlerei = TABs Kraftwerk, Munitionsfabrik = TABs Lagerhaus. Etwa 133 MB.
- **Effekte** (`tab_effects.py`): TABs Effekt-Vorlagen (Bilder, Dauer, Größe, Färbung, Ausblenden) – wann welcher kommt,
  wie TAB (`VisualEffects.cs`): Treffer → `blood_splash`, toter Zombie → `blood_zombie` + `blood_stain`, tote Einheit →
  `blood_human`, Säure → `venom` + `venom_pool`, Fass → `fire_blast`, MG-Treffer → `gun_impact_a/b`, infiziert →
  `crystal`, fallendes Gebäude → was `explosion` in `data\buildings.json` sagt. Mündungsfeuer `flash_*` (Werte aus TABs
  `CShotEffect`). Höchstens 48 Bilder je Effekt. Etwa 20 MB.
- **TABs späte Gebäude** in **TABs Größe** (`TAB_BUILDINGS` in `tab_buildings.py`: `wood_house_1..10`, `power_plant`, `oil_platform`,
  `quarry`, `big_quarry`, `big_farm`, `big_mill`, `warehouse`, `engineering_center`, `shocking_tower`, `executor` –
  dreht sich wie die Ballista –, `radar`, `lookout_tower`, die Wunder `spire`, `academy`, `victorious`,
  `transmutator`, `crystal_palace`), Fallen (`blade_trap_1..4`, `mine_trap`) und `stone_gate` (noch keins). In `decor\` dieselbe
  Gebäude-JSON: `wheat`, `loot_*`, `treasure_ruin_<Landschaft>`, `power_tower_*`, `truck` (2 × 4: `site`), `tent_ruin_*`, `well_ruin` und die Bahn: `rail_x`/`rail_y` (ein Stück Gleis), `rail_end_n|e|s|w` (Prellbock), `rail_curve_ne|nw|se|sw` (Kurve, 6 × 6), `rail_arch_a|b|c` (Bögen einer Bahnbrücke, nicht quadratisch: `site` = [x, y] Kacheln, `size` = die x-Seite), `rail_barrier_a|b`, `rail_crossing`, `rail_trolley`, `mine_wagon_a|b`, `train_lamp`. In
  `effects\`: Feuer, Rauch, Infektion, Tesla- und Schockkugel, Schockstrahl, Wasser, Blätter, Regen und Schnee
  (`rain_*`, `snow_*`: TABs GIFs, addiert), Lichter, Reichweitenkreis (grün gedreht wie TAB), Rauchwolken, Lava;
  `projectiles\missile` (Thanatos). In `animals\`: `raven` (mit `take_off` und `fly`), `rat`. Noch ungenutzt:
  Zelt- und Brunnenruinen, Lava, Rauchwolken, `electric_sphere`/`electric_halo`.
- **Wann welcher bleibende Effekt** (`src\effects\lasting.rs`): Gebäude unter 50 % HP brennen und rauchen (`fire`,
  `smoke_b`, orangener Schein `light_white_a` – unsere Idee, TABs Gebäude brennen nur von Feuer), infizierte
  qualmen grün (`infection`, TABs `CInfected`), brennende Zombies tragen `fire`, Wasser schimmert (`water_a`), Schnee
  auf AL (`snow_b`, 320°), Regen auf VO (`rain_a`, 15°), Blätter fallen auf FA von jedem zehnten Baum (`leaf`, TABs
  `ZXMapThemeFA.AddLeave`). Der Reichweitenkreis liegt unter ausgewählten Türmen und beim Bauen.
- **Töne** (`tab_sounds.py`): TABs Ordner und Namen (Leerzeichen → `_`), `.mp3` als `.ogg` (macroquad spielt nur ogg
  und wav; braucht ffmpeg). Ohne Helden und Mutant (Kampagne und Züge sind drin). Etwa 190 MB. Das Spiel lädt jeden
  Ton erst, wenn er gebraucht wird (auf einem eigenen Thread) – Musik und Umgebung lässt es danach wieder los.
- **Leiste** (`tab_ui.py`): einzelne PNGs, keine JSON. Icons, Ereignisse und Mauszeiger heißen wie in TAB
  (`icons\IconAttack.png`), die **Porträts wie unsere Arten in Rust** (`portraits\Soldier.png`,
  `portraits\MachineGunTower.png`, `portraits\Walker.png`) – so findet das Spiel sie ohne Tabelle. Sanitäter und
  Speerträger bekommen die Farbe ihres Skins (silbern, Marine). Ein eigenes Bild unter demselben Namen ersetzt TABs.
- **Ändern ohne neu bauen:** Bild oder JSON speichern, im Spiel **Esc → Restart**.
- **Nicht in die Exe eingebaut** (wird später zu groß): `build.ps1` kopiert den Ordner nach `dist\assets\`.

## Ein Sprite = JSON + PNG

```json
{
  "image": "figure.png",
  "frame": [96, 128],
  "feet": [48, 108],
  "pixels_per_tile": 128,
  "directions": 16,
  "tint": true,
  "idle":   { "at": [0, 0],  "frames": 4, "fps": 3 },
  "walk":   { "at": [4, 0],  "frames": 6, "tiles": 0.6 },
  "attack": { "at": [10, 0], "frames": 4, "fps": 8 }
}
```

| Wert | Bedeutung |
|---|---|
| `image` | Das PNG, neben der JSON-Datei. Mehrere JSONs dürfen dasselbe Bild nehmen (`fat.json` = dieselbe Figur, größer) |
| `frame` | Breite und Höhe **eines** Einzelbilds in Pixeln |
| `feet` | Wo im Einzelbild die **Füße** sind (Pixel von oben links). Dieser Punkt steht auf der Position der Figur |
| `pixels_per_tile` | So viele Pixel sind eine Kachelbreite. Kleiner = größer gezeichnet. 128 = bei größtem Zoom scharf. Die Testfiguren stehen auf 256 (Fat 150): halb so groß, wie sie gemalt sind – so groß wie die Figuren auf den Türmen |
| `directions` | Richtungen pro Animation (8 oder 16). **Eine Zeile pro Richtung:** Zeile 0 schaut nach rechts unten (+x auf der Karte), dann im Uhrzeigersinn |
| `mirror` | `true` = wie TAB nur die Richtungen nach rechts (TAB: 10, je 18°) – nach links wird gespiegelt gezeichnet. Weglassen = jede Richtung hat ihre Zeile |
| `tint` | `true` = in der Farbe ihrer Art eingefärbt (für ein graues Testbild). Echte Bilder: weglassen |
| `idle` | Pflicht. Stehen |
| `walk`, `attack` | Optional (fehlt eine: `idle`). Gehen / Angreifen |
| `fight` | Optional: ein zweiter Angriff aus der Nähe (Thanatos schlägt mit dem Werfer zu), einmal pro Schlag |
| `take_off`, `fly` | Optional, nur der Rabe: Auffliegen (TABs „PrepareFly“, rückwärts: Landen) und Fliegen |
| `load` | Optional, nur Einheiten: **Waffe heben** (TABs „LoadAttack“). Bild 0 = Waffe unten, das letzte = oben – so steht sie zwischen zwei Schüssen. Senken: rückwärts. Fehlt es: das erste Bild von `attack`, sobald die Waffe oben ist |
| `run`, `run_from` | Optional: **Rennen**, ab `run_from` Kacheln pro Sekunde (TABs Zombies gehen beim Wandern und rennen beim Angriff). Fehlt es: immer `walk` |
| `special` | Optional: **Knurren** (TABs „Special“ – der Riese brüllt), einmal abgespielt, dann das letzte Bild gehalten. Fehlt es: `idle` |
| `die` | Optional: **Sterben** (TABs „Die“), einmal abgespielt, dann liegt die Figur im letzten Bild. Fehlt es: `idle` |
| `at` | Spalte und Zeile ihres ersten Bilds (Richtung 0), **in Einzelbildern** gezählt. Die Bilder gehen nach rechts, die Richtungen nach unten |
| `frames` | Wie viele Einzelbilder |
| `fps` | Bilder pro Sekunde (Spielzeit: steht bei Pause, schneller bei ×2) |
| `tiles` | Nur `walk` und `run`: so viele Kacheln geht die Figur in einem Durchlauf – schnelle Figuren treten schneller, die Füße rutschen nicht |

- **Zombie-Angriff:** genau **ein** Durchlauf pro Schlag (in der Pause bis zum nächsten), `fps` zählt da nicht.
  Einheiten: `attack` genau **einmal pro Schuss** (beginnt mit dem Ausholen, `fps` zählt), dazwischen das letzte Bild
  von `load`. Beide sollen also in derselben Haltung enden bzw. beginnen – bei TABs Bildern ist das so. Nur der
  Medic (Strahl) läuft im Kreis.
- **Kamera fürs Rendern** (Blender): orthografisch, 45° von oben (Rotation X = 45°), um 45° gedreht (Z = 45°).
  Dann passt die Figur genau auf die Rauten (√2 : 1, wie TABs).
- **Rand lassen:** ein paar durchsichtige Pixel um jede Figur, sonst färben beim Herauszoomen die Nachbarbilder ab.
- **Testbild neu malen:** `cargo run --release --example test_sprites` → `assets\test\figure.png`.

## Ein Gebäude = JSON + PNG

Von `tools\tab_buildings.py` gemacht (von Hand geht es auch). Alle Bilder eines Gebäudes liegen in einem PNG; jedes
weiß, wo seine **Füße** sind: die unterste Ecke seines Bauplatzes auf dem Bildschirm (wie TABs `BottomCenterToCellCorner`).

| Wert | Bedeutung |
|---|---|
| `image` | Das PNG, neben der JSON-Datei |
| `pixels_per_tile` | So viele Pixel sind eine Kachelbreite … |
| `size` | … bei einem Gebäude so viele Kacheln breit. Ist es im Spiel breiter oder schmaler (ein großes Dorfhaus), wird es entsprechend größer oder kleiner gezeichnet |
| `pictures` | Jedes Bild: `[x, y, Breite, Höhe, Füße x, Füße y]` in Pixeln – wo es im PNG liegt und wo darin die Füße sind |
| `body` | Das ganze Gebäude (beim Turm: sein Körper hinter den Einheiten) in drei Looks: `working`, `idle` (aus, infiziert, neutral), `building` (im Bau) – jeder eine Liste von **Teilen**, von hinten nach vorn gezeichnet |
| Teil: `frames` | Seine Bildnummern – eins: steht still, mehr: im Kreis mit `fps` |
| Teil: `add` | `true` = Licht: wird zum Bild darunter **addiert** (leuchtet) statt es zu verdecken |
| Teil: `turn_from`, `turn` | Ein drehendes Geschütz: die Bilder gehen einmal im Kreis, das erste schaut nach `turn_from` Grad (0 = rechts unten, 90 = links unten), jedes nächste `turn` Grad weiter. Gezeigt wird das zum Ziel |
| Teil: `shot`, `shot_ms` | Nur Ballista: je 18° (ab 0) die Bilder eines Schusses, einmal abgespielt in `shot_ms` |
| `front`, `roof` | Nur Türme: die Vorderwand vor den Einheiten, und wie hoch (Kachelbreiten über der Mitte) die Einheiten darauf stehen |
| `pieces` | Nur Mauern und Tor, je in denselben drei Looks: `pillar` (auf jeder Kachel), `left` (zum Nachbarn bei +y), `right` (+x), `horizontal` / `vertical` (schräg, wenn keine gerade Mauer die Ecke schließt), `pillar_straight` (Steinmauer in gerader Linie), `door_x` / `door_y` (die Tür des Tors, gedreht oder nicht – offen weg) |

## Ein Zug = JSON + PNG

Von `tools\tab_trains.py` gemacht, wie TAB ihn zeichnet (`TrainCarriage.cs`). Lok und Wagen: je 91 Bilder einmal im
Kreis, **nichts gespiegelt**; die Räder sind eigene Bilder.

| Wert | Bedeutung |
|---|---|
| `pictures` | Wie bei Gebäuden `[x, y, Breite, Höhe, Mitte x, Mitte y]` – hier die **Mitte** des Bilds (TAB setzt die Mitte, nicht die Füße) |
| `angles` | 91 Bildnummern: für Winkel Nummer n (= n × 360/91 Grad, 0 = rechts unten wie bei den Figuren, 90 = links unten) dieses Bild – TABs eigene Formel, zwei Nachbarn zeigen manchmal dasselbe. Den Rest bis zum genauen Winkel dreht TAB das Bild |
| `length`, `gap` | So viele Kacheln lang; so viel Abstand zum nächsten Wagen |
| `offset`, `height` | Die Bildmitte steht so viele Kacheln (x, y auf der Karte, **nicht** mitgedreht) neben der Wagenmitte und so viele Kachelbreiten höher |
| `shadow` | Dasselbe Bild schwarz: so deckend, so viele Kachelbreiten nach links, so viel größer |
| `wheels` | Wo die 6 Räder sitzen: Kacheln längs und quer, mit dem Wagen gedreht |

`wheel.json`: 91 Winkel × `frames` (20) Bilder (Winkel i: Bilder i × 20 …), gewählt mit der **Bildnummer** des Wagens;
das Rad dreht sich `per_tile_travelled` Bilder pro gefahrener Kachelbreite und sitzt `up` Kachelbreiten höher, hinter
dem Wagen.

## Ein Effekt = JSON + PNG

Von `tools\tab_effects.py` gemacht. Die Bilder nebeneinander, Zeile für Zeile.

| Wert | Bedeutung |
|---|---|
| `image`, `frame`, `columns`, `frames` | Das PNG; Breite und Höhe eines Bilds; so viele nebeneinander; so viele insgesamt |
| `feet` | Wo im Bild der Effekt steht – wie TAB: die untere Mitte auf der untersten Ecke eines Quadrats der Größe `size` um seinen Punkt |
| `pixels_per_tile` | So viele Pixel sind eine Kachelbreite – bei Größe 1. Das Spiel gibt die Größe vor (Treffer 0,4, Fass 5) |
| `duration`, `loop` | Sekunden Spielzeit für alle Bilder (0: ein stilles Bild); `true` = im Kreis |
| `vanish` | Ab diesem Anteil der Zeit blendet er aus (TABs `VanishFactorTime`) |
| `add`, `opacity` | Leuchtet (addiert); wie deckend |
| `turns` | Wird in seine Richtung gedreht: Mündungsfeuer (zeigt im Bild nach rechts), Pfeile (nach oben) |
| `squash`, `ahead`, `height` | Nur Mündungsfeuer: Höhe mal das; so weit vor dem Schützen, so hoch (Kachelbreiten) |
| `ground` | Ein Fleck am Boden (liegt flach, gedreht, wächst und verblasst) |
