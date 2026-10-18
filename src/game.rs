//! One map on the screen – a round in the game, or a map in the editor: the world and everything around it (camera,
//! input, menu, measurements), the fixed ticks of the simulation, drawing – and what only happens here: the end screen
//! of a round, the editor's switch between Edit and Test, saving the map file, and the question about unsaved changes.
//! (Moved here from `main.rs` when the editor became a program of its own.)

use std::path::PathBuf;
use std::time::Instant;

use macroquad::prelude::*;

use crate::app::Program;
use crate::data;
use crate::draw::{self, Fog};
use crate::editor::{EditorRequest, Mode, Sight};
use crate::home;
use crate::hud::{self, Layout};
use crate::iso::Camera;
use crate::mapfile::{self, MapFile};
use crate::mapgen::MapSetup;
use crate::menu::{Menu, MenuAction};
use crate::overlay::{self, Action, Overlay, Switch};
use crate::picture::{Extras, MapPicture};
use crate::settings::Settings;
use crate::sim::{day_and_hour, Order, Outcome, TimedWave, WaveFrom, World, TICK};
use crate::ui::Ui;
use crate::vision::Vision;
use crate::widgets::{button, Column};

/// The checksum is recorded at this tick (600 ticks = 30 s).
const CHECK_TICK: u64 = 600;
/// At most this many ticks per frame – otherwise one slow frame can make the next one even slower.
const MAX_TICKS_PER_FRAME: u32 = 40;

/// What the map on the screen wants `app.rs` to do.
pub enum GameAction {
    Nothing,
    /// Back to the home screen (the game's "Main menu", the editor's "Close").
    Home,
    /// The setup screen with the values of this map (the editor's "New map…").
    Setup,
    /// A new map with the same setup and a new seed (the game's "New map").
    NewMap,
    Quit,
}

pub struct Game {
    program: Program,
    world: World,
    camera: Camera,
    pub ui: Ui,
    menu: Menu,
    /// The map's file. `None`: it has none (yet) – a new map in the editor before its first "Save", or `--seed`.
    path: Option<PathBuf>,
    /// The map as it was loaded or last saved: the game's "Restart" starts from it, the editor compares with it to
    /// see unsaved changes.
    file: MapFile,
    /// Editor: the map being edited, put aside while a test runs – the test plays a copy (`switch_mode`).
    stash: Option<World>,
    orders: Vec<Order>,
    accumulator: f32,
    /// Editor (Edit): no tick runs there, but the zombie brush still places its zombies 20 times a second.
    brush_clock: f32,
    checksum_at_check: Option<u64>,
    // Moving averages, so the display doesn't jump.
    avg_tick_ms: f64,
    avg_draw_ms: f64,
    // Editor "View: Zombies": what the zombies see, and for which tick that was computed. Only computed while
    // someone looks at it, and at most once per tick – with thousands of zombies it's not cheap.
    zombie_view: Vision,
    zombie_view_tick: Option<u64>,
    // Zombies that leave your sight fade out instead of vanishing.
    fades: draw::Fades,
    /// The picture on the minimap – painted only when the map or the fog changed (`picture.rs`).
    minimap: MapPicture,
    /// The end screen was put away ("Look at the map") – the round stays over.
    end_hidden: bool,
    /// Editor: what to do once the question about unsaved changes is answered (`unsaved_dialog`).
    leaving: Option<GameAction>,
    /// Editor: the window's X was clicked (`app.rs`).
    quit_asked: bool,
}

impl Game {
    /// A new map from this setup and seed – not written anywhere yet (the game does that right after, `app.rs`). It
    /// goes through its map file at once (`from_world` → `to_world`), so the round starts exactly as it will after
    /// "Restart" or on the other PC in co-op.
    pub fn generate(seed: u64, setup: &MapSetup, settings: Settings, program: Program) -> Game {
        let world = World::generate(seed, setup);
        let mut file = MapFile::from_world(&world, &mapfile::free_name(setup.generator.name()));
        file.setup = Some(setup.clone());
        // `expect` stops the program with this text if it's an `Err`. It can't be one here: `from_world` writes
        // exactly what `to_world` reads.
        Game::open(file, None, settings, program).expect("a map just made from a world always loads")
    }

    /// A map from its file – to play (game) or to edit (editor). `Err` if the file doesn't add up.
    pub fn open(file: MapFile, path: Option<PathBuf>, settings: Settings, program: Program) -> Result<Game, String> {
        let world = file.to_world()?;
        let mut ui = Ui::new(settings, program == Program::Editor);
        ui.editor.map_name = file.name.clone();
        ui.editor.map_info = file.info();
        Ok(Game {
            program,
            camera: Camera::new(world.base),
            zombie_view: Vision::new(world.map.size),
            world,
            ui,
            menu: Menu::new(program),
            path,
            file,
            stash: None,
            orders: Vec::new(),
            accumulator: 0.0,
            brush_clock: 0.0,
            checksum_at_check: None,
            avg_tick_ms: 0.0,
            avg_draw_ms: 0.0,
            zombie_view_tick: None,
            fades: draw::Fades::default(),
            minimap: MapPicture::new(),
            end_hidden: false,
            leaving: None,
            quit_asked: false,
        })
    }

    /// The game: a new round gets its file right away, in the maps folder – one game = one file, like in TAB.
    /// If that doesn't work the round still runs; it just can't be found in the list later.
    pub fn write_new_file(&mut self) {
        let Some(path) = mapfile::path_for(&self.file.name) else {
            return;
        };
        match self.file.write(&path) {
            Ok(()) => self.path = Some(path),
            Err(e) => self.ui.alert(&format!("The map file couldn't be written: {e}")),
        }
    }

    /// The setup this map was made with – for "New map". A map without one (older file): the last one of the setup
    /// screen.
    pub fn setup(&self) -> MapSetup {
        self.file.setup.clone().unwrap_or_else(home::load_setup)
    }

    /// The window's X was clicked (editor) – handled in the next frame, like the "Quit" button.
    pub fn ask_to_quit(&mut self) {
        self.quit_asked = true;
    }

    /// One frame: input, ticks, drawing, and what the menu, the editor's panel and the dialogs ask for.
    pub fn frame(&mut self) -> GameAction {
        let settings_before = self.ui.settings.clone();
        let dt = get_frame_time();
        self.ui.update_cursor();

        // A question or the end screen in front: only their buttons count – no Esc menu, no input on the map.
        let modal = self.leaving.is_some() || self.end_screen_shown();
        // Esc first cancels a waiting command, placing or an editor tool, then clears the selection, otherwise it opens
        // the menu. An open menu it always closes.
        if !modal && is_key_pressed(KeyCode::Escape) && (self.menu.open || !self.ui.is_busy()) {
            self.menu.escape();
        }
        // While the menu is open, everything stands still in single player (DESIGN.md 7.1): no input, no ticks.
        if !self.menu.open && !modal {
            // The overlay (F3) first: a click on it is for it, not for the map behind it.
            self.ui.overlay = None;
            if self.ui.settings.dev_overlay {
                let overlay = self.overlay(dt);
                if let Some(action) = overlay.click(&self.ui.settings) {
                    self.overlay_action(action);
                }
                self.ui.overlay = Some(overlay.area(&self.ui.settings));
            }
            self.ui.handle_input(&self.world, &mut self.camera, &mut self.orders, dt);
        }
        self.simulate(dt, modal);
        self.draw(dt);

        let mut action = GameAction::Nothing;
        // What the editor's panel asks for – only `Game` can do it.
        if let Some(request) = self.ui.editor.request.take() {
            match request {
                EditorRequest::Save => self.save(),
                EditorRequest::Close => action = self.leave(GameAction::Home),
                EditorRequest::NewMap => action = self.leave(GameAction::Setup),
                EditorRequest::Mode(mode) => self.switch_mode(mode),
            }
        }
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if self.program == Program::Editor && ctrl && is_key_pressed(KeyCode::S) && !self.ui.editor.is_typing() && !modal {
            self.save();
        }
        if self.end_screen_shown() {
            action = self.end_screen();
        }
        if self.leaving.is_some() {
            action = self.unsaved_dialog();
        }
        // The menu on top of everything; what was clicked there is carried out here.
        match self.menu.show(&mut self.ui.settings) {
            MenuAction::Nothing => {}
            MenuAction::Restart => self.restart(),
            MenuAction::NewMap => action = GameAction::NewMap,
            MenuAction::Save => self.save(),
            MenuAction::MainMenu => action = self.leave(GameAction::Home),
            MenuAction::Quit => action = self.leave(GameAction::Quit),
        }
        // `mem::take`: reads the value and leaves `false` behind – "asked once, handled once".
        if std::mem::take(&mut self.quit_asked) {
            action = self.leave(GameAction::Quit);
        }
        self.ui.settings.apply_changes(&settings_before);
        action
    }

    /// The simulation in fixed ticks (DESIGN.md 12.5) – or, where no time passes, only the orders.
    fn simulate(&mut self, dt: f32, modal: bool) {
        // The editor's Edit mode: time stands still, every change shows at once.
        let editing = self.ui.editor.editing() && !self.ui.editor.testing();
        let runs = !editing && !self.ui.paused && !self.menu.open && !modal && self.world.outcome.is_none();
        let first_tick = self.world.tick;
        let sim_start = Instant::now();
        let mut ticks = 0;
        if runs {
            self.accumulator += dt.min(0.25) * self.ui.speed as f32;
            while self.accumulator >= TICK && ticks < MAX_TICKS_PER_FRAME {
                // The zombie brush of the editor places its zombies every tick.
                if let Some(order) = self.ui.brush_order() {
                    self.orders.push(order);
                }
                self.world.step(&self.orders);
                self.orders.clear();
                self.accumulator -= TICK;
                ticks += 1;
                if self.world.tick == CHECK_TICK {
                    self.checksum_at_check = Some(self.world.checksum());
                }
                if self.world.outcome.is_some() {
                    break; // the round is over – not a tick more
                }
            }
        } else {
            if editing && !self.menu.open {
                self.brush_clock += dt;
                while self.brush_clock >= TICK {
                    self.brush_clock -= TICK;
                    if let Some(order) = self.ui.brush_order() {
                        self.orders.push(order);
                    }
                }
            }
            if !self.orders.is_empty() {
                // Paused (Space) or editing: take the orders over right away – you see the lines, queues and rally
                // points, the painted terrain, the placed zombies – but no time passes, so nobody starts moving.
                self.world.apply_orders(&self.orders);
                self.orders.clear();
                if editing {
                    self.world.update_vision();
                    self.zombie_view_tick = None;
                }
            }
        }
        if ticks > 0 {
            let tick_ms = sim_start.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
            self.avg_tick_ms = smooth(self.avg_tick_ms, tick_ms);
        }
        self.announce_waves(first_tick);
    }

    /// A wave of the timeline came in the ticks since `from`: say so at the top of the screen.
    fn announce_waves(&mut self, from: u64) {
        let came: Vec<TimedWave> = self.world.waves.iter().filter(|w| w.at >= from && w.at < self.world.tick).copied().collect();
        if came.is_empty() {
            return;
        }
        let sides = |from: WaveFrom| match from {
            WaveFrom::OneSpawner => 1,
            WaveFrom::Sides(n) => n.min(4),
        };
        let zombies: u32 = came.iter().map(|w| w.count * sides(w.from)).sum();
        let text = if came.iter().any(|w| w.final_wave) {
            format!("THE FINAL WAVE is coming - {zombies} zombies!")
        } else {
            format!("A wave is coming - {zombies} zombies!")
        };
        self.ui.alert(&text);
    }

    /// The game: the same map again from the start – read anew from its file, so a change made in the editor counts
    /// (and the values in `data\` too). The editor: back to the map as it was saved ("Revert to saved").
    fn restart(&mut self) {
        data::reload();
        if let Some(file) = self.path.as_ref().and_then(|path| MapFile::load(path).ok()) {
            self.file = file;
        }
        match self.file.to_world() {
            Ok(world) => {
                self.ui.editor.map_name = self.file.name.clone();
                self.reset(world);
            }
            Err(e) => self.ui.alert(&format!("Couldn't load the map: {e}")),
        }
    }

    /// A fresh start with `world`. The game starts its round anew (camera, selection, groups, speed); the editor keeps
    /// its tab, tools and camera and goes back to Edit.
    fn reset(&mut self, world: World) {
        match self.program {
            Program::Game => {
                self.camera = Camera::new(world.base);
                self.ui = Ui::new(self.ui.settings.clone(), false);
            }
            Program::Editor => {
                self.ui.forget_selection();
                self.ui.editor.set_mode(Mode::Edit);
                self.ui.editor.map_info = self.file.info();
            }
        }
        self.zombie_view = Vision::new(world.map.size);
        self.world = world;
        self.stash = None;
        self.orders.clear();
        self.accumulator = 0.0;
        self.checksum_at_check = None;
        self.zombie_view_tick = None;
        self.fades = draw::Fades::default(); // the new world numbers its zombies from 1 again
        self.end_hidden = false;
    }

    /// The map in the editor as it is now – while a test runs, the one put aside (not the test).
    fn edited(&self) -> &World {
        self.stash.as_ref().unwrap_or(&self.world)
    }

    /// What the map's file would hold if it were saved now. `..`: all other fields from `from_world`.
    fn current_file(&self) -> MapFile {
        MapFile { setup: self.file.setup.clone(), ..MapFile::from_world(self.edited(), &self.ui.editor.map_name) }
    }

    /// Editor: Edit ↔ Test. The test plays the map as the game would – from its file as it would be saved now, on a
    /// copy. The edited map waits in `stash` and comes back unchanged: whatever happens in the test is gone after it.
    fn switch_mode(&mut self, mode: Mode) {
        if !self.ui.editor.editing() || mode == self.ui.editor.mode {
            return;
        }
        match mode {
            Mode::Test => match self.current_file().to_world() {
                // `mem::replace`: puts the test world in, and hands back the one that was there – into `stash`.
                Ok(test) => self.stash = Some(std::mem::replace(&mut self.world, test)),
                Err(e) => {
                    self.ui.alert(&format!("Can't test this map: {e}"));
                    return;
                }
            },
            Mode::Edit => {
                if let Some(edited) = self.stash.take() {
                    self.world = edited;
                }
            }
        }
        self.ui.editor.set_mode(mode);
        // The two worlds number their units and buildings differently.
        self.ui.forget_selection();
        self.ui.paused = false;
        self.orders.clear();
        self.accumulator = 0.0;
        self.zombie_view_tick = None;
        self.fades = draw::Fades::default();
    }

    /// Editor: writes the map into its file – named after the map. Renamed: the file gets the new name (one map = one
    /// file), unless another map already has it. What happened goes into the "Map" tab.
    fn save(&mut self) {
        if self.program != Program::Editor {
            return;
        }
        let result = self.write_file();
        if let Err(e) = &result {
            self.ui.alert(&format!("Not saved: {e}"));
        }
        self.ui.editor.status = Some(match result {
            Ok(name) => format!("Saved: {name}"),
            Err(e) => format!("Not saved: {e}"),
        });
    }

    /// `save` without the messages: `Ok` with the file name, or `Err` with the reason. `?` returns the error early.
    fn write_file(&mut self) -> Result<String, String> {
        let file = self.current_file();
        let target = mapfile::path_for(&file.name).ok_or("Windows doesn't say where APPDATA is")?;
        // Windows doesn't care about upper and lower case in file names – "wild 1" is the file "Wild 1".
        let same = |a: &PathBuf, b: &PathBuf| a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy());
        let renamed = self.path.as_ref().is_none_or(|old| !same(old, &target));
        if renamed && target.exists() {
            return Err(format!("there already is a map called \"{}\"", file.name));
        }
        file.write(&target)?;
        if let (true, Some(old)) = (renamed, &self.path) {
            let _ = std::fs::remove_file(old);
        }
        let name = target.file_name().map_or(String::new(), |f| f.to_string_lossy().into_owned());
        self.path = Some(target);
        self.ui.editor.map_info = file.info();
        self.file = file;
        Ok(name)
    }

    /// Editor: are there changes that aren't in the file? A map that was never saved always counts as changed.
    fn unsaved(&self) -> bool {
        self.program == Program::Editor && (self.path.is_none() || self.current_file() != self.file)
    }

    /// Leaving the map (close, quit, new map): in the editor with unsaved changes first ask, otherwise go right away.
    fn leave(&mut self, action: GameAction) -> GameAction {
        if self.unsaved() {
            self.leaving = Some(action);
            GameAction::Nothing
        } else {
            action
        }
    }

    /// The question before leaving with unsaved changes: save, don't save, or stay (Esc too).
    fn unsaved_dialog(&mut self) -> GameAction {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.55));
        let mut col = Column::centered("Unsaved changes", 2, 520.0);
        col.note(&format!("\"{}\" has changes that aren't saved yet.", self.ui.editor.map_name));
        col.gap();
        let row = col.split(3);
        if button(row[0], "Save", true) {
            self.save();
            if !self.unsaved() {
                return self.leaving.take().unwrap_or(GameAction::Nothing);
            }
            self.leaving = None; // saving failed – the message says why, the map stays open
        } else if button(row[1], "Don't save", true) {
            return self.leaving.take().unwrap_or(GameAction::Nothing);
        } else if button(row[2], "Cancel", true) || is_key_pressed(KeyCode::Escape) {
            self.leaving = None;
        }
        GameAction::Nothing
    }

    /// The game: is the round over, and is the end screen up?
    fn end_screen_shown(&self) -> bool {
        self.program == Program::Game && self.world.outcome.is_some() && !self.end_hidden
    }

    /// The end of the round (DESIGN.md 4.2): won or lost, when, and what happened – and what next.
    fn end_screen(&mut self) -> GameAction {
        let Some(outcome) = self.world.outcome else {
            return GameAction::Nothing;
        };
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.6));
        let (title, line) = match outcome {
            Outcome::Won { .. } => ("Victory", "The final wave is dead - the colony survived."),
            Outcome::Lost { .. } => ("Defeat", "The command center has fallen."),
        };
        let mut col = Column::centered(title, 9, 500.0);
        col.note(line);
        let (day, hour) = day_and_hour(outcome.tick());
        let s = self.world.score;
        col.note(&format!("Day {day} of {}, {hour:02}:00", self.world.days));
        col.note(&format!("Zombies killed: {}", s.zombies_killed));
        col.note(&format!("Units trained: {}   lost: {}", s.units_trained, s.units_lost));
        col.note(&format!("Buildings built: {}   lost: {}", s.buildings_built, s.buildings_lost));
        col.note(&format!("Gold earned: {}   spent: {}", s.gold_earned, s.gold_spent));
        col.gap();
        if button(col.row(), "Restart (same map)", true) {
            self.restart();
        }
        if button(col.row(), "Look at the map", true) {
            self.end_hidden = true;
        }
        if button(col.row(), "Main menu", true) {
            return GameAction::Home;
        }
        GameAction::Nothing
    }

    fn draw(&mut self, dt: f32) {
        let world = &self.world;
        let draw_start = Instant::now();
        clear_background(Color::from_rgba(20, 22, 28, 255));
        let alpha = (self.accumulator / TICK).min(1.0);
        self.fades.update(world, dt);
        let fog = match self.ui.sight() {
            Sight::All => Fog::Off,
            Sight::Player => Fog::Player,
            Sight::Zombies => {
                if self.zombie_view_tick != Some(world.tick) {
                    world.zombie_view(&mut self.zombie_view);
                    self.zombie_view_tick = Some(world.tick);
                }
                Fog::Zombies(&self.zombie_view)
            }
        };
        let view = draw::View {
            area: self.ui.view_area(world),
            editing: self.ui.editor.editing(),
            full_plan: self.ui.settings.always_full_queue
                || is_key_down(KeyCode::LeftShift)
                || is_key_down(KeyCode::RightShift),
            always_bars: self.ui.settings.always_health_bars,
            fog,
            fades: &self.fades,
            spawners: self.ui.spawner_shapes(world, &self.camera),
        };
        draw::draw_world(world, &self.camera, &view, &self.ui.selected, alpha);
        self.ui.draw(world, &self.camera);
        // The minimap's picture: with fog only in the player's view – in the editor's other views it stays readable.
        let fog = (self.ui.sight() == Sight::Player).then_some(&world.vision);
        self.minimap.refresh(&world.map, &Extras { fog, spawners: &[], base: false, shade_ring: false }, dt);
        hud::draw(&Layout::new(), world, &self.camera, &self.ui, &self.minimap);
        // A broken file in `data\`: the game runs on with the old values – but you have to see it.
        if let Some(problem) = data::problem() {
            let text = format!("data\\ not loaded, old values in use - {problem}");
            draw_rectangle(8.0, 40.0, measure_text(&text, None, 18, 1.0).width + 16.0, 26.0, Color::new(0.4, 0.05, 0.05, 0.9));
            draw_text(&text, 16.0, 58.0, 18.0, WHITE);
        }
        self.ui.editor.draw(world);
        self.draw_round_over();
        self.avg_draw_ms = smooth(self.avg_draw_ms, draw_start.elapsed().as_secs_f64() * 1000.0);

        if self.ui.settings.dev_overlay {
            self.overlay(dt).draw(&self.ui.settings);
        }
    }

    /// The round is over but the end screen isn't up: in the game after "Look at the map", in the editor's test.
    fn draw_round_over(&self) {
        let Some(outcome) = self.world.outcome else {
            return;
        };
        if self.end_screen_shown() {
            return;
        }
        let (day, hour) = day_and_hour(outcome.tick());
        let result = if matches!(outcome, Outcome::Won { .. }) { "won" } else { "lost" };
        let text = match self.program {
            Program::Game => format!("The round is over - {result} on day {day}, {hour:02}:00. Esc: menu"),
            Program::Editor => format!("Test over - the colony {result} on day {day}, {hour:02}:00. Edit (F10): back to the map"),
        };
        let w = measure_text(&text, None, 22, 1.0).width + 32.0;
        let (x, y) = ((screen_width() - w) / 2.0, 96.0);
        draw_rectangle(x, y, w, 36.0, Color::new(0.08, 0.09, 0.11, 0.9));
        draw_rectangle_lines(x, y, w, 36.0, 1.0, Color::new(0.95, 0.62, 0.20, 1.0));
        draw_text(&text, x + 16.0, y + 25.0, 22.0, WHITE);
    }

    /// The overlay in the top left (F3), built anew every frame – in sections that fold with a click (`overlay.rs`).
    fn overlay(&self, dt: f32) -> Overlay {
        let (world, ui) = (&self.world, &self.ui);
        let check = match self.checksum_at_check {
            Some(sum) => format!("{sum:016x}"),
            None => "not yet".to_owned(),
        };
        let mut o = Overlay::new();
        o.section("stats", "Measurements")
            .text(format!("FPS {}   frame {:.1} ms", get_fps(), dt * 1000.0))
            .text(format!("Simulation {:.2} ms per tick (budget 50 ms)", self.avg_tick_ms))
            .text(format!("Drawing {:.2} ms per frame (CPU only)", self.avg_draw_ms))
            .text(format!("Zombies {}   units {}   buildings {}", world.zombies.len(), world.units.len(), world.buildings.len()))
            .text(format!("Tick {}   speed x{} (+/-)", world.tick, ui.speed))
            .button(format!("Seed {}   map {} x {}   (click: copy)", world.seed, world.map.size, world.map.size), Action::CopySeed)
            .text(format!("Checksum now  {:016x}", world.checksum()))
            .text(format!("Checksum tick {CHECK_TICK}  {check}"))
            .text(match world.spawn_group() {
                Some(g) => format!("Spawn: {}-{} {:?}s every {:.1} s", g.min, g.max, g.breed, g.every as f32 * TICK),
                None => "Spawn: nothing yet (the constant spawn of the map)".to_owned(),
            });
        o.section("switches", "Switches")
            .switch("Pause (Space)", ui.paused, Switch::Pause)
            .switch("Edge scrolling", ui.settings.edge_scroll, Switch::EdgeScroll)
            .switch("Keep the mouse in the window (F12)", ui.settings.confine_mouse, Switch::ConfineMouse)
            .switch("Health bars on all units", ui.settings.always_health_bars, Switch::HealthBars)
            .switch("Whole command queue (else: hold Shift)", ui.settings.always_full_queue, Switch::FullQueue);
        o.section("select", "Select")
            .text("Click / box, Shift: add or remove, on nothing or Esc: none")
            .text("Click a zombie: its values")
            .text("Ctrl-click, double click: all of this type")
            .text("F2: all   Tab: active type (buildings too)");
        o.section("commands", "Commands")
            .text("Right click: move / attack zombie / follow")
            .text("  right drag: move in formation (X line, C box, V circle)")
            .text("M S H P T (or buttons), T/P/M + drag: formation")
            .text("Shift: queue - let go: the command leaves the mouse,")
            .text("  press Shift again: it's back")
            .text("Medic: T + click on a unit = beam at it");
        o.section("buildings", "Buildings")
            .text("Command center: Q colonists (Q tent), S military (Q barracks),")
            .text("  D defense (Q wall, W gate - R turns, E tower), W E A Y: planned")
            .text("  (costs gold, not within 4 tiles of a zombie). Right click / Esc: stop")
            .text("  B, Esc or select it again (click, group key): back to its main buttons")
            .text("Barracks: Q soldier, W ranger, R sniper, T medic, G rally")
            .text("Right click a tower with units: they go in. Tower selected: Q unload")
            .text("Any building: F repair (gold, all selected) - cures infected ones")
            .text("X (no units selected, or the button in the stock): repair all")
            .text("  cheapest first, not with a zombie within 4 tiles");
        o.section("groups", "Groups and camera")
            .text("Groups (units and buildings): Ctrl+number set, Shift+number add,")
            .text("  Ctrl+Shift+number steal, number select (2x: go)")
            .text("Camera: WASD (nothing selected), arrows, edge, wheel, minimap,")
            .text("  Ctrl+F5-F8 save, F5-F8 jump, Backspace: base")
            .text("  middle mouse drag: move the map");
        o.section("keys", "Other keys")
            .text("Space pause   +/- speed   F3 this overlay")
            .text("F12 keep mouse in window   Esc menu");
        if self.program == Program::Editor {
            o.text("F10 edit / test (top right)   Ctrl+S save").text("Click a spawner, drag its edge, Del removes it");
        }
        o
    }

    /// Carries out a click on the overlay.
    fn overlay_action(&mut self, action: Action) {
        let s = &mut self.ui.settings;
        match action {
            Action::Fold(key) => overlay::toggle_fold(s, key),
            Action::Switch(Switch::Pause) => self.ui.paused = !self.ui.paused,
            Action::Switch(Switch::EdgeScroll) => s.edge_scroll = !s.edge_scroll,
            Action::Switch(Switch::ConfineMouse) => s.confine_mouse = !s.confine_mouse,
            Action::Switch(Switch::HealthBars) => s.always_health_bars = !s.always_health_bars,
            Action::Switch(Switch::FullQueue) => s.always_full_queue = !s.always_full_queue,
            Action::CopySeed => {
                macroquad::miniquad::window::clipboard_set(&self.world.seed.to_string());
                self.ui.alert("Seed copied");
            }
        }
    }
}

/// Mixes a new measurement into the average at 5 % – outliers hardly count.
fn smooth(average: f64, value: f64) -> f64 {
    average * 0.95 + value * 0.05
}
