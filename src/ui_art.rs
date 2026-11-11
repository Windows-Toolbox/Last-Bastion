//! TAB's interface pictures in `assets\ui\` (made by `tools\tab_ui.py`, explained in `assets\README.md`): the bar at
//! the bottom, its buttons, the icons, the portraits, the events' pictures and the mouse pointers.
//!
//! Every picture is read the first time someone asks for it and then kept. One that isn't there is `None` – whoever
//! draws it then draws its own placeholder, like before (the game runs without TAB's pictures too).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use macroquad::prelude::*;

use crate::data;
use crate::sprites::smooth;

// `thread_local!`: a value per thread – macroquad draws on one thread only, so these are simply "the" cache. Like a
// module-level dict in Python; `RefCell` lets us change it although the `static` itself can't be `mut`
// (it checks the borrowing rules while the program runs instead of when it's compiled).
thread_local! {
    static TEXTURES: RefCell<HashMap<String, Option<Texture2D>>> = RefCell::new(HashMap::new());
    /// The bar's picture once more, on the CPU side: where it's see-through (`Layout::covers` in `hud.rs`).
    static PANEL: RefCell<Option<Option<Image>>> = const { RefCell::new(None) };
    /// The mouse pointer of this frame – `ui.rs` picks it, `app.rs` draws it last.
    static POINTER: Cell<Pointer> = const { Cell::new(Pointer::Arrow) };
}

/// A picture of `assets\ui\` by its path without `.png` (`"panel"`, `"icons/IconAttack"`, `"portraits/Soldier"`).
/// `Texture2D` is only a handle to the picture on the graphics card – cloning it copies the handle, not the picture.
pub fn get(name: &str) -> Option<Texture2D> {
    cached(&format!("ui/{name}"))
}

/// Our own logo (`assets\logo\last_bastion.png`, made by `tools\make_logo.py`) – not TAB's, so it's in git.
pub fn logo() -> Option<Texture2D> {
    cached("logo/last_bastion")
}

/// A picture of `assets\` by its path without `.png`, read once and kept.
fn cached(path: &str) -> Option<Texture2D> {
    TEXTURES.with(|textures| textures.borrow_mut().entry(path.to_owned()).or_insert_with(|| load(path).map(|(t, _)| t)).clone())
}

/// Read anew next time (after `tools\tab_ui.py` made the pictures again – "Restart" in the menu).
pub fn forget() {
    TEXTURES.with(|textures| textures.borrow_mut().clear());
    PANEL.with(|panel| *panel.borrow_mut() = None);
}

/// `path`: inside `assets\`, without `.png` (`"ui/panel"`).
fn load(path: &str) -> Option<(Texture2D, Image)> {
    let path = data::folder("assets")?.join(format!("{path}.png"));
    let bytes = std::fs::read(path).ok()?;
    let image = Image::from_file_with_format(&bytes, None).ok()?;
    let texture = Texture2D::from_image(&image);
    smooth(&texture);
    Some((texture, image))
}

/// Is the bar's picture solid at this point (0..1 across and down)? `None`: there is no picture.
pub fn panel_solid(u: f32, v: f32) -> Option<bool> {
    PANEL.with(|panel| {
        let mut panel = panel.borrow_mut();
        let image = panel.get_or_insert_with(|| load("ui/panel").map(|(_, image)| image)).as_ref()?;
        let x = ((u * image.width as f32) as u32).min(image.width as u32 - 1);
        let y = ((v * image.height as f32) as u32).min(image.height as u32 - 1);
        Some(image.get_pixel(x, y).a > 0.1)
    })
}

/// TAB's mouse pointers (`ZXTypeOfCursor`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pointer {
    Arrow,
    /// An enemy under the mouse, or a command waiting for its target.
    Target,
    /// Can't go there (a full tower).
    Invalid,
    /// Into a tower.
    Enter,
}

impl Pointer {
    fn file(self) -> &'static str {
        match self {
            Pointer::Arrow => "cursors/CursorArrow",
            Pointer::Target => "cursors/CursorTarget",
            Pointer::Invalid => "cursors/CursorInvalid",
            Pointer::Enter => "cursors/CursorEnter",
        }
    }

    /// Where on the picture the mouse really points (0..1 across and down) – TAB's `ZXPalette`.
    fn tip(self) -> Vec2 {
        match self {
            Pointer::Arrow => vec2(0.05, 0.0),
            Pointer::Enter => vec2(0.2, 0.5),
            _ => vec2(0.5, 0.5),
        }
    }
}

/// This frame's pointer – the last call wins. `app.rs` sets it back to the arrow every frame.
pub fn set_pointer(pointer: Pointer) {
    POINTER.with(|p| p.set(pointer));
}

/// Draws the mouse pointer at the mouse, in front of everything – TAB's picture instead of Windows' arrow. Without
/// the pictures Windows' arrow stays – and outside the window, or with another window in front.
pub fn draw_pointer() {
    let pointer = POINTER.with(|p| p.replace(Pointer::Arrow));
    let mouse = crate::cursor::inside_window();
    let texture = get(pointer.file()).or_else(|| get(Pointer::Arrow.file()));
    let (Some(texture), Some(mouse)) = (texture, mouse) else {
        show_mouse(true);
        return;
    };
    show_mouse(false);
    // TAB's size: 26 of its 1280 virtual pixels (`ZXPalette`, `DXScene.ScreenVirtualUnit`) – as big as the bar's
    // buttons grow with the window.
    let side = 26.0 * crate::hud::scale();
    let size = vec2(side, side * texture.height() / texture.width());
    let corner = mouse - pointer.tip() * size;
    set_default_camera();
    draw_texture_ex(&texture, corner.x, corner.y, WHITE, DrawTextureParams { dest_size: Some(size), ..Default::default() });
}
