//! The real mouse on Windows – macroquad can't do this (DESIGN.md 12.2):
//! it doesn't notice when the mouse leaves the window or the window loses focus,
//! and then simply keeps reporting the last position inside the window.
//!
//! Here we ask Windows directly: does our window have the focus, where is the mouse really?
//! And like StarCraft 2 ("confine cursor to window") the mouse can be held inside the window –
//! only then does it stop at the edge instead of sliding onto the second monitor.
//!
//! Held mouse + a game that stops drawing (a long frame, a new map being made, a hang) = a PC that looks frozen: the
//! mouse can't get out of a window that doesn't move. So a watchdog thread lets go of the mouse as soon as no frame
//! came for `STALL_MS` – the next frame holds it again.
//!
//! Also here: the mouse's side buttons (4 and 5, Windows' "X buttons", often "back" and "forward"). macroquad doesn't
//! report them at all, so they are asked from Windows once per frame (`poll_side_buttons`) and can go on an action
//! like a key (`keys.rs`).

use std::cell::Cell;

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
        win::heartbeat();
        let Some(window) = win::our_foreground_window() else {
            self.release(); // another window in front (e.g. Alt+Tab): release the mouse right away
            return None;
        };
        let (w, h) = win::client_size(window);
        let inside = win::cursor_in_client(window).filter(|&(x, y)| x >= 0 && y >= 0 && x < w && y < h);
        // Only hold the mouse once it is inside. On the title bar or the frame (moving or resizing the window) it stays
        // free – otherwise Windows pushes it down into the window area, the dragged window follows it down, the mouse
        // gets pushed again, and so on: the window slides down and can't be pulled up again.
        if confine && inside.is_some() {
            win::clip_to_client(window);
            self.clipped = true;
        } else {
            self.release();
        }
        let (x, y) = inside?;
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

/// Where the mouse is inside our window, if it is – like `Cursor::update`, but without holding it. For the mouse
/// pointer drawn by the game (`ui_art.rs`): outside the window, or with another window in front, Windows draws its own.
pub fn inside_window() -> Option<Vec2> {
    let window = win::our_foreground_window()?;
    let (w, h) = win::client_size(window);
    let (x, y) = win::cursor_in_client(window).filter(|&(x, y)| x >= 0 && y >= 0 && x < w && y < h)?;
    Some(vec2(x as f32, y as f32) / macroquad::window::screen_dpi_scale())
}

/// The two side buttons: held now, and pressed since the last frame.
#[derive(Clone, Copy, Default)]
struct Side {
    down: bool,
    pressed: bool,
}

thread_local! {
    /// Mouse 4 and 5, as of this frame (`poll_side_buttons`). Only the main thread reads input – `thread_local` and a
    /// `Cell` are enough, no lock.
    static SIDE: Cell<[Side; 2]> = const { Cell::new([Side { down: false, pressed: false }; 2]) };
}

/// Once per frame, before any input is read: asks Windows for mouse 4 and 5. Only while our window is in front – a
/// "back" click in the browser next to the game is none for the game.
pub fn poll_side_buttons() {
    let front = win::our_foreground_window().is_some();
    let old = SIDE.get();
    let mut new = [Side::default(); 2];
    for (i, side) in new.iter_mut().enumerate() {
        // Asked even in the background: that clears Windows' "pressed since last time", so an old click doesn't count
        // the moment the game comes back to the front.
        let (down, tapped) = win::side_button(i);
        if front {
            side.down = down;
            // `tapped`: pressed and let go between two frames – a macro of the mouse's software can be that quick.
            side.pressed = (down && !old[i].down) || (tapped && !old[i].down);
        }
    }
    SIDE.set(new);
}

/// Is side button `i` (0 = mouse 4, 1 = mouse 5) held right now?
pub fn side_down(i: usize) -> bool {
    SIDE.get()[i].down
}

/// Was side button `i` pressed in this frame?
pub fn side_pressed(i: usize) -> bool {
    SIDE.get()[i].pressed
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
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Once, OnceLock};
    use std::time::{Duration, Instant};

    /// No frame for this long while the mouse is held: let go of it (see the top of the file).
    const STALL_MS: u64 = 400;

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
        fn GetAsyncKeyState(key: i32) -> i16;
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
        HELD.store(true, Ordering::Relaxed);
        heartbeat();
        start_watchdog();
    }

    pub fn unclip() {
        HELD.store(false, Ordering::Relaxed);
        unsafe { ClipCursor(std::ptr::null()) };
    }

    /// Is the mouse held by us right now? Read by the watchdog.
    static HELD: AtomicBool = AtomicBool::new(false);
    /// When the last frame came – milliseconds since `START`.
    static BEAT: AtomicU64 = AtomicU64::new(0);
    static START: OnceLock<Instant> = OnceLock::new();

    fn now_ms() -> u64 {
        START.get_or_init(Instant::now).elapsed().as_millis() as u64
    }

    /// Every frame: "still drawing".
    pub fn heartbeat() {
        BEAT.store(now_ms(), Ordering::Relaxed);
    }

    /// The watchdog (started with the first hold): a thread of its own, so it still runs while the game is stuck.
    /// `ClipCursor` works from any thread of the program.
    fn start_watchdog() {
        static STARTED: Once = Once::new();
        STARTED.call_once(|| {
            let _ = std::thread::Builder::new().name("mouse watchdog".into()).spawn(|| loop {
                std::thread::sleep(Duration::from_millis(100));
                if HELD.load(Ordering::Relaxed) && now_ms().saturating_sub(BEAT.load(Ordering::Relaxed)) > STALL_MS {
                    unclip();
                }
            });
        });
    }

    /// Side button `i` (0 = mouse 4, 1 = mouse 5): (held now, pressed since the last call).
    pub fn side_button(i: usize) -> (bool, bool) {
        const VK_XBUTTON1: i32 = 0x05;
        let state = unsafe { GetAsyncKeyState(VK_XBUTTON1 + i as i32) } as u16;
        (state & 0x8000 != 0, state & 1 != 0)
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
    pub fn heartbeat() {}
    pub fn side_button(_: usize) -> (bool, bool) {
        (false, false)
    }
}
