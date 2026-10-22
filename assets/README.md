# Bilder (`assets\`)

Die Bilder der Zombies und Einheiten – vorgerenderte **Sprite-Sheets** wie in TAB (DESIGN.md 12.3). Gelesen von
`src\sprites.rs`.

- **Welche Art welches Bild hat**, steht in `data\`: `"sprite": "test/figure"` → `assets\test\figure.json`.
  Ohne `sprite` (oder wenn die Dateien fehlen/kaputt sind) bleibt sie ein Kästchen, oben links steht dann eine rote Zeile.
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
| `tint` | `true` = in der Farbe ihrer Art eingefärbt (für ein graues Testbild). Echte Bilder: weglassen |
| `idle` | Pflicht. Stehen |
| `walk`, `attack` | Optional (fehlt eine: `idle`). Laufen / Angreifen |
| `at` | Spalte und Zeile ihres ersten Bilds (Richtung 0), **in Einzelbildern** gezählt. Die Bilder gehen nach rechts, die Richtungen nach unten |
| `frames` | Wie viele Einzelbilder |
| `fps` | Bilder pro Sekunde (Spielzeit: steht bei Pause, schneller bei ×2) |
| `tiles` | Nur `walk`: so viele Kacheln geht die Figur in einem Durchlauf – schnelle Figuren treten schneller, die Füße rutschen nicht |

- **Zombie-Angriff:** genau **ein** Durchlauf pro Schlag (in der Pause bis zum nächsten), `fps` zählt da nicht.
  Einheiten: `attack` läuft im Kreis, solange sie zielt und schießt.
- **Kamera fürs Rendern** (Blender): orthografisch, 30° von oben (Rotation X = 60°), um 45° gedreht (Z = 45°).
  Dann passt die Figur genau auf die Rauten (2 : 1).
- **Rand lassen:** ein paar durchsichtige Pixel um jede Figur, sonst färben beim Herauszoomen die Nachbarbilder ab.
- **Testbild neu malen:** `cargo run --release --example test_sprites` → `assets\test\figure.png`.
