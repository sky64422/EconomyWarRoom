//! In-memory Windows backdrop capture, enabled by the Glass and Liquid themes.
//! Capture -> web lens follows https://github.com/molian313/dynamic-island;
//! raw RGBA IPC avoids per-frame decoding; legacy probes can still request BMP.

use crate::application::diagnostics::DiagLevel;
use crate::domain::types::AppTheme;
use super::liquid_capture::{CaptureWorker, FrameKey};
use crate::state::AppHandleState;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Manager, State, WebviewWindow};

use crate::domain::notch::valid_region;
const CAPTURE_LEASE: Duration = Duration::from_millis(80);
const LEASE_CHECK: Duration = Duration::from_millis(20);

#[derive(Default)]
pub struct LiquidBackdrop(Mutex<BackdropState>, CaptureWorker);

#[derive(Default)]
struct BackdropState {
    lease: Option<Lease>,
    watching: bool,
    in_flight: usize,
    release_pending: bool,
}
struct Lease {
    hwnd: usize,
    original: u32,
    renewed: Instant,
}

pub fn reset_on_load(view: &tauri::Webview) {
    if let Some(window) = view.get_webview_window("main") {
        if let Err(error) = release_liquid_backdrop(window) {
            eprintln!("Liquid backdrop reset: {error}");
        }
    }
}

#[tauri::command]
pub fn log_capture_event(
    state: State<'_, AppHandleState>,
    tag: String,
    message: String,
) -> Result<(), String> {
    crate::infrastructure::capture_log::log_event(&tag, &message);
    state.core.note(DiagLevel::Info, format!("[{tag}] {message}"));
    Ok(())
}

#[tauri::command]
pub fn release_liquid_backdrop(window: WebviewWindow) -> Result<(), String> {
    if let Some(state) = window.try_state::<LiquidBackdrop>() {
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        if guard.in_flight > 0 {
            guard.release_pending = true;
            crate::infrastructure::capture_log::log_event("RELEASE", "release_liquid_backdrop queued (in_flight > 0)");
            return Ok(());
        }
        if let Some(lease) = guard.lease.take() {
            set_affinity(lease.hwnd, lease.original, "release_backdrop")?;
        }
        guard.release_pending = false;
    }
    Ok(())
}

#[tauri::command]
pub fn destroy_liquid_backdrop(window: WebviewWindow) -> Result<(), String> {
    release_liquid_backdrop(window.clone())?;
    if let Some(state) = window.try_state::<LiquidBackdrop>() {
        state.1.reset()?;
    }
    Ok(())
}

#[cfg(windows)]
struct InFlight(WebviewWindow);
#[cfg(windows)]
impl Drop for InFlight {
    fn drop(&mut self) {
        let state = self.0.state::<LiquidBackdrop>();
        let pending = match state.0.lock() {
            Ok(mut guard) => {
                guard.in_flight = guard.in_flight.saturating_sub(1);
                guard.release_pending && guard.in_flight == 0
            }
            Err(error) => {
                eprintln!("Liquid completion: {error}");
                false
            }
        };
        if pending {
            if let Err(error) = release_liquid_backdrop(self.0.clone()) {
                eprintln!("Liquid deferred release: {error}");
            }
        }
    }
}

#[cfg(windows)]
fn set_affinity(hwnd: usize, value: u32, reason: &str) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY,
    };
    crate::infrastructure::capture_log::log_event(
        "AFFINITY",
        &format!("SetWindowDisplayAffinity(0x{value:02X}) reason={reason} hwnd=0x{hwnd:X}"),
    );
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as *mut _), WINDOW_DISPLAY_AFFINITY(value)) }
        .map_err(|e| format!("Liquid capture exclusion: {e}"))
}
#[cfg(not(windows))]
fn set_affinity(_: usize, _: u32, _: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn live_window(hwnd: usize) -> bool {
    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindow};
    unsafe { IsWindow(Some(HWND(hwnd as *mut _))).as_bool() }
}
#[cfg(not(windows))]
fn live_window(_: usize) -> bool {
    false
}

fn watch_lease(window: WebviewWindow) {
    std::thread::spawn(move || loop {
        std::thread::sleep(LEASE_CHECK);
        let state = window.state::<LiquidBackdrop>();
        let mut guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
        if guard.in_flight > 0 { continue; }
        if guard
            .lease
            .as_ref()
            .is_some_and(|lease| lease.renewed.elapsed() < CAPTURE_LEASE)
        {
            continue;
        }
        if let Some(lease) = guard.lease.take() {
            if let Err(error) = set_affinity(lease.hwnd, lease.original, "lease_expired") {
                window
                    .state::<AppHandleState>()
                    .core
                    .note(DiagLevel::Warn, error);
                if live_window(lease.hwnd) {
                    continue;
                }
            }
        }
        guard.watching = false;
        break;
    });
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn capture_liquid_backdrop(
    window: WebviewWindow,
    state: State<'_, AppHandleState>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    surface: Option<u8>,
    generation: Option<u64>,
) -> Result<tauri::ipc::Response, String> {
    if !state.core.is_visible() || !matches!(state.core.get_state()?.settings.theme, AppTheme::Glass | AppTheme::Liquid) {
        return Err("Desktop capture requires a live glass theme".into());
    }
    if surface.is_some_and(|s| s > 1) || (surface.is_some() && generation.is_none()) {
        return Err("Invalid Liquid surface generation".into());
    }
    let size = window.inner_size().map_err(|e| e.to_string())?;
    if !valid_region(x, y, width, height, size.width, size.height)? {
        return Ok(tauri::ipc::Response::new(Vec::<u8>::new()));
    }
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Dwm::DwmFlush;
        use windows::Win32::UI::WindowsAndMessaging::{
            WDA_EXCLUDEFROMCAPTURE,
        };
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let pos = window.inner_position().map_err(|e| e.to_string())?;
        let backdrop = window.state::<LiquidBackdrop>();
        {
        let mut guard = backdrop.0.lock().map_err(|e| e.to_string())?;
        if guard.lease.is_none() {
            set_affinity(hwnd.0 as usize, WDA_EXCLUDEFROMCAPTURE.0, "capture_start")
                .inspect_err(|e| state.core.note(DiagLevel::Warn, e.clone()))?;
            guard.lease = Some(Lease {
                hwnd: hwnd.0 as usize,
                original: 0,
                renewed: Instant::now(),
            });
            if !guard.watching {
                guard.watching = true;
                watch_lease(window.clone());
            }
            unsafe {
                DwmFlush().map_err(|e| e.to_string())?;
            }
        }
        guard.lease.as_mut().unwrap().renewed = Instant::now();
        guard.in_flight += 1;
        }
        let capture_start = Instant::now();
        let _flight = InFlight(window.clone());
        let bytes = backdrop.1.frame(FrameKey {
            x: pos.x + x as i32, y: pos.y + y as i32, width, height,
            generation: generation.unwrap_or(0),
        }, surface).await
            .inspect_err(|e| state.core.note(DiagLevel::Warn, e.clone()))?;
        crate::infrastructure::capture_log::log_event(
            "BACKDROP",
            &format!("frame captured in {:?} (bytes={})", capture_start.elapsed(), bytes.len()),
        );
        Ok(tauri::ipc::Response::new(bytes))
    }
    #[cfg(not(windows))]
    Err("Live Liquid backdrop requires Windows".into())
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
pub struct BackdropSurface {
    pub slot: u8,
    pub x: f32, pub y: f32, pub width: f32, pub height: f32,
    pub edge: Option<String>, pub radius: f32, pub blur: f32,
}

#[tauri::command]
pub async fn begin_liquid_backdrop() -> Result<u64, String> {
    Ok(1)
}

#[tauri::command]
pub async fn sync_liquid_backdrop(_session: u64, _revision: u64, _surfaces: Vec<BackdropSurface>) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_region;
    #[test]
    fn bounds_reject_zero_overflow_and_unbounded_pixel_allocations() {
        assert_eq!(valid_region(268, 372, 56, 376, 324, 1120), Ok(true));
        assert_eq!(valid_region(300, 0, 56, 1, 324, 1120), Ok(false));
        for (x, y, w, h) in [
            (0, 0, 0, 1),
            (u32::MAX, 0, 1, 1),
            (0, 0, 2048, 2048),
        ] {
            assert!(valid_region(x, y, w, h, 324, 1120).is_err());
        }
    }

    #[test]
    fn capture_lease_duration_is_bounded_to_prevent_screenshot_exclusion() {
        assert!(super::CAPTURE_LEASE <= std::time::Duration::from_millis(100));
        assert!(super::LEASE_CHECK < super::CAPTURE_LEASE);
    }
}
