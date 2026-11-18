//! Which monitor the game is on (settings "Monitor", DESIGN.md 11.4) – macroquad can't choose: its borderless
//! fullscreen is always the main monitor, and the window opens wherever Windows likes. Here we ask Windows for the
//! monitors and put the window on the one chosen.
//!
//! The monitors are numbered with the main one first, then the others from left to right (so the number stays the same
//! when the arrangement doesn't change). A number that isn't there (a monitor unplugged) is the main monitor.
//!
//! The settings only say what is wanted (`set_target`); `frame` – once per frame – does it. It can't be done on the spot:
//! macroquad carries out "borderless" or "window" a little later, and what we set before would be undone by it.
//!
//! Borderless: the window covers the monitor and sticks out one row above it. A window exactly as big as the screen is
//! taken for real ("exclusive") fullscreen – Windows and the graphics driver then show it past the desktop compositor,
//! and as soon as any window pops up over it (a screenshot tool, a notification) they switch back: the game blinks and
//! seems to "jump out". One pixel more and they leave it alone (the trick of browsers and many games).

use std::cell::Cell;

/// A monitor, for the settings page.
pub struct Monitor {
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

thread_local! {
    /// Wanted: borderless or window, which monitor.
    static TARGET: Cell<(bool, usize)> = const { Cell::new((false, 0)) };
    /// A window that is still to be put on its monitor (`frame`) – once, so it can be moved by hand afterwards.
    static MOVE: Cell<bool> = const { Cell::new(false) };
}

/// What is wanted now. `move_window`: a window (not borderless) is put onto its monitor as well – once.
pub fn set_target(borderless: bool, monitor: usize, move_window: bool) {
    TARGET.set((borderless, monitor));
    if move_window {
        MOVE.set(true);
    }
}

/// The monitors, the main one first.
pub fn monitors() -> Vec<Monitor> {
    win::rects().into_iter().map(|(r, primary)| Monitor { width: r.right - r.left, height: r.bottom - r.top, primary }).collect()
}

/// Once per frame: keeps a borderless window on its monitor, puts a window on it when asked.
pub fn frame() {
    let (borderless, wanted) = TARGET.get();
    let rects = win::rects();
    let Some(&(monitor, _)) = rects.get(wanted).or(rects.first()) else {
        return;
    };
    let Some(window) = win::our_window() else {
        return;
    };
    match (win::is_borderless(window), borderless) {
        // One row more than the monitor, upwards (see the top of the file). miniquad sets the exact size when it goes
        // borderless – so this checks every frame, not only once.
        (true, true) => win::place(window, monitor.left, monitor.top - 1, monitor.right - monitor.left, monitor.bottom - monitor.top + 1),
        // A window on its way to borderless or back – macroquad hasn't done it yet.
        (true, false) | (false, true) => {}
        (false, false) => {
            if MOVE.get() {
                MOVE.set(false);
                win::center_on(window, &monitor);
            }
        }
    }
}

/// A rectangle of the screen: the monitors', the window's.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    use super::Rect;

    type Hwnd = *mut c_void;
    type Hmonitor = *mut c_void;

    /// Windows' `MONITORINFO`.
    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplayMonitors(dc: *mut c_void, clip: *const Rect, callback: extern "system" fn(Hmonitor, *mut c_void, *mut Rect, isize) -> i32, data: isize) -> i32;
        fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfo) -> i32;
        fn EnumThreadWindows(thread: u32, callback: extern "system" fn(Hwnd, isize) -> i32, data: isize) -> i32;
        fn IsWindowVisible(window: Hwnd) -> i32;
        fn GetWindowLongW(window: Hwnd, index: i32) -> i32;
        fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
        fn SetWindowPos(window: Hwnd, after: Hwnd, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    extern "system" fn collect_monitor(monitor: Hmonitor, _: *mut c_void, _: *mut Rect, data: isize) -> i32 {
        // `data` is the list `rects` lent us.
        let list = unsafe { &mut *(data as *mut Vec<(Rect, bool)>) };
        let mut info = MonitorInfo { size: size_of::<MonitorInfo>() as u32, monitor: Rect { left: 0, top: 0, right: 0, bottom: 0 }, work: Rect { left: 0, top: 0, right: 0, bottom: 0 }, flags: 0 };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            list.push((info.monitor, info.flags & 1 != 0)); // MONITORINFOF_PRIMARY
        }
        1
    }

    /// The monitors with whether each is the main one: the main one first, then left to right, top to bottom.
    pub fn rects() -> Vec<(Rect, bool)> {
        let mut list: Vec<(Rect, bool)> = Vec::new();
        unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), collect_monitor, &mut list as *mut _ as isize) };
        list.sort_by_key(|(r, primary)| (!primary, r.left, r.top));
        list
    }

    extern "system" fn first_visible(window: Hwnd, data: isize) -> i32 {
        let found = unsafe { &mut *(data as *mut Hwnd) };
        if unsafe { IsWindowVisible(window) } != 0 {
            *found = window;
            return 0; // stop
        }
        1
    }

    /// Our window: the first visible one of the thread that made it (macroquad's, the one running the game).
    pub fn our_window() -> Option<Hwnd> {
        let mut found: Hwnd = std::ptr::null_mut();
        unsafe { EnumThreadWindows(GetCurrentThreadId(), first_visible, &mut found as *mut _ as isize) };
        (!found.is_null()).then_some(found)
    }

    /// A window without a frame (`WS_POPUP`, that's how miniquad makes it borderless).
    pub fn is_borderless(window: Hwnd) -> bool {
        const GWL_STYLE: i32 = -16;
        const WS_POPUP: u32 = 0x8000_0000;
        unsafe { GetWindowLongW(window, GWL_STYLE) as u32 & WS_POPUP != 0 }
    }

    fn rect_of(window: Hwnd) -> Rect {
        let mut r = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(window, &mut r) };
        r
    }

    /// The window at this place and size – if it isn't already.
    pub fn place(window: Hwnd, x: i32, y: i32, w: i32, h: i32) {
        const SWP_NOZORDER: u32 = 0x0004;
        const SWP_NOACTIVATE: u32 = 0x0010;
        if rect_of(window) != (Rect { left: x, top: y, right: x + w, bottom: y + h }) {
            unsafe { SetWindowPos(window, std::ptr::null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE) };
        }
    }

    /// A window with a frame into the middle of `monitor` – unless its middle is on it already. Too big for it: as big
    /// as the monitor.
    pub fn center_on(window: Hwnd, monitor: &Rect) {
        let r = rect_of(window);
        let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
        if cx >= monitor.left && cx < monitor.right && cy >= monitor.top && cy < monitor.bottom {
            return;
        }
        let w = (r.right - r.left).min(monitor.right - monitor.left);
        let h = (r.bottom - r.top).min(monitor.bottom - monitor.top);
        let x = monitor.left + (monitor.right - monitor.left - w) / 2;
        let y = monitor.top + (monitor.bottom - monitor.top - h) / 2;
        place(window, x, y, w, h);
    }
}

/// On other systems: one monitor, and we trust macroquad.
#[cfg(not(windows))]
mod win {
    use super::Rect;

    pub type Hwnd = ();

    pub fn rects() -> Vec<(Rect, bool)> {
        let (w, h) = (macroquad::window::screen_width() as i32, macroquad::window::screen_height() as i32);
        vec![(Rect { left: 0, top: 0, right: w, bottom: h }, true)]
    }
    pub fn our_window() -> Option<Hwnd> {
        None
    }
    pub fn is_borderless(_: Hwnd) -> bool {
        false
    }
    pub fn place(_: Hwnd, _: i32, _: i32, _: i32, _: i32) {}
    pub fn center_on(_: Hwnd, _: &Rect) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// There is always a main monitor to go back to, and it comes first.
    #[test]
    fn the_main_monitor_comes_first() {
        let monitors = monitors();
        if !monitors.is_empty() {
            assert!(monitors[0].primary);
            assert!(monitors.iter().skip(1).all(|m| !m.primary));
        }
    }
}
