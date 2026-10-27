# TAB-Nachbau – die Schritte

Ziel: Last Bastion soll sich **genau wie They Are Billions (TAB)** anfühlen. Erst TAB 1:1 nachbauen, dann
weiterentwickeln. Grundlage ist TABs dekompilierter Code; jeder Schritt unten ist so geschrieben, dass eine neue
Unterhaltung damit starten kann.

**So startest du eine Unterhaltung:** „Lies `docs/tab-nachbau.md`, zuerst ‚Für jeden Schritt', dann ‚Schritt N', und
setz ihn um."

---

## Für jeden Schritt

- **Arbeitsregeln:** README → „Regeln für Claude" (Code Stück für Stück erklären, mit Vergleich zu Python; am Ende kurze
  Zusammenfassung; `DESIGN.md`, README „Prototyp ausprobieren" und `data\README.md` an Ort und Stelle nachziehen, keine
  datierten Einträge). Nie `git stash`/`checkout` auf dem Arbeitsbaum – zum Vergleichen mit dem alten Stand einen
  `git worktree` im Scratchpad anlegen und danach wieder entfernen.
- **TAB-Quellen:**
  - Dekompilierter Code: `C:\Users\schme\TheyAreBillions_Decompiled\` – `TheyAreBillions\` (Spiellogik, Namen von
    Eazfuscator verschleiert, Strings lesbar), `DXVision_Engine\` (Engine, Namen lesbar).
  - Spielordner: `C:\SteamLibrary\steamapps\common\They Are Billions\`. `ZXGame.dxprj` ist ein ZIP mit XML (Vorlagen:
    Größe, Grafik – keine Spielwerte).
  - `ZXCampaign.dat` und `ZXCampaignStrings.dat` lassen sich öffnen: Passwort wie `ZipSerializer.ReadCrypted`
    (`DXVision_Engine\DXVision.Serialization\ZipSerializer.cs`) mit dem Hash aus `DXHelper_HashCode.From`
    (der alte 32-Bit-.NET-String-Hash).
  - **`ZXRules.dat` (alle Werte: Tempo, Sicht, Leben, Wachsamkeit, Landschaften …) bleibt zu** – ihr Lader steckt in
    Eazfuscators virtueller Maschine, die knacken wir nicht. Werte daher: unsere vorhandenen, sonst schätzen und über
    den Balancing-Lauf einstellen.
- **TABs Takt umrechnen:** TAB-Logik läuft mit 30 Ticks/s, Physik mit 12 Schritten/s. `IsIAFrameModule(n)` (in
  `DXEntity.cs`) heißt „alle n/2 Ticks" = **alle n/60 Sekunden** (z. B. 240 → 4 s, 60 → 1 s, 720 → 12 s). Bei uns:
  20 Ticks/s (`TICK` 50 ms). 1 TAB-Tag = 60 s Spielzeit.
- **Namen der TAB-Zombies** (aus `ZXGame.dxprj`, Größe = Durchmesser in Kacheln): WeakA/B/C (schwach, 0,4),
  WorkerA/B und MediumA/B (mittel, 0,4), DressedA (stark, 0,4), StrongA = **Chubby** (0,7), Harpy (0,6), Venom (0,5),
  Giant (1,6), Mutant (haben wir nicht). Unsere Arten (**bestätigt** in Schritt 2): Weak → Walker, Worker/Medium →
  Runner, DressedA → Armored (Riot), StrongA → Fat, Venom → Spitter, Harpy → Harpy, Giant → Giant. Infested (SC2) gibt
  es in TAB nicht. Ränge (`level`, geschätzt): Walker 1, Runner 2, Riot/Harpyie 3, Spitter/Infested 4, Fat 5, Riese 6.
- **Determinismus** (DESIGN.md 12.8): Zufall nur aus `World::rng`, feste Reihenfolge, keine `HashMap` iterieren.
  Ändert sich die Simulation absichtlich, die Prüfsumme in `src/sim/tests.rs` (`SCRIPTED_GAME_CHECKSUM`) neu eintragen.
- **Prüfen:** `cargo test --release`, `cargo clippy --release --all-targets` (keine neuen Warnungen in geänderten
  Dateien).
- **Gedächtnis:** `tab-adoption-plan` (Reihenfolge, Status), `gap-search-removed`, `coal-later`, `tab-sources`.

**Schritt 1 (Lärm nach TAB-Formel) ist fertig** – `src/sim/sound.rs`.

**Schritt 2 (Entscheidungsbaum) ist fertig** – `src/sim/zombies.rs` (`watch`, `wall_in_way`, `zombie_hit_by`,
`update_minds`, `joins_wave`, `next_idle`, `roam_spot`, `zombie_intent`), Schlafende in `src/sim/sleep.rs`
(`idle_wait`, `wake_joining`), Infektion in `src/sim/buildings.rs`. Was er für die nächsten Schritte hinterlässt:
- `keep_out` der Schwierigkeit gilt nur noch beim Verteilen zu Beginn (`scatter_roamers`, `place_giants`) – für
  Schritt 6. Der Riese hält beim Wandern 54 Kacheln (`roam_ring`).
- Häuser des Untergangs schicken ihre Zombies weiter mit `Lure::seen`/`Lure::sent` (Schritt 4 baut das um).
- Geschätzt, weil `ZXRules.dat` zu ist: `watch_seconds` 0,5, `stand_seconds`/`growl_seconds` 2, `stroll`-Minima,
  `level`, `extra_infected` (Zelt 2, Steinhaus 8 – so bleibt es bei 4 bzw. 16 Infizierten).

**Schritt 3 (flüssiges Gedränge) ist fertig** – `src/sim/crowd.rs` (`move_crowd`, `neighbours`, `push_out`,
`out_of_blocked`), aufgerufen am Ende von `move_zombies`; Felsen der Einheiten in `src/sim/units.rs` (`still`,
`is_rock`, `shove`). Was er hinterlässt:
- Geschätzt: `mass` je Art in `data\zombies.json` (Walker 1 … Fat 3,25, Riese 25), Einheiten 1 (`UNIT_MASS`).
  8 Runden × `RELAX` 0,8 pro Tick; ein Zombie behält 30 % seines letzten Schritts (`TURN`, gegen Zucken an Ecken).
- Unsere Größen sind kleiner als TABs (Walker 0,25 statt 0,4 Durchmesser, Riese 0,9 statt 1,6) – nicht angefasst.
- TABs Schrittrhythmus (`BellWalkingFactor`) fehlt – erst mit echten Lauf-Animationen, und dann nur im Bild.
- TABs „angekommen, wenn 1 s lang nicht vorangekommen (innerhalb 2 Kacheln)“ / „aufgeben nach 5 s“ fehlen noch –
  unsere Ziele haben eigene Geduld (`LURE_PATIENCE`, Leerlauf-Timer).

**Schritt 6 (TABs Kartengenerator) ist fertig** – `src/mapgen/tab.rs` (`generate`, `attempt_map`, `Params`), Rausch-
Ebenen in `src/mapgen/layer.rs` (`Layer`, `spiral`), TABs Simplex-Rauschen in `src/noise.rs` (`simplex`,
`simplex_octaves`), Landschaften als Karten „TAB BR“ … „TAB VO“ in `data\game_maps.json` (BR ist der Standard),
`population` der Schwierigkeit, `Ore::Gold`. `World::generate_with` übernimmt Basis, Zombies (`Generated::zombies`,
schlafend) und Beute (`LootSpot::haul`) vom Generator. Was er hinterlässt:
- Alle Zahlen der Landschaften geschätzt (gemessen mit `tab_numbers`, `noise_quantiles` – ignorierte Tests in
  `tab.rs`); mit Screenshots echter TAB-Karten nachstellen.
- Unsere Ergänzung: unter 30 % der Spielfläche erreichbar → neuer Seed (`MIN_REACHED`) – sonst war bei FA etwa jede
  zehnte Karte fast leer. TAB prüft das nicht. Anzahlen (Dörfer, Ruinen, Riesen) wachsen mit der Fläche (`TAB_AREA`).
- Für Schritt 4: ein Dorfhaus bekommt vorerst klein 1/5, mittel 2/5, groß alle `doom_zombies` (`World::generate_with`)
  – TABs Dörfer haben viel mehr Häuser als unsere. Die Häuser liegen wie in TAB; Schübe, Überfälle usw. fehlen.
- Gold liegt auf der Karte, aber keine Mine baut es ab (`Resource::from_ore`) – Schritt 8 (Kohle) klärt das mit.
- Nicht übernommen: alte Türme (Schritt 7, „neutral → Kolonie“), Raben, Vorräte, Mutanten, Deko, TABs Start-Einheiten
  (unsere kommen von der Schwierigkeit), Kartengröße als Zweierpotenz.
- `keep_out`, `roaming_zombies`, `villages_percent`, `giants_percent` gelten nur noch für unsere Generatoren. Ob die
  alten (Wild, Regions …) bleiben, entscheidet der Nutzer.

---

## Schritt 2 – Der Entscheidungsbaum der Zombies wie in TAB

**Entscheidungen (schon getroffen):** den ganzen Baum mit TABs Takten übernehmen (die Verzögerungen sind Absicht,
nicht Sparen); Ansteckung der Wellen; Leerlauf 1300/150/150; Ziele nur Kommandozentrale, Soldaten und Gebäude mit
Leuten, Wände nur wenn sie im Weg stehen; getroffen → die ganze Kachel zielt auf den Schützen; Infektion wie TAB;
**Lückensuche raus** (wie sie ging: Gedächtnis `gap-search-removed`); **Ringe wie TAB**: normale Zombies haben beim
Wandern keinen Ring mehr, nur Riesen (54 Kacheln); die Abstände je Stärke gibt es nur noch bei der Kartenerzeugung
(Schritt 6).

**So macht TAB es** – `ZX.Behaviours\ZXBehaviour.cs`, Baum `z_3WFDS4wsbSYv` (Zombies) und `z_mAyC9fE5rX6eU1fUHQ`
(Riese). Die erste Regel, die passt, gewinnt:

1. Tot → Sterbe-Animation.
2. Ziel ist eine Wand → öfter nach Feinden schauen (`WatchForEnemies(2)`) und losrennen, sobald einer in Sicht ist.
3. **Feind in Sicht** → hinrennen und angreifen (`ChaseEnemies`), solange einer da ist.
4. **Feind aus den Augen verloren** → zu seinem letzten Platz rennen, dort stehen bleiben (`LastEnemyPosition`).
5. **Wellen-Zombie** (`AttackCommandCenter`) → zur Kommandozentrale rennen und alles angreifen, was unterwegs in
   Reichweite kommt (`RunToPositionAndAttack`).
6. **Ansteckung, alle ~4 s** (`CheckNearZombiesToAttackCommandCenter`, Takt 238): stehen auf seiner Kachel mindestens
   **3 Wellen-Zombies mit gleicher oder höherer Stufe** (`Level`), wird er selbst Wellen-Zombie. Braucht eine Stufe je
   Art in `data\zombies.json` (Wert schätzen: schwach 1 … Riese höchste).
7. Lärm (schon gebaut, Schritt 1).
8. **Leerlauf:** stehen bleiben, dann gewürfelt mit Gewichten **1300 stehen** (so lange wie seine Steh-Animation) /
   **150 knurren** (Spezial-Animation) / **150 wandern**. Wanderziel (`GetRandomRoamingPosition`): pro Achse
   zufällig ± (min … max) Kacheln vom jetzigen Platz – ein Quadrat, kein Kreis; 20 Versuche, das Ziel muss frei und
   auf der Spielfläche sein; gelaufen wird im Gehtempo. Ring nur, wenn die Art einen hat (`CellsAwayCommandCenterWhenRoaming`
   > 0): dann kein Ziel im Ring, außer er steht schon drin.

- **Riese:** dasselbe ohne Punkt 4 und 6; dazu **alle 12 s** (Takt 720) stehen bleiben und brüllen.
- **Nach Feinden schauen** (`WatchForEnemies`): alle `EntityWatchInterval` (Wert pro Art, unbekannt) – außerhalb der
  Sicht des Spielers 3× seltener, in unerkundetem Gebiet 5× seltener.
- **Wen Zombies angreifen** (`CEntityWatcher.cs`, Maske `HasPeople`, gesetzt in `ZXEntity.cs` um Zeile 1233): die
  Kommandozentrale, Soldaten und jedes Gebäude mit Arbeitern, Bewohnern oder Arbeiter-Angebot, solange es nicht
  infiziert ist. Den nächsten in Sicht (Sichtweite, Sichtlinie). Ein infiziertes Gebäude zählt für sie als tot – sie
  lassen davon ab. **Wände, leere Bauwerke, Strommasten sind kein Ziel.**
- **Wand-Regel** (`CEntityWatcher.z_qyVWs5Nb64VULfbWHg`; gilt nicht für Springer wie die Harpyie):
  - Hat er einen Feind und ist weiter als 0,5 weg: steht er in einer Wand → die. Läuft er schräg zwischen zwei
    Wandkacheln hindurch → eine davon (nach Zufall/Hash). Sonst unter den Wänden im Quadrat (eigene Größe + 3
    Kacheln) die nächste, deren Kachel die gerade Linie zum Feind schneidet → die wird sein Ziel.
  - Kein Feind, aber das letzte Ziel war eine stehende Wand → weiter auf sie.
  - Wellen- oder gelockter Zombie: dieselbe Prüfung auf der Linie zum nächsten Wegpunkt, Umkreis 1.
  - Wegplanung der Zombies geht **durch Bauwerke hindurch** (`Zombie.PathfindingMask` ohne `Structure`) – sie suchen
    nie eine Lücke.
- **Getroffen** (`Zombie.z_YgT24UvF4uULJ_0tEQ`, `CEntityWatcher.z_XrMC4kibLpjfUD8SVHAoA1I`): hat er kein Ziel, nehmen
  er und **alle Zombies im Umkreis von 1 Kachel ohne Ziel** den Schützen ins Visier; steht eine Wand dazwischen
  (Umkreis 1), zuerst die Wand.
- **Sicherung:** sind weniger als 100 Zombies übrig, tötet sich alle 2 Minuten jeder, der keinen Weg zur
  Kommandozentrale hat (`Zombie.OnUpdate`) – damit die Runde nicht am letzten Zombie hängt.
- **Infektion** (`ZX.Commands\ZXCommand.cs` `z_ySWFpQVBW8aiJpKOR5RMuUQ`, `ZX.Components\CInfected.cs`,
  `ZXEntity.z__IXLMWUM6hFWp6KjLg`): ein infizierbares Gebäude kippt, sobald sein Schaden `DeffensesLife` erreicht (bei
  uns: der Schild). Heraus kommen **max(0, Arbeiter + Bewohner − Arbeiter-Angebot) + `ExtraUnitsWhenInfected`**
  Zombies, zufällig aus WeakA/B/C, DressedA, WorkerA/B, MediumA/B, alle **auf die Kommandozentrale**; Lärm 50 je
  Person darin. Von Zombies getötete Einheiten stehen als Zombies auf.

**Bei uns heute:** `src/sim/zombies.rs` (`zombie_intent` mit eigener Prioritätenliste, `update_minds`, `start_seeking`,
`Lure`/`Why`, `Wander`), `src/hunt.rs` (Lückensuche, `Through`), `src/sim/sleep.rs` (Schlafende bummeln mit Ring),
`src/sim/buildings.rs` (Schild, `infect`), `keeps_out` in `src/sim/mod.rs`, Ringe `keep_out`/`ring_ramp`/`inside_step`
in `data\game_difficulties.json` und `data\roaming.json`, `notice_buildings` und `seek_*` in `data\zombies.json`.

**Umsetzung:** `zombie_intent` in TABs Reihenfolge umbauen (kein echter Baum, eine Prioritätenliste); Takte als
Werte in `data\zombies.json`; Lückensuche und `hunt`-Variante `Never` entfernen; Ziel „Gebäude in 5 Kacheln" durch
„Gebäude mit Leuten in Sicht" ersetzen; Wand-Regel prüfen und angleichen; Ringe beim Wandern weg (auch bei den
Schlafenden), Riese 54; Ansteckung, Leerlauf-Würfel, Brüllen des Riesen, Nachbarn bei Treffer, Sicherung dazu.
Tests zu Lücke, Ringen und „bleibt draußen" anpassen oder streichen; neue Tests für Ansteckung, Leerlauf, Wand-Regel,
Ziele.

---

## Schritt 3 – Flüssiges Gedränge wie TABs Physik, aber deterministisch

**Entscheidung:** das Gefühl, dass die Masse an Mauern **wie eine Flüssigkeit** fließt, auf jeden Fall nachbauen –
mit eigenem Löser, Lockstep-tauglich.

**So macht TAB es:**
- Echte Physik-Engine (`DXVision_Engine\DXVision\DXPhysicWorld.cs`, native DLL): **12 Schritte/s, 10 Löser-Runden**,
  keine Schwerkraft, beim Zeichnen interpoliert. Zombies sind reibungsfreie **Kreise** (Durchmesser siehe oben: 0,4,
  Dicker 0,7, Riese 1,6). Masse kommt aus `Params.Mass` (in `ZXRules.dat`, unbekannt; die 1 in den Vorlagen gilt nicht).
- Steuerung (`ZX.Components\CMovable.cs`, `z_i4HgedM`): Kraft = (Wunsch-Geschwindigkeit − Geschwindigkeit) × Masse × 12
  – die Wunsch-Geschwindigkeit ist nach einem Schritt erreicht, Kontakte verändern sie im Löser.
- **Tempo im Schrittrhythmus:** die Geschwindigkeit schwankt mit der Lauf-Animation (`BellWalkingFactor`):
  Phase p der Animation, `b` = Bell-Faktor → f = p/b bis b, danach (1−p)/(1−b); Tempo = max(v/2, 2,5·v·f²) × 1,3.
- **Felsen:** wer gerade angehalten hat (5 s lang) bekommt die **500-fache Masse** und Geschwindigkeit 0; ebenso
  „Halten" und Menschen, die gerade schießen. Daran fließt die Menge vorbei.
- Bewegung wird alle 2 Ticks neu entschieden. Ziel erreicht bei ≤ 0,2 Kacheln; innerhalb 2 Kacheln und 1 s keine
  Bewegung → angekommen; kommt er 5 s lang nicht voran → aufgeben.
- Wege: A* pro Einheit (asynchron, Zwischenspeicher, gedrosselt); ist die gerade Linie frei, ohne Pfad direkt hin;
  Wegpunkte mit freier Sicht werden übersprungen. Für uns nicht nötig – wir haben Flow Fields.

**Warum es flüssig wirkt:** 10 Löser-Runden pro Schritt lassen den Druck durch etwa zehn Reihen Zombies laufen; die
vordere Reihe weicht seitlich aus, reibungsfreie Kreise rutschen aneinander vorbei, alles fließt um Ecken.

**Plan (Position Based Dynamics):**
1. Jeder Zombie macht seinen Wunschschritt (aus `zombie_intent`).
2. 6–10 Runden: jeder schiebt sich aus den Nachbarn heraus, die er überlappt, anteilig nach Masse (Gewicht 1/m);
   aus Wänden, Gebäuden und festem Gelände (Kacheln als Rechtecke) ganz.
3. Geschwindigkeit = was dabei herauskam.
- **Deterministisch und parallel (Jacobi):** in jeder Runde rechnet jeder Zombie nur aus den Positionen der vorigen
  Runde (`rayon`, nur lesen, in Listenreihenfolge einsammeln) – bitgleich auf jedem PC.
- Felsen-Regel und Massen (Riese schwer, Dicker schwerer als Walker) übernehmen; Schrittrhythmus nur beim Zeichnen,
  falls er die Simulation stören würde.
- Ruhige Zombies weit weg weiter billig lassen (`is_calm`, Schlaf).

**Bei uns heute:** `src/sim/zombies.rs` – `crowd_step`, `unpile_zombies`, `push_apart`, `slide`, `shove`, `INERTIA`,
Kerne (DESIGN.md 8.1, die Absätze „A crowd stands a little inside itself", „They push each other", „The mass
pushes"); `src/grid.rs` (Nachbarn). Viele Sonderregeln dort fallen mit dem Löser weg.

**Prüfen:** vorher/nachher messen (`horde_tick_time`, 65.000 Zombies); die Gedränge-Tests (600 an der Mauer, Strom
von 200 an die Mauer, Dicker schiebt Walker …) anpassen; im Spiel Screenshots einer Horde an einer Mauer vergleichen.

---

## Schritt 4 – Dörfer des Untergangs wie TAB

**So macht TAB es** (`ZX.Components\CInfectionNest.cs`):
- Die Zombies liegen nicht im Haus, sie **entstehen bei Bedarf** – bis zu `InfectionNestMaxUnits` × Bevölkerungsfaktor.
- **Schub** = aufgerundet (zufällig 0,5 … 1,5) × Hausgröße × Bevölkerungsfaktor × `InfectionNestsFactor` der
  Landschaft × k. Hausgröße: klein 1, mittel 2, groß 5. Höchstens **ein Schub pro Sekunde**.
- Wann:
  - **Jede Sekunde hören** (schon gebaut, Schritt 1) → Schub × 0,5 geht zum Lärm.
  - **Alle 12 Spielstunden** (versetzt je Haus): stehen weniger als 2 × Größe Zombies ums Haus, Schub × 1 bleibt dort
    – die Wachen füllen sich auf.
  - **Ab Tag 20, alle 5 Tage** (versetzt je Haus): Schub × 1 **marschiert zur Kommandozentrale**.
  - **Getroffen:** Schub × 3 auf den nächsten Menschen in 10 Kacheln, und **jedes Nachbarhaus** in der Umgebung
    (Hausfläche × 3 + 4 Kacheln) gibt einen Schub × 1 dazu, zum selben Punkt.
  - **Finale Welle:** jede Sekunde Schub × 0,8 zur Kommandozentrale, bis leer.
- **Inhalt:** klein: WeakA/B/C; mittel: MediumA/B, WorkerA/B; groß: MediumA/B, WorkerA/B, DressedA (je Gewicht 100)
  und StrongA/Chubby (Gewicht 15). Sie erscheinen zufällig 1 Kachel ums Haus und blenden in 0,2 s ein.

**Bei uns heute:** `src/sim/doom.rs` (`DoomHouse` mit `inside`, `near_zombies`, `release_per_second`, `hit_zombies`,
`listen_houses`), `data\doom.json`, `doom_zombies` der Schwierigkeit, `src/mapgen/villages.rs`. Umbauen auf Schübe,
Überfälle, Nachfüllen, Nachbarn; Arten nach der Zuordnung aus „Für jeden Schritt". Wo die Häuser liegen, macht
Schritt 6.

---

## Schritt 5 – Wellen aus Regeln (= Events im Editor)

**So macht TAB es** – Survival-Plan in `TheyAreBillions\--zyl_NPjjlA7DRfVtsRJCX1kN4BxSr.cs` (Zeilen ~861–1029), Ablauf
in `ZX.GameSystems\ZXLevelEvent.cs`, Spawner in `ZX.Components\CUnitGenerator.cs`. Basis 100 Tage; **Zeiten × Tage/100**,
**Anzahlen × Bevölkerungsfaktor** (nur Arten mit Stärke ≤ 100, mindestens 1).

| Ereignis | ab Tag | Wiederholung | von wo | Arten (je) | Wachstum pro Mal | Streuung |
|---|---|---|---|---|---|---|
| Streuner schwach | 2 | alle 2 Tage, 10× | alle 4 Seiten | WeakA/B/C je 1–2 | +10 % | ganze Seite, still |
| Schwarm leicht | 13 ± 1 | alle 10 Tage, 4× | 1 zufällige Seite | MediumA/B, WorkerA/B je 4–6 | +300 % | 8 Kacheln, 8 h vorher angekündigt |
| Streuner mittel | 25 ± 1 | alle 6 Tage, 10× | alle 4 Seiten | DressedA, MediumA/B, WorkerA/B je 1–2 | +50 % | still |
| Schwarm schwer | 50 ± 1 | alle 7 Tage, 5× | 1 zufällige Seite | DressedA, MediumA/B, WorkerA/B je 25–40, StrongA 3–5 | +350 % | 20 Kacheln, 8 h vorher |
| Finale Welle | 92 | 1× | alle 4 Seiten | 5 × 550 + StrongA, Harpy, Venom je 50 – **je Seite** | – | ganze Seite, 24 h vorher |
| Sieg | 100 | | | | | |

- **Wachstum:** Anzahl = Basis × (1 + Faktor × (n − 1)), höchstens × Obergrenze. „Alle 4 Seiten" heißt: **jede**
  Seite bekommt die volle Anzahl. Seiten-Ausdruck: `N | S | E | W` = eine davon zufällig, `N & S & E & W` = alle.
- **Bevölkerung unter 0,5:** Streuner schwach erst ab Tag 10, alle 5 Tage, 5×; Streuner mittel ab Tag 40, alle 10
  Tage, 5×; nur halb so viele Dörfer.
- **Finale Welle:** deckt die Karte auf, schickt **jeden** Zombie und jedes Dorf zur Kommandozentrale.
- Alle Ereignisse: `AttackCommandCenter` – die Zombies laufen zur Kommandozentrale.
- **Spawner:** 4 Streifen N/S/O/W, je 70 % der Seitenbreite, 2 Kacheln tief, 10 Kacheln außerhalb der Spielfläche.
  Ein Spawner arbeitet alle 4 Ticks (`VeryFast` = 8) und gibt dabei bis zu `GenerationSpeed` Zombies aus (Standard 6,
  1–12) → bis ~45 pro Sekunde und Seite. Streuung: ein Quadrat dieser Größe an einer zufälligen Stelle des Streifens.
- **Event-Felder** (Editor-Datenmodell): Name, Startzeit, Zufallsversatz, „alle X", höchstens N-mal, Endzeit, Wachstum
  pro Mal, Obergrenze, Ankündigung (an/aus, Vorlauf, Countdown, Minikarte, „als nächster Schwarm zeigen"), Spawner-
  Ausdruck, Streuung, Tempo, bis 10 Arten mit Spanne, Musik, Skript beim Start/Ende, Gewonnen/Verloren, Finale
  Welle, Karte zeigen, alle zur Kommandozentrale, „gewonnen, wenn der Schwarm tot ist".
- Kampagnen-Beispiele: Tabelle `LevelEvents` in `ZXCampaign.dat` (öffnen siehe oben).

**Bei uns heute:** `data\spawning.json` (`stages`, `specials`, `waves.first/last`), `data\game_durations.json`,
`src/sim/spawning.rs`, `waves`/`constant_spawn` in der Kartendatei (`src/mapfile.rs`, `WaveGroup`, `WaveFrom`),
Tab „Waves" im Editor (`src/editor.rs`), die Unruhe (`unrest`, unsere Idee – vorerst behalten).

**Mit dem Nutzer klären:** Sieg wie TAB nach Zeit (8 Tage nach der finalen Welle) statt „wenn der letzte tot ist"?
Tageslänge (TAB 60 s, offene Frage 18)?

---

## Schritt 6 – TABs Kartengenerator mit den sechs Landschaften

**Entscheidungen:** TABs Generierung 1:1 übernehmen, darauf später aufbauen; Wirtschaftsunterschiede erstmal
ignorieren; **Gold so legen wie TAB** statt Kohle – **Kohle kommt später wieder dazu** (Gedächtnis `coal-later`).
TABs Generator wird der Standard; unsere alten (Wild, Regions, Rings, Canyons, Blank) bleiben im Editor wählbar, bis
der Nutzer entscheidet.

**Wichtig:** TAB hat **einen** Generator. Die Landschaften BR, TM, AL, DS, FA, VO (`ZX.GameSystems\ZXMapTheme*.cs`,
zufällig nach Gewicht `PW`) unterscheiden sich nur durch Zahlen, Grafik und kleine Nachschritte. Die Zahlen stehen in
`ZXRules.dat` → **pro Landschaft schätzen**, am besten mit Screenshots echter TAB-Karten vom Nutzer.

Zahlen je Landschaft: `LandDispersionFactor`, `LandOctaves`, `LandWaterFactor`, `MountainLandFactor`,
`VegetationOctaves`, `GrassEarthFactor`, `ForestGrassFactor`, `ForestEarthFactor`, `NumDoomVillages`,
`DoomVillagesSize`, `NumTreasures`, `NumSuppyFood`, `NumSuppyEnergy`, `Stone/Iron/GoldFormationMinDistance`,
`Stone/Iron/GoldEarthFactor`, `NCellsPerOilSource`, `NumOldTowers`, `InfectedDispersion`, `InfectedFactor`,
`MinDistanceFor{Weak,Medium,Strong,Powerful}Infected`, `NumInfectedGiants`, `NumInfectedMutants`,
`InfectionNestsFactor`, `NumRavens`, `ScoreFactor`, `PW`.

**Ablauf** (`--zyl_NPjjlA7DRfVtsRJCX1kN4BxSr.cs`, `z_Ezgd90E`; Rausch-Ebenen in `DXVision_Engine\DXVision\DXNoyseLayer.cs`
– dessen `FillWithNoyse`, `TruncateTo01`, `Enlarge`, `SetContrast`, `SubstractCircle`, `DeleteAloneCells`,
`GetAreaNearPoint` zuerst lesen):
1. Größe 128/256/512 (sonst die nächste Zweierpotenz, Spielfläche verkleinert). Rand = alles außerhalb der
   Spielfläche (Raute im Raster); Rand + 10 Kacheln extra.
2. **Land:** Rauschen (Streuung 0,03 ± 0,025 × `LandDispersionFactor`, `LandOctaves`) + Rand; über
   `LandWaterFactor` Erde, sonst Wasser.
3. **Berge:** dasselbe Rauschen ohne Rand, über `MountainLandFactor`.
4. **Pflanzen:** zweites Rauschen (0,03, `VegetationOctaves`) × Land, Kontrast 2 → Gras über `GrassEarthFactor`; Wald
   auf Gras über `ForestGrassFactor × GrassEarthFactor`; drittes Rauschen (0,03, 2 Oktaven) für Wald auf Erde über
   `ForestEarthFactor`. Beides zusammen = Wald.
5. Frei = Land ohne Wald, Berge, Rand. **Kommandozentrale in die Mitte** (nächster freier Platz Größe + 4). Kein Platz
   oder **weniger als 18 Wald** im Umkreis von 11 Kacheln → neuer Seed (aus dem Zufall, also weiter deterministisch).
6. Erreichbar: Flutfüllung von der Kommandozentrale; der Rest gilt als nicht frei.
7. **Dörfer:** Anzahl `NumDoomVillages`, Größe `DoomVillagesSize`. Mitte erreichbar, Abstand (größte Achse) zur
   Kommandozentrale ≥ 16 + 1,4 × Größe und ≤ Kartengröße/2 − 60 − Größe, ≥ 20 Kacheln zu anderen Dörfern. Kreis frei
   räumen (Gelände → Erde, außer Gras). In der Mitte ein großes Haus (Größe > 8) oder mittleres; drumherum spiralförmig
   im Kreis: jede fünfte Stelle übersprungen; bei Größe > 12 jedes 8. Haus groß (braucht 7 × 7 frei), 2 von 3 klein
   (4 × 4), sonst mittel (5 × 5).
8. **Ruinen mit Beute:** Anzahl `NumTreasures` + Vorräte (`NumSuppyFood`, `NumSuppyEnergy`) auf 4 × 4 Erde, frei,
   **8 bis 20 + Kartengröße/5** Kacheln von der Kommandozentrale, ≥ 20 voneinander. Eine Schatz-Ruine hat 2–4 Stücke:
   35 % Gold (500), sonst 10 einer Ressource – Wurf + Entfernung − Stücke × 15 entscheidet, welche (weiter draußen
   wertvoller).
9. **Stein:** Rauschen 0,07 ohne Oktaven, × frei, ohne Gras und Ruinen, Kreis `StoneFormationMinDistance` um die
   Kommandozentrale frei, über `StoneEarthFactor`; **mindestens 4 Stein im Quadrat ±20** um die Kommandozentrale, sonst
   neu würfeln (100 Versuche, dann neuer Seed). **Eisen** genauso, mindestens 10 im Quadrat ±50. **Gold** genauso mit
   `GoldFormationMinDistance`, einzelne Zellen gelöscht (`DeleteAloneCells`), ohne Mindestmenge.
10. **Öl:** Raster aus Blöcken `NCellsPerOilSource`; pro Block bis 500 Versuche, eine 2 × 2-Quelle mit 4 Kacheln
    Abstand zum Blockrand, 6 × 6 rundherum frei von allem.
11. **Alte Türme:** `NumOldTowers` auf freien 5 × 5-Plätzen 16–40 Kacheln von der Mitte, ≥ 20 voneinander; neutral
    und übernehmbar, 75 % Leben, 5 Wandstücke auf der Seite zur Kommandozentrale (passt zu 12.9, Schritt 7:
    „neutral → Kolonie").
12. **Start:** 4 + 1 Einheiten unten rechts neben der Kommandozentrale.
13. **Zombies:** Rauschen `InfectedDispersion` ohne Rand (+ 4), × frei, Kreis `MinDistanceForWeakInfected` frei.
    Schwelle t = 1 − `InfectedFactor` × Bevölkerung; Stufe = gerundet 4 × (Wert − t)/(1 − t). Würfel 0–99 je Kachel:
    ≥ 50 und weiter als `MinDistanceForMedium` → (≥ 60 und weiter als `Powerful` → **Powerful** 1 oder 2, sonst
    weiter als `Strong` → **Strong** 1–3, sonst **Medium** 1–3); sonst ≥ 30 → **Weak** 1–3; sonst leer. Die Zahl
    ist die Anzahl auf der Kachel (zufällig bis 0,4 um die Mitte). Weak = WeakA/B/C; Medium = WorkerA/B, MediumA/B,
    DressedA; Strong = DressedA; Powerful = StrongA, Harpy, Venom. (Stufe 0 ergibt in TAB 3 – Eigenheit.)
14. **Riesen:** `NumInfectedGiants` × (0 unter 0,5 Bevölkerung, ½ bis 1, 1 ab 1, 2 ab 1,2, 3 ab 1,4); auf
    Zombie-Gebiet, ≥ 70 von der Kommandozentrale, ≥ 40 voneinander; wandern nie näher als 54 Kacheln.
15. Nachschritt der Landschaft: alle legen 2 Deko-Objekte in 5 × 5-Waldstücke (≥ 20 auseinander); VO zusätzlich bis
    600 Felsbrocken auf 10 × 10-Bergstücke; sonst nur Grafik.
16. Spawner und Wellen wie Schritt 5.

**Bei uns heute:** `src/mapgen/` (`mod.rs`, `wild.rs`, `regions.rs`, `rings.rs`, `canyons.rs`, `start.rs`, `ores.rs`,
`villages.rs`, `lanes.rs`, `tools.rs`), `src/noise.rs`, `data\game_maps.json` (Presets wie Peaceful Lowlands), PNG-
Vorschau `dump_previews` (ignorierter Test). Unsere Spielfläche ist ein eigenes Rechteck im Raster (12.9) – TABs
Kommandozentrale steht in der Mitte der Spielfläche.

---

## Schritt 7 – Editor wie TABs nachbauen, dann weiter

**Entscheidung:** Funktionen und Datenmodell von TABs Editor 1:1, Oberfläche unsere (egui); Undo/Redo und Testmodus
bleiben; TABs freie Skriptsprache (`ZX.Editor\ZXAction_GenericCode.cs`, `ZXCompiler.cs`) nicht – dafür die geplanten
Trigger in Sätzen (DESIGN.md 12.9, Schritt 6). Danach die SC2-Teile.

**TAB lesen:** `ZX.Editor\FMain.cs` (Hauptfenster), `FEditObject.cs` (Eigenschaften eines Objekts),
`ZX.GameSystems\ZXLevelExtension.cs` (Karteneinstellungen „Info"), `ZXPainter*.cs` (Ebenen: Terrain, Objects, Zombies,
Roads, Fortress, Pipes, Belts), `ZXLevelEvent.cs` (Events = Schritt 5), `ZX.Editor\ZXScript.cs` und `ZXAction_*.cs`
(Aktionen: Block, Auswahl, Schleife/Abbruch, Einheiten erzeugen, Objekte als Bonus geben, Ressourcen geben, Technik
freischalten, zufällige Häuser infizieren, Zufall, Ton an/aus, Zug schicken/ausladen, Meldung, Fenster),
`ZXLevelCondition.cs`, `ZXMapCondition.cs`.

**Bei uns heute:** `src/editor.rs`, `src/undo.rs`, `src/mapfile.rs` (Format 4), DESIGN.md 12.9 (Schritte 1–8).
Zuerst eine Leserunde durch TABs Editor und eine Liste „TAB hat – wir haben – fehlt" mit dem Nutzer abstimmen.

---

## Schritt 8 – Kohle wieder dazu

Nach Schritt 6: Kohle fehlt (TABs Gold liegt an ihrer Stelle). Mit dem Nutzer klären, wo und wie viel – Köhlerei und
Gießerei brauchen sie (DESIGN.md 5).
