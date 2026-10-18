// In the release build don't open a black console window next to the game.
// In the debug build the console stays, so println! output is visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The game: `LastBastion.exe` (`cargo run`). All of its code is in the library (`src\lib.rs`) – the map editor
//! (`src\bin\editor.rs`) shares it.

use last_bastion::app::{self, Program};
use macroquad::window::Conf;

fn window_conf() -> Conf {
    app::window(Program::Game)
}

#[macroquad::main(window_conf)]
async fn main() {
    app::run(Program::Game).await;
}
