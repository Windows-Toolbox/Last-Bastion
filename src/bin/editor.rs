// In the release build don't open a black console window next to the editor.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The map editor: `LastBastionEditor.exe` (`cargo run --bin editor`). Opens a map file from
//! `%APPDATA%\Last Bastion\maps\` – also one given on the command line, like the game's "Edit" button does – changes
//! it, tests it and saves it again. All of its code is in the library (`src\lib.rs`), shared with the game.

use last_bastion::app::{self, Program};
use macroquad::window::Conf;

fn window_conf() -> Conf {
    app::window(Program::Editor)
}

#[macroquad::main(window_conf)]
async fn main() {
    app::run(Program::Editor).await;
}
