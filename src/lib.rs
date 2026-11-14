//! Last Bastion as a library: all the code of the two programs – the game (`src\main.rs` → `LastBastion.exe`) and
//! the map editor (`src\bin\editor.rs` → `LastBastionEditor.exe`). Both are only a few lines that call
//! `app::run` and say which of the two they are (`app::Program`).
//!
//! `pub mod`: the programs may use what's in there. The other modules are only used inside the library.

pub mod app;
mod aggro;
mod audio;
mod building_sprites;
mod cursor;
mod data;
mod draw;
mod editor;
mod critters;
mod effects;
mod flow;
mod formation;
mod game;
mod grid;
mod ground;
mod home;
mod hud;
mod hunt;
mod iso;
mod keymap;
mod keys;
mod kinds;
mod lure;
mod map;
mod mapfile;
mod mapgen;
mod menu;
mod noise;
mod overlay;
mod picture;
mod picking;
mod placement;
mod post;
mod rail_planner;
mod rng;
mod settings;
mod sim;
mod sprites;
mod train_sprites;
mod story_panel;
mod ui;
mod ui_art;
mod undo;
mod view;
mod vision;
mod widgets;
