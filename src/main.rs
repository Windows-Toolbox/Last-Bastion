// In the release build don't open a black console window next to the game.
// In the debug build the console stays, so println! output is visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Throwaway prototype (DESIGN.md 12.2): 12 controllable units against a horde in a flow field,
// controls and bottom panel like in StarCraft 2. It's meant to show whether macroquad is fast enough
// and whether the controls feel good.

mod aggro;
mod cursor;
mod data;
mod draw;
mod editor;
mod flow;
mod formation;
mod grid;
mod home;
mod hud;
mod hunt;
mod iso;
mod kinds;
mod map;
mod mapfile;
mod mapgen;
mod menu;
mod noise;
mod picture;
mod placement;
mod rng;
mod settings;
mod sim;
mod ui;
mod vision;
mod widgets;

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use macroquad::prelude::*;

use draw::Fog;
use editor::{EditorRequest, Mode, Sight};
use home::{Home, HomeAction};
use hud::Layout;
use iso::Camera;
use mapfile::MapFile;
use mapgen::MapSetup;
use menu::{Menu, MenuAction};
use picture::{Extras, MapPicture};
use settings::{Settings, WindowMode};
use sim::{Order, World, TICK};
use ui::Ui;
use vision::Vision;

/// The checksum is recorded at this tick (600 ticks = 30 s).
const CHECK_TICK: u64 = 600;
/// At most this many ticks per frame – otherwise one slow frame can make the next one even slower.
const MAX_TICKS_PER_FRAME: u32 = 40;

/// Window settings that macroquad asks for before starting.
fn window_conf() -> Conf {
    Conf {
        window_title: "Last Bastion".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        window_resizable: true,
        fullscreen: Settings::load().window_mode == WindowMode::Borderless,
        ..Default::default()
    }
}

/// What is on the screen: the home screen or a round. An enum whose variants carry data – the home screen its
/// pages and values, the round its world. `Box`: the round is big, this way only a pointer to it sits in the enum.
enum Screen {
    Home(Home),
    Game(Box<Game>),
}

#[macroquad::main(window_conf)]
async fn main() {
    // `--seed 12345` skips the home screen and starts that map right away (with the last setup) – to repeat a map
    // or a bug, and for the checksum (README).
    data::reload();
    let mut screen = match seed_argument() {
        Some(seed) => {
            let setup = home::load_setup();
            Screen::Game(Box::new(Game::new(World::generate(seed, &setup), Settings::load(), Source::Generated { seed, setup }, false)))
        }
        None => Screen::Home(Home::new(Settings::load(), random_seed())),
    };

    loop {
        // What comes next is decided inside the `match` – but the screen can only be swapped after it, because
        // inside it `screen` is borrowed (`&mut screen`). So the next screen goes into `next` first.
        let mut next: Option<Screen> = None;
        match &mut screen {
            Screen::Home(home) => {
                let before = home.settings.clone();
                match home.frame(random_seed) {
                    HomeAction::Nothing => {}
                    HomeAction::Start { seed, setup, editor } => {
                        data::reload();
                        let world = World::generate(seed, &setup);
                        next = Some(Screen::Game(Box::new(Game::new(world, home.settings.clone(), Source::Generated { seed, setup }, editor))));
                    }
                    HomeAction::Open { file, editor } => {
                        data::reload();
                        match file.to_world() {
                            Ok(world) => next = Some(Screen::Game(Box::new(Game::new(world, home.settings.clone(), Source::File(file), editor)))),
                            Err(e) => eprintln!("Could not open the map: {e}"),
                        }
                    }
                    HomeAction::Quit => break,
                }
                apply_settings(&home.settings, &before);
            }
            Screen::Game(game) => match game.frame() {
                GameAction::Nothing => {}
                GameAction::MainMenu => next = Some(Screen::Home(Home::new(game.ui.settings.clone(), random_seed()))),
                GameAction::NewMapSetup => {
                    let mut home = Home::new(game.ui.settings.clone(), random_seed());
                    let (setup, seed) = match &game.source {
                        Source::Generated { seed, setup } => (setup.clone(), *seed),
                        Source::File(_) => (home.setup.clone(), random_seed()),
                    };
                    home.open_setup(setup, seed, true);
                    next = Some(Screen::Home(home));
                }
                GameAction::Quit => break,
            },
        }
        if let Some(screen_next) = next {
            screen = screen_next;
        }
        next_frame().await;
    }
}

/// Changed settings: apply them right away and save them (DESIGN.md 11.4).
fn apply_settings(now: &Settings, before: &Settings) {
    if now != before {
        if now.window_mode != before.window_mode {
            set_fullscreen(now.window_mode == WindowMode::Borderless);
        }
        now.save();
    }
}

/// Where the map of a round came from – "Restart" makes it again the same way.
enum Source {
    Generated { seed: u64, setup: MapSetup },
    File(MapFile),
}

/// What the round wants `main` to do.
enum GameAction {
    Nothing,
    MainMenu,
    /// The editor's "New map…": the setup screen, with the values of this map.
    NewMapSetup,
    Quit,
}

/// One round: the world and everything around it – camera, input, menu, measurements.
struct Game {
    world: World,
    camera: Camera,
    ui: Ui,
    menu: Menu,
    source: Source,
    orders: Vec<Order>,
    accumulator: f32,
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
}

impl Game {
    fn new(world: World, settings: Settings, source: Source, editor: bool) -> Game {
        let mut ui = Ui::new(settings);
        if editor {
            ui.editor.set_mode(Mode::Editor);
        }
        ui.editor.map_name = match &source {
            Source::Generated { .. } => "Untitled".to_owned(),
            Source::File(file) => file.name.clone(),
        };
        ui.editor.map_info = match &source {
            Source::Generated { seed, setup } => format!("{} map, seed {seed}", setup.generator.name()),
            Source::File(file) => format!("Saved map, made from seed {}", file.seed),
        };
        Game {
            camera: Camera::new(world.base),
            zombie_view: Vision::new(world.map.size),
            world,
            ui,
            menu: Menu::new(),
            source,
            orders: Vec::new(),
            accumulator: 0.0,
            checksum_at_check: None,
            avg_tick_ms: 0.0,
            avg_draw_ms: 0.0,
            zombie_view_tick: None,
            fades: draw::Fades::default(),
            minimap: MapPicture::new(),
        }
    }

    /// The same map again from the start: generated anew, or loaded again from its file. The values in `data\` are
    /// read anew first – change a number, "Restart", and it counts.
    fn restart(&mut self) {
        data::reload();
        let world = match &self.source {
            Source::Generated { seed, setup } => World::generate(*seed, setup),
            Source::File(file) => file.to_world().unwrap_or_else(|_| World::generate(random_seed(), &MapSetup::default())),
        };
        self.reset(world);
    }

    /// A new map with the same setup and a new seed. (A saved map has no setup – then the last one of the setup
    /// screen.)
    fn new_map(&mut self) {
        data::reload();
        let seed = random_seed();
        let setup = match &self.source {
            Source::Generated { setup, .. } => setup.clone(),
            Source::File(_) => home::load_setup(),
        };
        self.ui.editor.map_info = format!("{} map, seed {seed}", setup.generator.name());
        self.source = Source::Generated { seed, setup: setup.clone() };
        self.reset(World::generate(seed, &setup));
    }

    /// A fresh start with `world` – the settings, the editor mode and the map name stay.
    fn reset(&mut self, world: World) {
        self.camera = Camera::new(world.base);
        let mut ui = Ui::new(self.ui.settings.clone());
        ui.editor.set_mode(self.ui.editor.mode);
        ui.editor.map_name = self.ui.editor.map_name.clone();
        ui.editor.map_info = self.ui.editor.map_info.clone();
        self.ui = ui;
        self.zombie_view = Vision::new(world.map.size);
        self.world = world;
        self.orders.clear();
        self.accumulator = 0.0;
        self.checksum_at_check = None;
        self.zombie_view_tick = None;
        self.fades = draw::Fades::default(); // the new world numbers its zombies from 1 again
    }

    /// One frame of the round: input, ticks, drawing, menu.
    fn frame(&mut self) -> GameAction {
        let settings_before = self.ui.settings.clone();
        let dt = get_frame_time();
        self.ui.update_cursor();

        // Esc first cancels a waiting command, placing or an editor tool, otherwise it opens or closes the menu.
        if is_key_pressed(KeyCode::Escape) && !self.ui.is_busy() {
            self.menu.escape();
        }

        // While the menu is open, everything stands still in single player (DESIGN.md 7.1): no input, no ticks.
        if !self.menu.open {
            self.ui.handle_input(&self.world, &mut self.camera, &mut self.orders, dt);
        }

        // --- Simulation in fixed ticks (DESIGN.md 12.5) ---
        let sim_start = Instant::now();
        let mut ticks = 0;
        if !self.ui.paused && !self.menu.open {
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
            }
        } else if !self.orders.is_empty() {
            // Paused (Space): take the orders over right away – you see the lines, queues and rally points – but no
            // time passes, so nobody starts until the game goes on.
            self.world.apply_orders(&self.orders);
            self.orders.clear();
        }
        if ticks > 0 {
            let tick_ms = sim_start.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
            self.avg_tick_ms = smooth(self.avg_tick_ms, tick_ms);
        }

        self.draw(dt);

        // --- Menu on top; what was clicked there is carried out here ---
        let mut action = GameAction::Nothing;
        match self.menu.show(&mut self.ui.settings) {
            MenuAction::Nothing => {}
            MenuAction::Restart => self.restart(),
            MenuAction::NewMap => self.new_map(),
            MenuAction::MainMenu => action = GameAction::MainMenu,
            MenuAction::Quit => action = GameAction::Quit,
        }
        // What the editor's buttons ask for that only `main` can do.
        if let Some(EditorRequest::NewMap) = self.ui.editor.request.take() {
            action = GameAction::NewMapSetup;
        }
        apply_settings(&self.ui.settings, &settings_before);
        action
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
        hud::draw_clock(world);
        // A broken file in `data\`: the game runs on with the old values – but you have to see it.
        if let Some(problem) = data::problem() {
            let text = format!("data\\ not loaded, old values in use - {problem}");
            draw_rectangle(8.0, 40.0, measure_text(&text, None, 18, 1.0).width + 16.0, 26.0, Color::new(0.4, 0.05, 0.05, 0.9));
            draw_text(&text, 16.0, 58.0, 18.0, WHITE);
        }
        self.ui.editor.draw(world);
        self.avg_draw_ms = smooth(self.avg_draw_ms, draw_start.elapsed().as_secs_f64() * 1000.0);

        if self.ui.settings.dev_overlay {
            self.draw_overlay(dt);
        }
    }

    fn draw_overlay(&self, dt: f32) {
        let (world, ui) = (&self.world, &self.ui);
        let check = match self.checksum_at_check {
            Some(sum) => format!("{sum:016x}"),
            None => "not yet".to_owned(),
        };
        let lines = vec![
            format!("FPS {}   frame {:.1} ms", get_fps(), dt * 1000.0),
            format!("Simulation {:.2} ms per tick (budget 50 ms)", self.avg_tick_ms),
            format!("Drawing {:.2} ms per frame (CPU only)", self.avg_draw_ms),
            format!("Zombies {}   units {}   buildings {}", world.zombies.len(), world.units.len(), world.buildings.len()),
            format!("Tick {}   speed x{}{}", world.tick, ui.speed, if ui.paused { "   PAUSED" } else { "" }),
            format!("Seed {}   map {} x {}", world.seed, world.map.size, world.map.size),
            format!("Checksum now  {:016x}", world.checksum()),
            format!("Checksum tick {CHECK_TICK}  {check}"),
            match world.spawn_group() {
                Some(g) => format!("Spawn: {}-{} {:?}s every {:.1} s", g.min, g.max, g.breed, g.every as f32 * TICK),
                None => "Spawn: nothing yet (from the first stage in data\\spawning.json)".to_owned(),
            },
            String::new(),
            format!("Edge scrolling {}   mouse confined {}", on_off(ui.settings.edge_scroll), on_off(ui.settings.confine_mouse)),
            String::new(),
            "Select: click / box, Shift: add or remove".to_owned(),
            "  Ctrl-click, double click: all of this type".to_owned(),
            "  F2: all   Tab: active type (buildings too)".to_owned(),
            "Right click: move / attack zombie / follow".to_owned(),
            "  right drag: move in formation (X line, C box, V circle)".to_owned(),
            "M S H P T (or buttons), Shift: queue, T/P/M + drag: formation".to_owned(),
            "Command center (grid): Q soldier, W ranger, R sniper, T medic,".to_owned(),
            "  S tent, D defense (Q wall, W gate - R turns, E tower), G rally".to_owned(),
            "  (not within 4 tiles of a zombie). Right click / Esc: stop building".to_owned(),
            "Right click a tower with units: they go in. Tower selected: Q unload".to_owned(),
            "Medic: T + click on a unit = beam at it".to_owned(),
            "Groups (units and buildings): Ctrl+number set, Shift+number add,".to_owned(),
            "  Ctrl+Shift+number steal, number select (2x: go)".to_owned(),
            "Camera: WASD (nothing selected), arrows, edge, wheel, minimap,".to_owned(),
            "  Ctrl+F5-F8 save, F5-F8 jump, Backspace: base".to_owned(),
            "  middle mouse drag: move the map".to_owned(),
            "Space pause   +/- speed   F3 this overlay".to_owned(),
            "F10 game view / editor mode (top right)".to_owned(),
            "  editor: click a spawner, drag its edge, Del removes it".to_owned(),
            "F12 keep mouse in window   Esc menu".to_owned(),
        ];
        draw::draw_debug(&lines);
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// Mixes a new measurement into the average at 5 % – outliers hardly count.
fn smooth(average: f64, value: f64) -> f64 {
    average * 0.95 + value * 0.05
}

/// A new, truly random seed. That's allowed because it happens *before* the simulation: the simulation itself only
/// gets the finished number (DESIGN.md 12.8) – in co-op the host picks it and sends it along.
///
/// Where the chance comes from: Rust's `RandomState` (the thing that makes `HashMap` unpredictable for attackers)
/// gets its keys from the operating system's random generator – on Windows the same one used for encryption. Hashing
/// the clock with it gives a number nobody can guess, without an extra library.
fn random_seed() -> u64 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(nanos);
    // Only up to 2^53: such a seed still fits exactly into a JSON number for every program reading a map file.
    hasher.finish() >> 11
}

/// `--seed 12345` on the command line (to repeat a map or a bug, DESIGN.md 12.6).
fn seed_argument() -> Option<u64> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2).find(|pair| pair[0] == "--seed").and_then(|pair| pair[1].parse().ok())
}
