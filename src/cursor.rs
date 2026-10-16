//! The real mouse on Windows – macroquad can't do this (DESIGN.md 12.2):
//! it doesn't notice when the mouse leaves the window or the window loses focus,
//! and then simply keeps reporting the last position inside the window.
//!
//! Here we ask Windows directly: does our window have the focus, where is the mouse really?
//! And like StarCraft 2 ("confine cursor to window") the mouse can be held inside the window –
//! only then does it stop at the edge instead of sliding onto the second monitor.

use macroquad::math::{vec2, Vec2};

pub struct Cursor {
    /// Are we currently holding the mouse? (Then we also have to release it again.)
    clipped: bool,
}

impl Cursor {
    pub fn new() -> Cursor {
        Cursor { clipped: false }
    }

    /// Once per frame. `confine`: hold the mouse inside the window while the window has the focus.
    /// Returns the mouse position inside the window (in the same units as `mouse_position()`) –
    /// or `None` if the window has no focus or the mouse is outside.
    pub fn update(&mut self, confine: bool) -> Option<Vec2> {
        let Some(window) = win::our_foreground_window() else {
            self.release(); // another window in front (e.g. Alt+Tab): release the mouse right away
            return None;
        };
        if confine {
            win::clip_to_client(window);
            self.clipped = true;
        } else {
            self.release();
        }
        let (x, y) = win::cursor_in_client(window)?;
        let (w, h) = win::client_size(window);
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        // Windows counts in screen pixels, macroquad divides by the DPI factor.
        Some(vec2(x as f32, y as f32) / macroquad::window::screen_dpi_scale())
    }

    pub fn release(&mut self) {
        if self.clipped {
            win::unclip();
            self.clipped = false;
        }
    }
}

/// When the `Cursor` is dropped (end of the game), always release the mouse.
impl Drop for Cursor {
    fn drop(&mut self) {
        self.release();
    }
}

/// The Windows functions. `unsafe`: Rust can't check what foreign C code does –
/// here we promise to call it correctly.
#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;

    /// Windows' `POINT` and `RECT`: `#[repr(C)]` lays the fields out in memory exactly like C.
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetForegroundWindow() -> Hwnd;
        fn GetWindowThreadProcessId(window: Hwnd, process_id: *mut u32) -> u32;
        fn GetClientRect(window: Hwnd, rect: *mut Rect) -> i32;
        fn ClientToScreen(window: Hwnd, point: *mut Point) -> i32;
        fn ScreenToClient(window: Hwnd, point: *mut Point) -> i32;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn ClipCursor(rect: *const Rect) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcessId() -> u32;
    }

    /// The window in front – but only if it belongs to our program.
    pub fn our_foreground_window() -> Option<Hwnd> {
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return None;
            }
            let mut process = 0;
            GetWindowThreadProcessId(window, &mut process);
            (process == GetCurrentProcessId()).then_some(window)
        }
    }

    pub fn client_size(window: Hwnd) -> (i32, i32) {
        let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetClientRect(window, &mut rect) };
        (rect.right - rect.left, rect.bottom - rect.top)
    }

    /// Where the mouse is, counted from the top left corner of the window area (may also be outside).
    pub fn cursor_in_client(window: Hwnd) -> Option<(i32, i32)> {
        let mut p = Point { x: 0, y: 0 };
        unsafe {
            if GetCursorPos(&mut p) == 0 || ScreenToClient(window, &mut p) == 0 {
                return None;
            }
        }
        Some((p.x, p.y))
    }

    /// Holds the mouse inside the window area (without title bar and frame).
    pub fn clip_to_client(window: Hwnd) {
        let (w, h) = client_size(window);
        let mut top_left = Point { x: 0, y: 0 };
        unsafe {
            ClientToScreen(window, &mut top_left);
            let rect = Rect { left: top_left.x, top: top_left.y, right: top_left.x + w, bottom: top_left.y + h };
            ClipCursor(&rect);
        }
    }

    pub fn unclip() {
        unsafe { ClipCursor(std::ptr::null()) };
    }
}

/// On other systems: no confining, and we trust macroquad.
#[cfg(not(windows))]
mod win {
    pub type Hwnd = ();
    pub fn our_foreground_window() -> Option<Hwnd> {
        Some(())
    }
    pub fn client_size(_: Hwnd) -> (i32, i32) {
        (i32::MAX, i32::MAX)
    }
    pub fn cursor_in_client(_: Hwnd) -> Option<(i32, i32)> {
        let (x, y) = macroquad::input::mouse_position();
        let scale = macroquad::window::screen_dpi_scale();
        Some(((x * scale) as i32, (y * scale) as i32))
    }
    pub fn clip_to_client(_: Hwnd) {}
    pub fn unclip() {}
}
