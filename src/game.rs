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
use crate::editor::{EditorRequest, MapInfo, Mode, Sight};
use crate::home;
use crate::hud::{self, Button, Layout};
use crate::iso::Camera;
use crate::mapfile::{self, MapFile};
use crate::mapgen::MapSetup;
use crate::menu::{Menu, MenuAction};
use crate::overlay::{self, Action, Overlay, Switch};
use crate::picture::{Extras, MapPicture};
use crate::settings::Settings;
use crate::sim::{day_and_hour, Breed, Notice, Order, Outcome, World, TICK};
use crate::ground::Landscape;
use crate::sprites::Sprites;
use crate::ui::Ui;
use crate::undo::History;
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
    /// Editor: undo and redo – the orders of the last steps in Edit mode (`undo.rs`).
    history: History,
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
    // Zombies that leave your sight fade out instead of vanishing, and turn the way they walk.
    looks: draw::Looks,
    /// The fog of war as a picture, painted anew every frame (`draw.rs`).
    fog_picture: draw::FogPicture,
    /// The pictures of zombies and units – read again on "Restart" (`restart`).
    sprites: Sprites,
    /// TAB's landscape pictures (`ground.rs`).
    landscape: Landscape,
    /// The picture on the minimap – painted only when the map or the fog changed (`picture.rs`).
    minimap: MapPicture,
    /// The end screen was put away ("Look at the map") – the round stays over.
    end_hidden: bool,
    /// Editor: what to do once the question about unsaved changes is answered (`unsaved_dialog`).
    leaving: Option<GameAction>,
    /// The windows of the map's scripts still to be read (TAB's "Display Window Message") – the oldest in front; the
    /// game is paused while one is up. And whether the player has just picked an option of a script's choice – the
    /// order is on its way, the window waits for it (`story_dialog`).
    windows: Vec<(String, String)>,
    chosen: bool,
    /// Editor: the window's X was clicked (`app.rs`).
    quit_asked: bool,
}

impl Game {
    /// A new map from this setup and seed – not written anywhere yet (the game does that right after, `app.rs`). It
    /// goes through its map file at once (`from_world` → `to_world`), so the round starts exactly as it will after
    /// "Restart" or on the other PC in co-op.
    pub fn generate(seed: u64, setup: &MapSetup, settings: Settings, program: Program) -> Game {
        let world = World::generate(seed, setup);
        // Named after the picked map ("Green Valley 1"), or the generator if it was set up by hand ("Wild 1").
        let name = if setup.map.is_empty() { setup.generator.name() } else { &setup.map };
        let mut file = MapFile::from_world(&world, &mapfile::free_name(name));
        file.setup = Some(setup.clone());
        // `expect` stops the program with this text if it's an `Err`. It can't be one here: `from_world` writes
        // exactly what `to_world` reads.
        Game::open(file, None, settings, program).expect("a map just made from a world always loads")
    }

    /// A map from its file – to play (game) or to edit (editor). `Err` if the file doesn't add up.
    pub fn open(file: MapFile, path: Option<PathBuf>, settings: Settings, program: Program) -> Result<Game, String> {
        let world = match program {
            Program::Game => file.world_to_play(crate::app::random_seed())?,
            Program::Editor => file.to_world()?,
        };
        let mut ui = Ui::new(settings, program == Program::Editor);
        ui.editor.map_name = file.name.clone();
        ui.editor.info = MapInfo::of(&file);
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
            history: History::new(),
            orders: Vec::new(),
            accumulator: 0.0,
            checksum_at_check: None,
            avg_tick_ms: 0.0,
            avg_draw_ms: 0.0,
            zombie_view_tick: None,
            looks: draw::Looks::default(),
            fog_picture: draw::FogPicture::new(),
            sprites: Sprites::default(),
            landscape: Landscape::default(),
            minimap: MapPicture::new(),
            end_hidden: false,
            leaving: None,
            windows: Vec::new(),
            chosen: false,
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

        // A question, a script's window or choice, or the end screen in front: only their buttons count – no Esc menu, no
        // input on the map, no time passing (TAB pauses for them too).
        let story = !self.windows.is_empty() || (!self.world.choices.is_empty() && !self.chosen);
        let modal = self.leaving.is_some() || self.end_screen_shown() || story;
        // The editor's panel first (egui): what it takes – a click, a field being typed into – the map and the keys
        // below don't get. Greyed out while the menu or a question is in front.
        // Not while the window is minimized: it has no size then, so there's no room for the panel (egui would panic).
        if self.ui.editor.available && screen_width() >= 1.0 && screen_height() >= 1.0 {
            let enabled = !self.menu.open && !modal;
            self.ui.editor.steps = self.history.counts();
            // Three fields of `self` lent apart: the closure gets the editor, the world and the orders – nothing else.
            let (editor, world, orders) = (&mut self.ui.editor, &self.world, &mut self.orders);
            egui_macroquad::ui(|ctx| editor.show(ctx, world, enabled, orders));
        }
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
                EditorRequest::Undo => self.undo(false),
                EditorRequest::Redo => self.undo(true),
                EditorRequest::Locate(at) => self.camera.center = at,
            }
        }
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if self.program == Program::Editor && ctrl && !self.ui.editor.is_typing() && !modal {
            if is_key_pressed(KeyCode::S) {
                self.save();
            }
            // Ctrl+Z undo, Ctrl+Y (or Ctrl+Shift+Z) redo. macroquad names the keys by their place on a US keyboard
            // (`keys.rs`), where Z and Y are swapped: `KeyCode::Y` is the Z of a German one.
            let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            if is_key_pressed(KeyCode::Y) && !self.menu.open {
                self.undo(shift);
            } else if is_key_pressed(KeyCode::Z) && !self.menu.open {
                self.undo(true);
            }
        }
        if story && !self.end_screen_shown() {
            self.story_dialog();
        }
        // The pick went through: the next choice may show.
        if self.world.choices.is_empty() || !self.orders.iter().any(|o| matches!(o, Order::Choose { .. })) {
            self.chosen = false;
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
        let sim_start = Instant::now();
        let mut ticks = 0;
        if runs {
            self.accumulator += dt.min(0.25) * self.ui.speed as f32;
            while self.accumulator >= TICK && ticks < MAX_TICKS_PER_FRAME {
                self.world.step(&self.orders);
                self.looks.effects.take(&self.world); // what only the picture shows (`effects.rs`)
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
            if !self.orders.is_empty() {
                // Paused (Space) or editing: take the orders over right away – you see the lines, queues and rally
                // points, the painted terrain, the placed zombies – but no time passes, so nobody starts moving.
                if editing {
                    // And remembered for undo: one step lasts as long as the left mouse button is held (`undo.rs`).
                    self.history.apply(&mut self.world, &self.orders, is_mouse_button_down(MouseButton::Left));
                    self.zombie_view_tick = None;
                } else {
                    self.world.apply_orders(&self.orders);
                }
                self.orders.clear();
            }
            if !is_mouse_button_down(MouseButton::Left) {
                self.history.close();
            }
        }
        if ticks > 0 {
            let tick_ms = sim_start.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
            self.avg_tick_ms = smooth(self.avg_tick_ms, tick_ms);
        }
        self.follow_size();
        self.announce_events();
    }

    /// The map got another size (editor: "Map size", or undo / redo of it): what is as big as the map starts anew –
    /// and what names units, buildings and zombies by their number, because the map was loaded again (`World::resize`)
    /// and they got new ones.
    fn follow_size(&mut self) {
        if self.zombie_view.size() == self.world.map.size {
            return;
        }
        self.zombie_view = Vision::new(self.world.map.size);
        self.zombie_view_tick = None;
        self.looks = draw::Looks::default();
        self.ui.forget_selection();
    }

    /// What the events had to say in the last ticks (`World::notices`): at the top of the screen, like TAB's – how many
    /// (dozens, a horde, a swarm, billions) and from where, when it's announced; that it's here, when it comes.
    fn announce_events(&mut self) {
        for notice in std::mem::take(&mut self.world.notices) {
            let text = match notice {
                Notice::Coming { at, zombies, sides, final_wave, .. } => {
                    let (day, hour) = day_and_hour(at);
                    let what = if final_wave {
                        "BILLIONS OF INFECTED are coming - every zombie on the map with them"
                    } else if zombies < 150 {
                        "Dozens of infected are coming"
                    } else if zombies < 1500 {
                        "A horde of infected is coming"
                    } else {
                        "A swarm of infected is coming"
                    };
                    format!("{what}{} - day {day}, {hour:02}:00.", from_sides(&sides))
                }
                Notice::Came { zombies, final_wave: true, .. } => format!("THE FINAL WAVE is here - {zombies} zombies!"),
                Notice::Came { zombies, .. } => format!("The swarm is here - {zombies} zombies!"),
                Notice::Joined { name } => format!("{name} joins the colony!"),
                Notice::Message { text } => text,
                // A window: it pauses the game until it's closed (TAB's "Display Window Message").
                Notice::Window { title, text } => {
                    self.windows.push((title, text));
                    continue;
                }
            };
            self.ui.alert(&text);
        }
    }

    /// The game: the same map again from the start – read anew from its file, so a change made in the editor counts
    /// (and the values in `data\` too). The editor: back to the map as it was saved ("Revert to saved").
    fn restart(&mut self) {
        data::reload();
        self.sprites = Sprites::default(); // changed pictures count too
        self.landscape = Landscape::default();
        if let Some(file) = self.path.as_ref().and_then(|path| MapFile::load(path).ok()) {
            self.file = file;
        }
        let world = match self.program {
            Program::Game => self.file.world_to_play(crate::app::random_seed()),
            Program::Editor => self.file.to_world(),
        };
        match world {
            Ok(world) => {
                self.ui.editor.map_name = self.file.name.clone();
                self.ui.editor.info = MapInfo::of(&self.file);
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
        self.history = History::new();
        self.orders.clear();
        self.accumulator = 0.0;
        self.checksum_at_check = None;
        self.zombie_view_tick = None;
        self.looks = draw::Looks::default(); // the new world numbers its zombies from 1 again
        self.end_hidden = false;
    }

    /// Editor (Edit): the last step undone (Ctrl+Z) – or with `redo` the one undone last done again (Ctrl+Y).
    fn undo(&mut self, redo: bool) {
        if !self.ui.editor.editing() || self.ui.editor.testing() {
            return;
        }
        let done = if redo { self.history.redo(&mut self.world) } else { self.history.undo(&mut self.world) };
        if done {
            // A spawner placed in the step taken back is gone – and with it the selection.
            self.ui.editor.select_spawner(None);
            self.zombie_view_tick = None;
            self.looks = draw::Looks::default();
            self.follow_size();
        }
    }

    /// The map in the editor as it is now – while a test runs, the one put aside (not the test).
    fn edited(&self) -> &World {
        self.stash.as_ref().unwrap_or(&self.world)
    }

    /// What the map's file would hold if it were saved now. `..`: all other fields from `from_world`. Whether it's a
    /// custom level stays as the file says until it's saved (`write_file`).
    fn current_file(&self) -> MapFile {
        let info = &self.ui.editor.info;
        MapFile {
            setup: self.file.setup.clone(),
            custom: self.file.custom,
            author: info.author.clone(),
            description: info.description.clone(),
            picture: info.picture.clone(),
            regenerate: info.regenerate && self.file.setup.is_some(),
            ..MapFile::from_world(self.edited(), &self.ui.editor.map_name)
        }
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
        self.looks = draw::Looks::default();
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
    /// Saved in the editor, a map is a custom level (`MapFile::custom`) – also the file of a round that was opened here.
    fn write_file(&mut self) -> Result<String, String> {
        let file = MapFile { custom: true, ..self.current_file() };
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

    /// A window of the map's story in front of everything: a message to read (OK, Enter or Esc closes it), or a choice –
    /// one button per option; the pick goes to the simulation as an order (`Order::Choose`), so it counts the same on
    /// every PC.
    fn story_dialog(&mut self) {
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.5));
        if let Some((title, text)) = self.windows.first().cloned() {
            let mut col = Column::centered(&title, 4, 620.0);
            col.wrap(&text);
            col.gap();
            if button(col.row(), "OK", true) || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) {
                self.windows.remove(0);
            }
            return;
        }
        let Some(choice) = self.world.choices.first().cloned() else {
            return;
        };
        let mut col = Column::centered("Your choice", choice.options.len() + 3, 620.0);
        col.wrap(&choice.text);
        col.gap();
        for (index, (label, _)) in choice.options.iter().enumerate() {
            if button(col.row(), label, true) {
                self.orders.push(Order::Choose { index });
                self.chosen = true;
            }
        }
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
            Outcome::Won { .. } => ("Victory", "The colony survived."),
            Outcome::Lost { .. } => ("Defeat", "The command center has fallen."),
        };
        let mut col = Column::centered(title, 10, 500.0);
        col.note(line);
        let (day, hour) = day_and_hour(outcome.tick());
        let s = self.world.score;
        col.note(&format!("Day {day} of {}, {hour:02}:00", self.world.days));
        col.note(&format!("Zombies killed: {}", s.zombies_killed));
        col.note(&format!("Houses of doom destroyed: {}   loot found: {}", s.doom_razed, s.loot_found));
        col.note(&format!(
            "Units trained: {}   lost: {}   deserted: {}   dismissed: {}",
            s.units_trained, s.units_lost, s.units_deserted, s.units_dismissed
        ));
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
        clear_background(draw::BACKGROUND);
        let alpha = (self.accumulator / TICK).min(1.0);
        // Game time in seconds, like the animations in `draw.rs` count it.
        self.looks.update(world, dt, (world.tick as f32 + alpha) * TICK);
        self.sprites.load_missing();
        let pictures = self.landscape.refresh(&world.map, &world.look);
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
        let area = self.ui.view_area(world);
        if !matches!(fog, Fog::Off) {
            self.fog_picture.paint(world, &fog, area);
        }
        let view = draw::View {
            area,
            editing: self.ui.editor.editing(),
            full_plan: self.ui.settings.always_full_queue
                || is_key_down(KeyCode::LeftShift)
                || is_key_down(KeyCode::RightShift),
            always_bars: self.ui.settings.always_health_bars,
            grid: self.ui.settings.show_grid,
            fog,
            fog_picture: &self.fog_picture,
            looks: &self.looks,
            sprites: &self.sprites,
            landscape: pictures.then_some(&self.landscape),
            spawners: self.ui.spawner_shapes(world, &self.camera),
        };
        draw::draw_world(world, &self.camera, &view, &self.ui.selected, alpha);
        self.ui.draw(world, &self.camera);
        // The minimap's picture: with fog only in the player's view – in the editor's other views it stays readable.
        let fog = (self.ui.sight() == Sight::Player).then_some(&world.vision);
        self.minimap.refresh(&world.map, &Extras { fog, spawners: &[], base: false, shade_ring: false }, dt);
        hud::draw(&Layout::new(), world, &self.camera, &self.ui, &self.minimap);
        // The map's goals (its story) – not over the stats of F3.
        if !self.ui.settings.dev_overlay {
            hud::draw_goals(world);
        }
        // A broken file in `data\`: the game runs on with the old values – but you have to see it.
        // The same for a picture in `assets\`: that kind stays a box.
        let problems = [
            data::problem().map(|p| format!("data\\ not loaded, old values in use - {p}")),
            self.sprites.problem().map(|p| format!("Picture not loaded, drawn as a box - {p}")),
            self.landscape.problem().map(|p| format!("Landscape pictures not loaded - {p}")),
            self.looks.effects.problem().map(|p| format!("Effect not loaded - {p}")),
        ];
        for (row, text) in problems.into_iter().flatten().enumerate() {
            let y = 40.0 + row as f32 * 30.0;
            draw_rectangle(8.0, y, measure_text(&text, None, 18, 1.0).width + 16.0, 26.0, Color::new(0.4, 0.05, 0.05, 0.9));
            draw_text(&text, 16.0, y + 18.0, 18.0, WHITE);
        }
        // The editor's panel over the map and the bottom panel – the menu and the questions come later, on top of it.
        if self.ui.editor.available {
            egui_macroquad::draw();
        }
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
            .text(format!(
                "Zombies {} awake, {} asleep   units {}   buildings {}",
                world.zombies.len(),
                world.sleep.len(),
                world.units.len(),
                world.buildings.len()
            ))
            .text(format!(
                "{} more can come out of {} houses of doom, {} asleep around {} loot",
                world.doom.iter().map(|d| d.inside).sum::<u32>(),
                world.doom.len(),
                world.loot.iter().map(|l| l.guards).sum::<u32>(),
                world.loot.len()
            ))
            .text(match world.loudest_sound() {
                Some((at, level)) => format!(
                    "Loudest noise {level} at ({:.0}, {:.0}) - a walker 10 tiles away comes with {}%",
                    at.x,
                    at.y,
                    Breed::Walker.hears(level, 10).min(100)
                ),
                None => "Quiet everywhere".to_owned(),
            })
            .text(format!("Tick {}   speed x{} (+/-)", world.tick, ui.speed))
            .button(format!("Seed {}   map {} x {}   (click: copy)", world.seed, world.map.size, world.map.size), Action::CopySeed)
            .text(format!("Checksum now  {:016x}", world.checksum()))
            .text(format!("Checksum tick {CHECK_TICK}  {check}"))
            .text(match world.events.iter().filter_map(|e| e.next_hour().map(|h| (h, e))).min_by_key(|&(h, _)| h) {
                Some((h, e)) => format!("Next event: {} on day {}, {:02}:00 ({} times so far)", e.name, h / 24 + 1, h % 24, e.fired()),
                None => "Next event: none".to_owned(),
            })
            .text(format!(
                "Unrest {} - every side of the stragglers +{} (one more per {})",
                world.unrest,
                world.unrest_zombies(),
                data::get().sound.unrest_per_zombie
            ));
        // The keys as they are set in the key mapping (`keys.rs`) – not the defaults.
        use crate::keys::Action as K;
        let keys = &ui.settings.keys;
        let k = |action: K| keys.label(action);
        let card = |button: Button| keys.label(K::of(button));
        o.section("switches", "Switches")
            .switch(&format!("Pause ({})", k(K::Pause)), ui.paused, Switch::Pause)
            .switch("Edge scrolling", ui.settings.edge_scroll, Switch::EdgeScroll)
            .switch(&format!("Keep the mouse in the window ({})", k(K::ConfineMouse)), ui.settings.confine_mouse, Switch::ConfineMouse)
            .switch("Health bars on all units", ui.settings.always_health_bars, Switch::HealthBars)
            .switch("Whole command queue (else: hold Shift)", ui.settings.always_full_queue, Switch::FullQueue)
            .switch(&format!("Grid view ({})", k(K::GridView)), ui.settings.show_grid, Switch::Grid);
        o.section("select", "Select")
            .text("Click / box, Shift: add or remove, on nothing or Esc: none")
            .text("Click a zombie: its values")
            .text("Ctrl-click, double click: all of this type")
            .text(format!("{}: all   {}: active type (buildings too)", k(K::SelectArmy), k(K::NextType)));
        let (line, block, ring) = (card(Button::FormLine), card(Button::FormBox), card(Button::FormCircle));
        let (go, stop, hold, patrol, attack) =
            (card(Button::Move), card(Button::Stop), card(Button::Hold), card(Button::Patrol), card(Button::Attack));
        o.section("commands", "Commands")
            .text("Right click: move / attack zombie / follow")
            .text(format!("  right drag: move in formation ({line} line, {block} box, {ring} circle)"))
            .text(format!("{go} {stop} {hold} {patrol} {attack} (or buttons), {attack}/{patrol}/{go} + drag: formation"))
            .text("Shift: queue - let go: the command leaves the mouse,")
            .text("  press Shift again: it's back")
            .text(format!("Medic: {attack} + click on a unit = beam at it"));
        o.section("buildings", "Buildings").text("Command center: a build menu, then the building:");
        for menu in hud::build_menus() {
            let buildings: Vec<String> =
                hud::menu_buttons(menu).into_iter().filter(|b| b.places().is_some()).map(|b| format!("{} {}", card(b), b.name())).collect();
            if !buildings.is_empty() {
                for text in wrap_list(format!("  {} {}:", card(Button::Open(menu)), Button::Open(menu).name()), &buildings) {
                    o.text(text);
                }
            }
        }
        o.text(format!("  {} turns the gate. Costs gold, wood, stone, workers; most need", k(K::TurnGate)))
            .text("  power - inside the blue circles; not within 4 tiles of a zombie,")
            .text("  pushes your units aside. Right click / Esc: stop")
            .text(format!("  {}, Esc or select it again (click, group key): back to its main buttons", card(Button::Back)));
        let barracks: Vec<String> = hud::BARRACKS_BUTTONS
            .into_iter()
            .filter(|b| b.trains().is_some() || *b == Button::Rally)
            .map(|b| format!("{} {}", card(b), b.name()))
            .collect();
        for text in wrap_list("Barracks:".to_owned(), &barracks) {
            o.text(text);
        }
        o.text("  (hold the key: again and again, Ctrl: 5 at once, Shift: on credit)")
            .text(format!("Right click a tower with units: they go in. Tower selected: {} unload", card(Button::Unload)))
            .text(format!("Any building: {} repair (gold, all selected) - cures infected ones", card(Button::Repair)))
            .text(format!("{} (no units selected, or the button in the stock): repair all", k(K::RepairAll)))
            .text("  cheapest first, not with a zombie within 4 tiles");
        let camera = [K::CameraUp, K::CameraLeft, K::CameraDown, K::CameraRight].map(k).join(", ");
        let spots: Vec<String> = (0..4).map(|b| k(K::Bookmark(b))).collect();
        o.section("groups", "Groups and camera")
            .text("Groups (units and buildings): Ctrl+number set, Shift+number add,")
            .text("  Ctrl+Shift+number steal, number select (2x: go)")
            .text(format!("Camera: {camera}"))
            .text("  (letters only with nothing selected), edge, wheel, minimap,")
            .text(format!("  {}: jump (Ctrl: save), {}: base", spots.join(" "), k(K::Base)))
            .text("  middle mouse drag: move the map");
        o.section("keys", "Other keys")
            .text(format!("{} pause   {} / {} speed   {} this overlay", k(K::Pause), k(K::Faster), k(K::Slower), k(K::Overlay)))
            .text(format!("{} keep mouse in window   Esc menu - all keys: Key mapping", k(K::ConfineMouse)));
        if self.program == Program::Editor {
            o.text(format!("{} edit / test (top right)   Ctrl+S save", k(K::EditTest))).text("Click a spawner, drag its edge, Del removes it");
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
            Action::Switch(Switch::Grid) => s.show_grid = !s.show_grid,
            Action::CopySeed => {
                macroquad::miniquad::window::clipboard_set(&self.world.seed.to_string());
                self.ui.alert("Seed copied");
            }
        }
    }
}

/// A list for the overlay ("  D Defense: Q Wall, W Gate, …") in lines short enough for it – the later ones indented.
fn wrap_list(head: String, items: &[String]) -> Vec<String> {
    const LONGEST: usize = 70;
    let mut lines = Vec::new();
    let mut line = head;
    for (i, item) in items.iter().enumerate() {
        let item = if i + 1 < items.len() { format!("{item},") } else { item.clone() };
        if line.chars().count() + 1 + item.chars().count() > LONGEST && !line.trim().is_empty() {
            lines.push(line);
            line = "     ".to_owned();
        }
        line = format!("{line} {item}");
    }
    lines.push(line);
    lines
}

/// Mixes a new measurement into the average at 5 % – outliers hardly count.
fn smooth(average: f64, value: f64) -> f64 {
    average * 0.95 + value * 0.05
}

/// " from the north" / " from the north and the east" – for an announcement, like TAB's (only for one or two sides).
fn from_sides(sides: &[u32]) -> String {
    let name = |side: &u32| match side {
        0 => "the north",
        1 => "the south",
        2 => "the west",
        _ => "the east",
    };
    match sides {
        [one] => format!(" from {}", name(one)),
        [a, b] => format!(" from {} and {}", name(a), name(b)),
        _ => String::new(),
    }
}
