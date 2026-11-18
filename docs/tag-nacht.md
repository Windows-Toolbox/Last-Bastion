# Tag/Nacht als Tönung: Himmel, Zyklus, Blutmond per Skript

**Ziel:** Die Szene bekommt je nach Tageszeit einen Farbton und eine Helligkeit, nachts bleiben nur die Lichtkreise um
Einheiten und Gebäude hell. Skripte können den Himmel jederzeit umstellen (Blutmond). **Keine Sonne, keine wandernden
Schatten** – die eingebackenen Schatten bleiben, wie sie sind.

Jeder Schritt ist ein Startpunkt für eine neue Unterhaltung („Lies docs/tag-nacht.md, Schritt N“).

## Die Idee in drei Sätzen

- Ein **Himmel** (`SkyLook`) sind fünf Zahlen: Farbton, Sättigung, Kontrast, Helligkeit – dieselben vier wie der
  Farbfilter der Landschaft (`Grade` in `src\post.rs`) – und **`ambient`**: wie hell die Karte abseits von Lichtern ist.
- **Farbe** läuft über den Filter, den es schon gibt: `Post::finish` schickt eine Farbmatrix an den Shader. Die Matrix
  des Himmels wird mit der der Landschaft multipliziert – **kein neuer Shader**.
- **Dunkelheit mit Lichtern** läuft über den Nebel, den es schon gibt: `FogPicture::paint` (`src\draw.rs`) dunkelt jede
  Kachel auf `max(Licht der Einheiten/Gebäude, ambient)` ab. Nachts wird nur `ambient` kleiner – die Lichtkreise
  (TABs `CMapWatcher`) bleiben hell und leuchten durch den Bloom von selbst.

## Für jeden Schritt

- README → „Regeln für Claude“. Am Ende `build.ps1` laufen lassen.
- **Der Himmel ist nur Bild**, wie das Wetter (`src\sim\weather.rs`): die Simulation fragt ihn nie, er steht nicht in
  `checksum`. Er hängt aber am **Takt** (`World::tick`), nicht an der echten Zeit – so steht er in der Pause still und
  ist in Koop bei allen gleich.
- **Der Himmel „Day“ ändert nichts** (weiß, 1, 1, 1, `ambient` 1). Karten ohne Zyklus und ohne Skript sehen genau aus
  wie heute – das ist in jedem Schritt zu prüfen.
- Werte nach `data\`, nicht in den Code. `data\README.md`, README („Prototyp ausprobieren“, Tabelle „Dateien“) und
  `DESIGN.md` (15, Parkplatz „Day/night cycle“: der Bild-Teil ist dann gebaut, „mehr Zombies nachts“ bleibt dort) an Ort
  und Stelle nachziehen.
- Prüfen: `cargo test --release`, `cargo clippy --release --all-targets`, Spiel und Editor ansehen (Skill
  `run-last-bastion`).

## Schritte

| # | Was | Status |
|---|---|---|
| 1 | Die Uhr: Stunde 0 ist Mitternacht, Startzeitpunkt je Karte (neu: Tag 0, 8 Uhr), Events auf „Tag X, Stunde Y“ | offen |
| 2 | Himmel als Werte: `data\sky.json`, `SkyLook`, Mischen | offen |
| 3 | Zustand in der Sim: Ziel, Überblendung, `World::sky_now` | offen |
| 4 | Aktion „Sky“ im Trigger-Editor, Kartenformat hoch | offen |
| 5 | Zeichnen: Farbmatrix und Nebel-Licht | offen |
| 6 | Tag/Nacht-Zyklus als Karten-Einstellung (Seite „Map“) | offen |
| 7 | Vorschau im Editor (Himmel und Uhrzeit wählen, ohne dass Zeit läuft) | offen |

### Schritt 1 – Die Uhr und der Startzeitpunkt

**Entschieden:** Es gibt **eine Uhr**, und alles richtet sich nach ihr. Stunde 0 ist Mitternacht, die Tage zählen
ab 0. Jede Karte hat einen **Startzeitpunkt** (Tag, Stunde), im Editor einstellbar; neue Karten starten an
**Tag 0, 8 Uhr**. Ein Event auf Stunde 24 kommt um Mitternacht (Tag 1, 0 Uhr) – bei Start um 8 also nach 16 Stunden.
Ein Event **vor oder genau auf** dem Startzeitpunkt läuft **sofort zu Beginn der Runde**; der Editor weist darauf hin.
Geld gibt es zum Start keins.

- **Der Startzeitpunkt:** `World::start_hour: u32` (Tag × 24 + Stunde), gesetzt über `Order::SetStartHour { hour }`
  (wie `Order::SetDays`, `src\sim\orders.rs`), gespeichert in `RoundData` (`src\mapfile.rs`).
- `src\sim\mod.rs`: `World::clock_ticks(&self) -> u64` = `tick + start_hour * HOUR_TICKS`. `day_and_hour` (Zeile ~783)
  rechnet damit und zählt die Tage ab 0: bei Start um 8 ist Takt 0 → `(0, 8)`, 16 Stunden später `(1, 0)`.
- **Jede Stelle mit `HOUR_TICKS` / `DAY_TICKS` auf die Uhr umstellen** (`clock_ticks()` statt `tick`):
  - `src\sim\funcs.rs`: `Day()` (ab 0), `Hour()`, `TotalHours()` = `Day * 24 + Hour` – die Hilfetexte anpassen.
  - `src\sim\events.rs`: `Event::hour` ist die Stunde **der Uhr** (Tag × 24 + Stunde). Ein Event kommt bei Takt
    `hour.saturating_sub(start_hour) * HOUR_TICKS` – was davor liegt, also bei Takt 0. Die Ankündigung
    (`next_announced`, `Notice::Coming { at }`) rechnet genauso. Wiederholungen zählen von der eingestellten Stunde
    weiter; die, die vor dem Start lägen, fallen nicht alle auf einmal an: nur einmal sofort, dann im Takt weiter.
  - `src\sim\watch.rs`: `TriggerEvent::Clock { day, hour }` mit Tag ab 0, feuert zur vollen Stunde der Uhr – vor oder
    auf dem Startzeitpunkt ebenfalls sofort bei Takt 0 (nach `TriggerEvent::Start`).
  - `src\sim\round.rs`, `set_days`: die Runde endet an Tag `days`, 0 Uhr (`days * 24` auf der Uhr) – bei Start um 8
    ist sie 8 Stunden kürzer als heute.
  - `src\sim\doom.rs` (Wachen je Stunde, Überfälle je Tag), `src\sim\sleep.rs`, `src\game.rs` (`swarm_coming`),
    `src\home.rs` (Minuten einer Runde: die Stunden bis zum Start abziehen).
  - Die Anzeige in `src\hud.rs`: „Day 0, 08:00“ beim Start.
- **Bleibt am Takt:** das Geld (`payout_ticks` in `src\sim\buildings.rs` – erste Ausschüttung nach einer vollen
  Periode, egal wie spät es ist), „Every N seconds“, `Wait`.
- **Editor:**
  - `src\editor\map_page.rs`, Abschnitt „Round“ bei „Days“: **„Starts on day ‹0›, hour ‹8›“**.
  - `src\editor\events.rs` und der `Clock`-Satz in `src\story_panel\sentences.rs`: Events werden als
    **„Day ‹X›, hour ‹Y›“** eingegeben und gezeigt (gespeichert bleibt eine Zahl, `hour = X * 24 + Y`); Wiederholungen
    bleiben eine Dauer in Stunden.
  - Liegt ein Event oder `Clock`-Trigger vor dem Startzeitpunkt: gelber Hinweis daneben, **„before the round starts –
    runs at once when it begins“**. Auch in der Zeitleiste der Events, falls sie eine hat.
- **Alte Karten** (`VERSION` hoch): eine Datei ohne Startzeitpunkt liest sich als **Tag 0, 0 Uhr** – damit kommt in ihr
  alles zum selben Zeitpunkt wie bisher. Nur neue Karten (Editor „New map“, „New game“) bekommen 8 Uhr. Ein
  `Clock`-Trigger zählte die Tage ab 1: beim Lesen einer älteren Datei `day - 1`. Mit Test. `data\` nach Stunden
  durchsehen (Events der Schwierigkeiten und Spieldauern, `doom.json`): was auf Stunde 0–8 liegt, käme in neuen Runden
  sofort – verschieben oder in der Zusammenfassung nennen.
- **Tests** (`src\sim\tests.rs`): bei Start um 8 ist `day_and_hour` bei Takt 0 `(0, 8)`, nach 16 Stunden `(1, 0)`; der
  Test bei Zeile ~172 und die Helfer bei Zeile ~94–160 (sie rechnen `hour * HOUR_TICKS`) anpassen; ein Event „hour 24“
  kommt nach 16 Stunden; ein Event „hour 3“ kommt im ersten Takt, genau einmal; ein `Clock`-Trigger „day 0, hour 9“
  feuert nach einer Stunde; mit Start „day 2, hour 20“ zeigt die Uhr das bei Takt 0; in den ersten Takten gibt es kein
  Geld; eine Runde über 3 Tage ist an Tag 3, 0 Uhr gewonnen; eine alte Datei startet um 0 Uhr und ihr Event „hour 24“
  kommt nach 24 Stunden.

### Schritt 2 – Himmel als Werte

- **`data\sky.json`** (neu; in `src\data.rs` in `FILES`, `BUILT_IN` und `GameData` eintragen, an derselben Stelle):
  ```json
  {
    "skies": [
      { "id": "Day",        "hue": [255, 255, 255], "saturation": 1.0,  "contrast": 1.0,  "brightness": 1.0,  "ambient": 1.0 },
      { "id": "Dusk",       "hue": [255, 190, 140], "saturation": 0.95, "contrast": 1.05, "brightness": 0.97, "ambient": 0.8 },
      { "id": "Night",      "hue": [150, 170, 255], "saturation": 0.7,  "contrast": 1.05, "brightness": 0.93, "ambient": 0.45 },
      { "id": "Dawn",       "hue": [255, 210, 190], "saturation": 0.9,  "contrast": 1.0,  "brightness": 0.98, "ambient": 0.8 },
      { "id": "Blood Moon", "hue": [255, 105, 90],  "saturation": 0.9,  "contrast": 1.15, "brightness": 0.95, "ambient": 0.55 }
    ],
    "cycle": [
      { "hour": 0,  "sky": "Night" },
      { "hour": 5,  "sky": "Night" },
      { "hour": 6,  "sky": "Dawn" },
      { "hour": 8,  "sky": "Day" },
      { "hour": 18, "sky": "Day" },
      { "hour": 20, "sky": "Dusk" },
      { "hour": 21, "sky": "Night" }
    ]
  }
  ```
  Die Zahlen sind Startwerte – in Schritt 5 nach Auge einstellen. `cycle`: zwischen zwei Stützpunkten wird gemischt,
  nach dem letzten zurück zum ersten (Stunde 24 = 0). Zwei Stützpunkte mit demselben Himmel = er bleibt so lange.
  **Stunde 0 ist Mitternacht**, neue Karten starten um 8 (Schritt 1) – also bei vollem Tag.
- **`src\sim\sky.rs`** (neu, neben `weather.rs`; in `sim\mod.rs` anmelden und `pub use`):
  - `pub struct SkyLook { hue: [f32; 3] (0..1), saturation, contrast, brightness, ambient }` – `Clone, Copy, PartialEq`.
  - `SkyLook::DAY` (der neutrale) und `fn mix(self, to: SkyLook, t: f32) -> SkyLook` (jedes Feld linear).
  - In `data.rs`: `SkyData { skies: Vec<SkyEntry>, cycle: Vec<CycleStop> }` mit `fn look(&self, id: &str) -> SkyLook`
    (unbekannt → `DAY`), `fn ids()` für den Editor, `fn at_hour(&self, hour: f32) -> SkyLook` (der Zyklus, `hour` in
    0..24) und `check()` wie `research.check()`: „Day“ gibt es, jeder Himmel des Zyklus existiert, Stunden steigen.
- **Tests:** `mix` bei 0 / 1 / 0,5; `at_hour` genau auf einem Stützpunkt, dazwischen (7 liegt zwischen „Dawn“ und „Day“), und über Mitternacht (23 ist
  „Night“); „Day“ aus der Datei ist `SkyLook::DAY`.
- Zu sehen ist nach diesem Schritt nichts.

### Schritt 3 – Zustand in der Sim

- In `sky.rs`:
  ```rust
  pub enum SkyTarget { Own, Named(String) }      // Own = der Himmel der Karte (Zyklus, sonst "Day")
  pub struct SkyState {
      pub cycle_days: u32,   // ein Tag/Nacht-Lauf dauert so viele Spieltage; 0 = kein Zyklus
      from: SkyLook,         // was zu sehen war, als das Ziel gesetzt wurde
      target: SkyTarget,
      since: u64,            // Takt, an dem es gesetzt wurde
      fade: u32,             // Takte bis ganz da; 0 = sofort
  }
  ```
  `World::sky: SkyState` (`sim\mod.rs`, neben `weather`), Anfang: `cycle_days` 0, `Own`, `from` = `DAY`, `fade` 0.
- `impl World`:
  - `fn own_sky(&self, alpha: f32) -> SkyLook`: bei `cycle_days == 0` → „Day“; sonst mit
    `lauf = cycle_days * DAY_TICKS` und `t = tick + start_hour * HOUR_TICKS * cycle_days`:
    `hour = ((t % lauf) as f32 + alpha) / lauf as f32 * 24.0` → `at_hour`. Bei Takt 0 ist das immer die Stunde des Startzeitpunkts – mit
    `cycle_days = 1` läuft der Himmel genau mit der Uhr der Leiste, mit mehr Tagen entsprechend langsamer.
  - `pub fn sky_now(&self, alpha: f32) -> SkyLook`: Ziel = `own_sky` oder `look(id)`;
    `t = ((tick - since) as f32 + alpha) / fade` (auf 0..1 begrenzt, `fade == 0` → 1), weich gemacht mit
    `t * t * (3 - 2t)`; Ergebnis `from.mix(ziel, t)`. `alpha` = der Anteil zwischen zwei Takten, den `draw_world` schon
    bekommt.
  - `pub(super) fn set_sky(&mut self, target: SkyTarget, fade_ticks: u32)`: **zuerst** `from = self.sky_now(0.0)`, dann
    Ziel, `since = tick`, `fade` setzen. So blendet auch ein Wechsel mitten in einer Überblendung ohne Sprung.
- Beim Neustart einer Runde (und „Test“ im Editor) geht der Zustand zurück auf den Anfang, `cycle_days` bleibt –
  nachsehen, wo `weather` dabei zurückgesetzt wird (`set_look`, `start_story` in `sim\story.rs`), und es genauso machen.
- **Tests** (`sim\tests.rs`): ohne alles ist `sky_now` = `DAY`; nach `set_sky(Named("Night"), 100)` ist es bei Takt +0
  noch `DAY`, bei +50 dazwischen, bei +100 „Night“; ein zweites `set_sky` bei +50 springt nicht (Wert davor = Wert
  danach); mit Start um 8 und `cycle_days = 1` ist es bei Takt 0 „Day“ und 14 Stunden später (22 Uhr) „Night“, mit
  `cycle_days = 2` erst nach 28 Stunden.

### Schritt 4 – Aktion „Sky“

Vorbild ist `Action::Weather`: einmal alle Stellen dazu suchen (`Weather` in `src\sim\story.rs`,
`src\story_panel\sentences.rs`, `src\sim\tests.rs`) und daneben den Himmel eintragen.

- `src\sim\story.rs`: `Action::Sky { sky: String, seconds: u32 }` – `sky` leer = zurück zum Himmel der Karte
  (`SkyTarget::Own`), sonst die `id` aus `sky.json`. In `act`:
  `self.set_sky(ziel, (seconds as f32 * TICKS_PER_SECOND) as u32)`.
- `src\story_panel\sentences.rs`: in `GROUPS` unter „Show“ nach „Weather“; `new_action` („Blood Moon“, 10 Sekunden);
  `action_help`; der Satz **„Sky becomes ‹Blood Moon› over ‹10› seconds“** – Wort 0 ein Menü aus „The map's own“ und
  `data::get().sky.ids()`, Wort 1 eine Zahl (wie bei „Noise“ die Lautstärke). Ein Name, den `sky.json` nicht (mehr)
  kennt, wird rot gezeigt und im Spiel wie „Day“ behandelt.
- `src\mapfile.rs`: `VERSION` hoch (falls Schritt 1 sie nicht in derselben Unterhaltung schon erhöht hat) mit einer Zeile im Kommentar (ein älteres Spiel kann die Aktion nicht lesen); Test, dass
  eine Datei mit der Aktion gespeichert und geladen dieselbe ist.
- **Test:** ein Start-Trigger mit `Sky` stellt `world.sky` um (wie
  `the_weather_comes_with_the_landscape_and_scripts_change_it`).
- `docs\trigger-und-28yl.md` („Was es schon gibt“ → Aktionen) und die README-Tabelle „Editor“ um „Sky“ ergänzen.

### Schritt 5 – Zeichnen

- **Farbe – `src\post.rs`:**
  - `Grade` bekommt `pub fn of_sky(sky: &SkyLook) -> Grade` (Farbton zurück nach 0..255 oder `hue` auf `[f32; 3]`
    umstellen – was weniger ändert).
  - `fn times(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4]`: das gewöhnliche Matrixprodukt `a · b`. Der Shader
    rechnet `color * Grade` mit der Farbe als Zeile, also ist „erst Landschaft, dann Himmel“ =
    `times(grade(look).matrix(), Grade::of_sky(sky).matrix())`. Die Helligkeit steckt in der Alpha-Zeile und geht im
    Produkt richtig mit – nichts am Shader ändern.
  - `Post::finish(&mut self, look: &str, sky: &SkyLook, unlit: bool)`.
  - **Tests:** `times(m, neutral) == m`; mit „Night“ ist die Blau-Diagonale größer als die Rot-Diagonale.
- **Dunkelheit – `src\draw.rs`, `FogPicture::paint`:** ein Parameter `sky_ambient: f32` dazu; das `ambient` dort
  (`EXPLORED_LIGHT`, `EXPLORED_LIGHT_BR`, bei `world.revealed` 1,0) wird damit **multipliziert**. Sonst ändert sich an
  `fog_cover` und `shine` nichts. Nie Gesehenes bleibt schwarz.
- **`src\game.rs`** (um Zeile 800–830): einmal `let sky = world.sky_now(alpha);`, an `fog_picture.paint(…, sky.ambient)`
  und `post.finish(&world.look, &sky, …)` geben.
- **Was sich nicht ändert:** Leiste, Minimap, Lebensbalken, Mauszeiger – sie werden nach `finish` gezeichnet.
- **Bekannte Grenzen (so lassen, in den Kommentar schreiben):**
  - Editor-Sicht „All“ hat keinen Nebel: dort ist nur die Farbe zu sehen, nicht die Dunkelheit.
  - Kann die Grafikkarte die Shader nicht (`Post::broken`), gibt es nur die Dunkelheit, keine Farbe.
- **Ansehen:** eine Karte mit Start-Trigger „Sky becomes Night over 10 seconds“, eine mit „Blood Moon“. Darauf achten:
  Lichtkreise um Einheiten und Gebäude klar zu sehen, Zombies im Dunkeln noch zu erkennen, die Leiste unverändert. Dann
  die Zahlen in `sky.json` nachstellen. Jede der sechs Landschaften einmal bei Nacht ansehen – ihr eigener Filter
  (`grade`) wird mit dem Himmel multipliziert, VO (türkis) und AL (blau, Kontrast 1,3) kippen am ehesten.

### Schritt 6 – Tag/Nacht-Zyklus als Karten-Einstellung

- **Haken:** ein Spieltag dauert 60 Sekunden (`DAY_TICKS`). Ein voller Tag/Nacht-Lauf pro Minute flackert. Deshalb
  `cycle_days`: ein Lauf dauert so viele Spieltage (Vorschlag im Editor: 4 = alle 4 Minuten eine Nacht). Mit 1 stimmt
  der Himmel mit der Uhr der Leiste überein, mit mehr nicht mehr – der Editor sagt das neben dem Feld. **0 = aus** ist
  der Standard, auch für alle alten Karten und für die Karten aus „New game“.
- `src\sim\orders.rs`: `Order::SetSkyCycle { days: u32 }` wie `Order::SetDays` (der Editor ändert die Welt nur über
  Orders, sonst geht Undo nicht).
- `src\mapfile.rs`: Feld `sky_cycle: u32` in `MapFile` (`#[serde(default, skip_serializing_if = …)]`, bei 0 nicht
  geschrieben), in der Schlüsselliste der lesbaren Datei (bei `"look"`) eintragen, beim Schreiben aus und beim Lesen in
  `world.sky.cycle_days`. Ein älteres Spiel überspringt das Feld – dafür braucht es keine neue `VERSION`. Test:
  gespeichert und geladen bleibt der Wert.
- `src\editor\map_page.rs`, `world_column`, Abschnitt „World“ unter „Landscape“: **„Day and night“** – ein Haken „on“
  und ein `DragValue` „one cycle lasts ‹4› days“ (1 bis `MAX_DAYS`), darunter klein und grau, was das in Minuten ist.
  Den Kopfkommentar der Datei anpassen („Left out … weather and music per map“ stimmt dann nur noch für Musik und
  Wetter).
- **Test:** `SetSkyCycle` setzt den Wert; `own_sky` folgt ihm (aus Schritt 3).

### Schritt 7 – Vorschau im Editor

Im Modus „Edit“ läuft keine Zeit, der Takt bleibt 0 – ohne Vorschau sähe man im Editor nie eine Nacht.

- `src\editor.rs`: `pub enum SkyPreview { Game, Named(String), Hour(f32) }` als Feld des Editors (Anfang `Game`), und
  `pub fn sky_preview(&self, world: &World, alpha: f32) -> SkyLook`: `Game` → `world.sky_now(alpha)`, `Named` → `look`,
  `Hour` → `at_hour`. Nur solange `editing()`; in „Test“ und im Spiel immer `world.sky_now`.
- `floating_bars` (dort, wo „Sight“ und „Grid“ schweben): ein dritter Balken **„Sky“** – ein Menü mit „As in game“,
  jedem Himmel aus `sky.json` und „Hour …“; bei „Hour“ daneben ein Regler 0–24. So lässt sich der ganze Tag
  durchschieben und der Blutmond auf der eigenen Karte ansehen, ohne sie zu starten.
- `src\game.rs`: statt `world.sky_now(alpha)` den Wert der Vorschau nehmen.
- Hinweis im Tooltip: die Dunkelheit zeigt nur die Sicht „Player“ (Schritt 5, Grenzen).
- Die Vorschau wird nicht gespeichert und steht nicht im Undo.

## Blutmond im Editor – so sieht es am Ende aus

Modul „Triggers“, ein Trigger:

- **Ereignis:** „Clock: day 10, hour 22“ (oder „Event comes: ‹die letzte Welle›“).
- **Aktionen:**
  1. `Sky becomes Blood Moon over 10 seconds`
  2. `Sound: Tension`, `Notify: "The blood moon rises." (Red)`
  3. `Zombies: 300 Runner, storm`
  4. `Wait 120`
  5. `Sky becomes The map's own over 20 seconds`

Mit „Every 600 seconds“ als Ereignis kommt er regelmäßig, mit „Random“ davor manchmal. Der Zyklus der Karte läuft
darunter weiter: nach der fünften Aktion ist es so hell oder dunkel, wie es zu der Stunde gerade wäre.

## Vorschläge (noch nicht entschieden)

- **Eigene Himmel je Karte:** heute nur die aus `sky.json`. Ein Abschnitt „Skies“ auf der Seite „Map“ mit fünf Reglern
  und Farbwähler, gespeichert in der Karten-Datei – dann baut sich jeder seinen eigenen Mond.
- **Fenster und Lampen nur nachts:** die Licht-Teile der Gebäude (`building_sprites.rs`, `glow`) werden immer gleich
  hell addiert. Ein sechster Wert `lights` im Himmel könnte sie tagsüber schwächer und nachts stärker machen.
- **Abfragen im Skript:** `Sky()` (Name des Ziels) und `IsNight()` in `src\sim\funcs.rs` – aus Takt und Ziel gerechnet,
  nicht aus den gemischten Kommazahlen, damit es deterministisch bleibt. Erst damit ginge „mehr Zombies nachts“
  (`DESIGN.md` 15).
- **Ton:** nachts ein anderes Umgebungsgeräusch (`audio.json`, `ambient` je Landschaft).
- **Wandernde Schatten** (Punkt 2 und 3 der ursprünglichen Liste): bewusst nicht Teil dieses Plans.
