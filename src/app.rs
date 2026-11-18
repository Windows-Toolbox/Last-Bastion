//! The two programs – the game (`LastBastion.exe`, from `src\main.rs`) and the map editor (`LastBastionEditor.exe`,
//! from `src\bin\editor.rs`) – share all their code. Those two files only say which one they are. Here is what's left
//! to decide: the window, the loop over the screens (home screen ↔ a map), the command line, and starting the editor
//! from the game.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use macroquad::prelude::*;

use crate::audio;
use crate::cursor;
use crate::data;
use crate::game::{Game, GameAction};
use crate::home::{self, Home, HomeAction};
use crate::mapfile::MapFile;
use crate::mapgen::MapSetup;
use crate::screen;
use crate::settings::{Settings, WindowMode};
use crate::ui_art;

/// Which of the two programs is running.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Program {
    /// `LastBastion.exe`: play.
    Game,
    /// `LastBastionEditor.exe`: open a map file, change it, test it, save it.
    Editor,
}

/// What macroquad asks for before the window opens (`#[macroquad::main(…)]` in the two `main` files).
pub fn window(program: Program) -> Conf {
    let title = match program {
        Program::Game => "Last Bastion",
        Program::Editor => "Last Bastion - Map Editor",
    };
    Conf {
        window_title: title.to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        window_resizable: true,
        fullscreen: Settings::load().window_mode == WindowMode::Borderless,
        ..Default::default()
    }
}

/// What is on the screen: a home screen, or a map – played or edited. An enum whose variants carry data. `Box`: a
/// map is big, this way only a pointer to it sits in the enum.
enum Screen {
    Home(Home),
    Map(Box<Game>),
}

/// The program itself: one turn of the loop per frame, until it's quit. `async`: macroquad's `next_frame().await` waits
/// for the screen's next frame – the loop pauses there until it comes.
pub async fn run(program: Program) {
    data::reload();
    // The window to the monitor of the settings (`screen::frame` does it, once the window is there).
    let start = Settings::load();
    screen::set_target(start.window_mode == WindowMode::Borderless, start.monitor, true);
    // The editor asks before it closes with unsaved changes: the window's X then only asks (`is_quit_requested`),
    // and the map decides (`Game::ask_to_quit`).
    if program == Program::Editor {
        prevent_quit();
    }
    let mut screen = first_screen(program);
    // Was the home screen up last frame? Coming to it, TAB's intro music starts (`audio.rs`).
    let mut was_home = false;
    loop {
        // Mouse 4 and 5 – macroquad doesn't report them, so they're asked from Windows before anything reads input.
        cursor::poll_side_buttons();
        let quit_asked = program == Program::Editor && is_quit_requested();
        // What comes next is decided inside the `match` – but the screen can only be swapped after it, because inside
        // it `screen` is borrowed (`&mut screen`). So the next screen goes into `next` first.
        let mut next: Option<Screen> = None;
        match &mut screen {
            Screen::Home(home) => {
                if quit_asked {
                    break;
                }
                if !was_home {
                    if program == Program::Game { audio::home() } else { audio::quiet() }
                }
                audio::frame(None, &home.settings.volume);
                let before = home.settings.clone();
                let action = home.frame(random_seed);
                home.settings.apply_changes(&before);
                // egui (the editor's panel) keeps every key and click until it's asked for them – also while no map is
                // open. Emptied every frame here, or they would all reach the panel at once when the next map opens.
                if program == Program::Editor {
                    egui_macroquad::ui(ui_art::leave_pointer_to_us);
                    egui_macroquad::draw();
                }
                match action {
                    HomeAction::Nothing => {}
                    HomeAction::Start { seed, setup } => {
                        next = Some(Screen::Map(Box::new(new_map(seed, &setup, home.settings.clone(), program))));
                    }
                    HomeAction::Open { file, path } => {
                        data::reload();
                        match Game::open(file, Some(path), home.settings.clone(), program) {
                            Ok(game) => next = Some(Screen::Map(Box::new(game))),
                            Err(e) => home.message = Some(format!("Couldn't open it: {e}")),
                        }
                    }
                    HomeAction::Quit => break,
                }
            }
            Screen::Map(game) => {
                if quit_asked {
                    game.ask_to_quit();
                }
                let settings = game.ui.settings.clone();
                match game.frame() {
                    GameAction::Nothing => {}
                    GameAction::Home => next = Some(Screen::Home(Home::new(program, settings, random_seed()))),
                    GameAction::Setup => {
                        let mut home = Home::new(program, settings, random_seed());
                        home.open_setup(game.setup(), random_seed());
                        next = Some(Screen::Home(home));
                    }
                    GameAction::NewMap => next = Some(Screen::Map(Box::new(new_map(random_seed(), &game.setup(), settings, program)))),
                    GameAction::Quit => break,
                }
            }
        }
        was_home = matches!(screen, Screen::Home(_));
        if let Some(screen_next) = next {
            screen = screen_next;
        }
        // TAB's mouse pointer, in front of everything (`ui_art.rs`) – picked during the frame, the arrow otherwise.
        ui_art::draw_pointer();
        // Which monitor the window is on (`screen.rs`) – and a borderless one covering it, one row more.
        screen::frame();
        next_frame().await;
    }
}

/// What the program starts with. The game: `--seed 12345` starts that map right away (with the last setup, without a
/// file – to repeat a map or a bug, and for the checksum, README), otherwise the home screen. The editor: a map file
/// given on the command line (the game's "Edit" button does that) opens right away.
fn first_screen(program: Program) -> Screen {
    let settings = Settings::load();
    match program {
        Program::Game => {
            if let Some(seed) = seed_argument() {
                return Screen::Map(Box::new(Game::generate(seed, &home::load_setup(), settings, program)));
            }
        }
        Program::Editor => {
            if let Some(path) = map_argument() {
                // `and_then`: only if loading worked, open it – the first error that comes up is the one we get.
                match MapFile::load(&path).and_then(|file| Game::open(file, Some(path), settings.clone(), program)) {
                    Ok(game) => return Screen::Map(Box::new(game)),
                    Err(e) => {
                        let mut home = Home::new(program, settings, random_seed());
                        home.message = Some(format!("Couldn't open the map: {e}"));
                        return Screen::Home(home);
                    }
                }
            }
        }
    }
    Screen::Home(Home::new(program, settings, random_seed()))
}

/// A new map from this setup and seed. The game starts a round on it and writes its file right away – one game = one
/// file; the editor opens it, and it gets its file when it's saved. The values in `data\` are read anew first.
fn new_map(seed: u64, setup: &MapSetup, settings: Settings, program: Program) -> Game {
    data::reload();
    let mut game = Game::generate(seed, setup, settings, program);
    if program == Program::Game {
        game.write_new_file();
    }
    game
}

/// Starts the map editor that lies next to this program – with a map to open, or without. `Err` says why not.
pub fn launch_editor(map: Option<&Path>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("the game's folder is unknown")?;
    // `dist\LastBastionEditor.exe` after `build.ps1`, `target\debug\editor.exe` after `cargo build`.
    let editor = ["LastBastionEditor.exe", "editor.exe"]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
        .ok_or("The editor isn't next to the game - build it: .\\build.ps1 (or cargo build)")?;
    let mut command = std::process::Command::new(editor);
    if let Some(map) = map {
        command.arg(map);
    }
    command.spawn().map(|_| ()).map_err(|e| format!("Couldn't start the editor: {e}"))
}

/// A new, truly random seed. That's allowed because it happens *before* the simulation: the simulation itself only
/// gets the finished number (DESIGN.md 12.8) – in co-op the host picks it and sends it along.
///
/// Where the chance comes from: Rust's `RandomState` (the thing that makes `HashMap` unpredictable for attackers)
/// gets its keys from the operating system's random generator – on Windows the same one used for encryption. Hashing
/// the clock with it gives a number nobody can guess, without an extra library.
pub fn random_seed() -> u64 {
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

/// The editor: the first word on the command line that isn't an option – the map file to open. (The first entry of
/// `args` is the program itself, `skip(1)` leaves it out.)
fn map_argument() -> Option<PathBuf> {
    std::env::args().skip(1).find(|arg| !arg.starts_with("--")).map(PathBuf::from)
}
