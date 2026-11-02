# Bilder (`assets\`)

Alle Bilder des Spiels – vorgerenderte **Sprite-Sheets** wie in TAB (DESIGN.md 12.3), je Art ein Ordner:

| Ordner | Was | Gemacht von | Gelesen von |
|---|---|---|---|
| `units\` | Unsere Einheiten (Ranger, Soldat, Scharfschütze) | `tools\tab_skins.py` | `src\sprites.rs` |
| `zombies\` | Die Zombies (15 Skins mit den Einheiten zusammen) | `tools\tab_skins.py` | `src\sprites.rs` |
| `buildings\` | Gebäude, Mauerstücke, Dorfhäuser, Fässer | `tools\tab_buildings.py` | `src\building_sprites.rs` |
| `terrain\<BR\|TM\|AL\|DS\|FA\|VO>\` | Boden, Wasser, Bäume, Felsen, Kräuter, Erz je Landschaft | `tools\tab_terrain.py` | `src\ground.rs` |
| `effects\` | Blut, Mündungsfeuer, Explosionen, Trümmer, Flecken am Boden | `tools\tab_effects.py` | `src\effects.rs` |
| `projectiles\` | Pfeil und Ballista-Bolzen | `tools\tab_effects.py` | `src\effects.rs` |
| `test\` | Unser graues Testbild (in git) | `examples\test_sprites.rs` | `src\sprites.rs` |

- **TABs Bilder** (alle Ordner außer `test\`): urheberrechtlich geschützt – nur auf diesem PC, **nie in git**
  (`.gitignore`). Die Skripte machen sie aus deinem entpackten TAB und seiner `ZXGame.dxprj` neu. Fehlt ein Bild, wird es
  übersprungen (Kästchen, Klötze, Rauten, Linien wie ohne TAB).
- **Welche Art welches Aussehen hat**, steht in `data\`: `"skins": ["zombies/walker_aged", …]` →
  `assets\zombies\walker_aged.json`. Jede Figur trägt eins davon ihr Leben lang (Einheiten nach ihrer Nummer, Zombies
  nach ihrem `look`, auch im Schlaf). Sind alle weg, nimmt sie `"sprite"` (das graue `test/figure`). Ohne beides (oder
  wenn die Dateien kaputt sind) bleibt sie ein Kästchen, oben links steht dann eine rote Zeile.
- **Figuren** (`tab_skins.py`): zugeschnitten, verkleinert (80 Pixel pro Kachel), eingefärbt wie TAB, höchstens 16
  Bilder je Animation (Stehen, Knurren, Sterben 12), die Geschwindigkeiten aus TABs `TimeAni…`. Etwa 290 MB
  Grafikspeicher.
- **Landschaften** (`tab_terrain.py`): je Landschaft `earth.png`, `grass.png`, `water.png` (wiederholen sich alle
  `tiles` Kacheln), `objects.png` (Bäume, Felsstücke, Kräuter, Erz, Kohle, Ölquelle) und `terrain.json`. Welche eine
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
| `run`, `run_from` | Optional: **Rennen**, ab `run_from` Kacheln pro Sekunde (TABs Zombies gehen beim Wandern und rennen beim Angriff). Fehlt es: immer `walk` |
| `special` | Optional: **Knurren** (TABs „Special“ – der Riese brüllt), einmal abgespielt, dann das letzte Bild gehalten. Fehlt es: `idle` |
| `die` | Optional: **Sterben** (TABs „Die“), einmal abgespielt, dann liegt die Figur im letzten Bild. Fehlt es: `idle` |
| `at` | Spalte und Zeile ihres ersten Bilds (Richtung 0), **in Einzelbildern** gezählt. Die Bilder gehen nach rechts, die Richtungen nach unten |
| `frames` | Wie viele Einzelbilder |
| `fps` | Bilder pro Sekunde (Spielzeit: steht bei Pause, schneller bei ×2) |
| `tiles` | Nur `walk` und `run`: so viele Kacheln geht die Figur in einem Durchlauf – schnelle Figuren treten schneller, die Füße rutschen nicht |

- **Zombie-Angriff:** genau **ein** Durchlauf pro Schlag (in der Pause bis zum nächsten), `fps` zählt da nicht.
  Einheiten: `attack` läuft im Kreis, solange sie zielt und schießt.
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
