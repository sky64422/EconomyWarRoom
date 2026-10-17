//! Keeps the notch in Windows' topmost z-order band for the life of the session.
//!
//! `tao` only calls `SetWindowPos` when its own internal flag changes value.
//! Under Windows, other applications (fullscreen apps, games, elevated windows)
//! can sink the notch behind normal windows even while `WS_EX_TOPMOST` remains set.
//! This module checks if the notch has been pushed out of the topmost band
//! and reasserts it using native `SetWindowPos(HWND_TOPMOST, ...)`.

use tauri::WebviewWindow;

#[cfg(windows)]
pub fn is_out_of_topmost_band(window: &WebviewWindow) -> bool {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowLongPtrW, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOPMOST,
    };

    let Ok(raw) = window.hwnd() else { return false };
    let hwnd = HWND(raw.0 as _);

    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if (ex_style as u32 & WS_EX_TOPMOST.0) == 0 {
        return true;
    }

    struct State {
        target: isize,
        found: bool,
        band_broken: bool,
    }
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        let state = &mut *(l.0 as *mut State);
        if !IsWindowVisible(h).as_bool() {
            return BOOL(1);
        }
        if h.0 as isize == state.target {
            state.found = true;
            return BOOL(0);
        }
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        if (ex as u32 & WS_EX_TOPMOST.0) == 0 {
            state.band_broken = true;
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut state = State {
        target: hwnd.0 as isize,
        found: false,
        band_broken: false,
    };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut state as *mut _ as isize));
    }
    !state.found || state.band_broken
}

#[cfg(not(windows))]
pub fn is_out_of_topmost_band(_window: &WebviewWindow) -> bool {
    false
}

#[cfg(windows)]
pub fn reassert(window: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    let Ok(raw) = window.hwnd() else { return };
    let hwnd = HWND(raw.0 as _);
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(not(windows))]
pub fn reassert(_window: &WebviewWindow) {}
