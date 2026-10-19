//! Native material sessions. Geometry updates are serialized on the window UI thread.
use super::liquid_capture::{CaptureWorker, FrameKey};
use crate::domain::types::AppTheme;
use crate::state::AppHandleState;
use serde::Deserialize;
use std::sync::Mutex;
use tauri::{Manager, State, WebviewWindow};

const MAX_CAPTURE_PIXELS: u64 = 1_048_576;
const MAX_BLUR: f32 = 64.;
#[derive(Default)]
pub struct LiquidBackdrop(Mutex<BackdropState>, CaptureWorker);
#[derive(Default)]
struct BackdropState {
    session: u64,
    revision: u64,
    #[cfg(windows)]
    material: Option<super::native_material::NativeMaterial>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BackdropSurface {
    pub slot: u8,
    pub x: f32, pub y: f32, pub width: f32, pub height: f32,
    pub edge: Option<String>, pub radius: f32, pub blur: f32,
}
fn validate_surfaces(surfaces: &[BackdropSurface]) -> Result<(), String> {
    let mut slots = [false; 2];
    if surfaces.len() > slots.len() { return Err("Too many backdrop surfaces".into()); }
    for s in surfaces {
        if s.slot > 1 || slots[s.slot as usize]
            || ![s.x,s.y,s.width,s.height,s.radius,s.blur].iter().all(|n| n.is_finite())
            || s.width <= 0. || s.height <= 0. || s.width * s.height > MAX_CAPTURE_PIXELS as f32
            || s.radius < 0. || !(0. ..=MAX_BLUR).contains(&s.blur)
            || (s.slot == 0 && !matches!(s.edge.as_deref(), Some("left" | "right" | "top" | "bottom")))
            || (s.slot == 1 && s.edge.is_some()) {
            return Err("Invalid backdrop surface".into());
        }
        slots[s.slot as usize] = true;
    }
    Ok(())
}
async fn on_ui<T: Send + 'static>(window: WebviewWindow,
    operation: impl FnOnce(&WebviewWindow) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let view = window.clone();
    window.run_on_main_thread(move || { let _ = sender.send(operation(&view)); }).map_err(|e| e.to_string())?;
    receiver.await.map_err(|e| e.to_string())?
}

pub fn reset_on_load(view: &tauri::Webview) {
    if let Some(window) = view.get_webview_window("main") {
        let ui = window.clone();
        if let Err(error) = window.run_on_main_thread(move || {
            let Some(state) = ui.try_state::<LiquidBackdrop>() else { return; };
            let mut guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
            guard.session += 1;
            guard.revision = 0;
            #[cfg(windows)]
            if let Some(material) = &mut guard.material {
                if let Err(error) = material.sync(&[]) { eprintln!("Backdrop navigation reset: {error}"); }
            }
        }) { eprintln!("Backdrop navigation reset: {error}"); }
    }
}
#[tauri::command]
pub async fn begin_liquid_backdrop(window: WebviewWindow) -> Result<u64, String> {
    on_ui(window, |window| {
        let state = window.state::<LiquidBackdrop>();
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            if guard.material.is_none() {
                let hwnd = window.hwnd().map_err(|e| e.to_string())?;
                guard.material = Some(super::native_material::NativeMaterial::new(windows::Win32::Foundation::HWND(hwnd.0)).map_err(|e| e.to_string())?);
            }
            guard.material.as_mut().unwrap().sync(&[]).map_err(|e| e.to_string())?;
            guard.session += 1;
            guard.revision = 0;
            Ok(guard.session)
        }
        #[cfg(not(windows))]
        Err("Native backdrop requires Windows".into())
    }).await
}
#[tauri::command]
pub async fn sync_liquid_backdrop(window: WebviewWindow, session: u64, revision: u64, surfaces: Vec<BackdropSurface>) -> Result<(), String> {
    validate_surfaces(&surfaces)?;
    on_ui(window, move |window| {
        let state = window.state::<LiquidBackdrop>();
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        if session != guard.session || revision <= guard.revision { return Ok(()); }
        let app = window.state::<AppHandleState>();
        if !app.core.is_visible() || !matches!(app.core.get_state()?.settings.theme, AppTheme::Glass | AppTheme::Liquid) {
            return Err("Backdrop requires a visible glass theme".into());
        }
        #[cfg(windows)]
        guard.material.as_mut().ok_or("Backdrop session is not initialized")?.sync(&surfaces).map_err(|e| e.to_string())?;
        guard.revision = revision;
        Ok(())
    }).await
}
#[tauri::command]
pub async fn release_liquid_backdrop(window: WebviewWindow, session: u64, revision: u64) -> Result<(), String> {
    on_ui(window, move |window| {
        let state = window.state::<LiquidBackdrop>();
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        if session != guard.session || revision <= guard.revision { return Ok(()); }
        #[cfg(windows)]
        if let Some(material) = &mut guard.material { material.sync(&[]).map_err(|e| e.to_string())?; }
        guard.revision = revision;
        state.1.reset()?;
        Ok(())
    }).await
}

fn valid_region(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> Result<bool, String> {
    let (Some(right), Some(bottom)) = (x.checked_add(width), y.checked_add(height)) else {
        return Err("Liquid capture region overflow".into());
    };
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS {
        return Err("Invalid Liquid capture dimensions".into());
    }
    Ok(right <= canvas_width && bottom <= canvas_height)
}

// Diagnostics and narrow foreground-contrast probes. Native material pixels are
// composed by Windows and never sourced from this readback.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn capture_liquid_backdrop(window: WebviewWindow, state: State<'_, AppHandleState>,
    x: u32, y: u32, width: u32, height: u32, surface: Option<u8>, generation: Option<u64>,
) -> Result<tauri::ipc::Response, String> {
    if !state.core.is_visible() || !matches!(state.core.get_state()?.settings.theme, AppTheme::Glass | AppTheme::Liquid) {
        return Err("Desktop capture requires a live glass theme".into());
    }
    if surface.is_some_and(|s| s > 1) || (surface.is_some() && generation.is_none()) { return Err("Invalid Liquid surface generation".into()); }
    let size = window.inner_size().map_err(|e| e.to_string())?;
    if !valid_region(x, y, width, height, size.width, size.height)? { return Ok(tauri::ipc::Response::new(Vec::<u8>::new())); }
    let pos = window.inner_position().map_err(|e| e.to_string())?;
    let bytes = window.state::<LiquidBackdrop>().1.frame(FrameKey {
        x: pos.x + x as i32, y: pos.y + y as i32, width, height, generation: generation.unwrap_or(0),
    }, surface).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::{valid_region, validate_surfaces, BackdropSurface};
    fn surface() -> BackdropSurface { BackdropSurface { slot: 0, x: -4., y: 0., width: 56., height: 376., edge: Some("right".into()), radius: 0., blur: 4. } }
    #[test]
    fn native_surfaces_accept_partial_edges_but_reject_invalid_or_duplicate_resources() {
        assert!(validate_surfaces(&[surface()]).is_ok());
        assert!(validate_surfaces(&[]).is_ok());
        assert!(validate_surfaces(&[surface(), surface()]).is_err());
        for mutate in [
            |s: &mut BackdropSurface| s.x = f32::NAN,
            |s: &mut BackdropSurface| s.width = 0.,
            |s: &mut BackdropSurface| s.height = 1_048_576.,
            |s: &mut BackdropSurface| s.blur = -1.,
            |s: &mut BackdropSurface| s.radius = f32::INFINITY,
            |s: &mut BackdropSurface| s.edge = None,
            |s: &mut BackdropSurface| s.slot = 2,
        ] { let mut invalid = surface(); mutate(&mut invalid); assert!(validate_surfaces(&[invalid]).is_err()); }
        let mut detail = surface(); detail.slot = 1;
        assert!(validate_surfaces(&[detail.clone()]).is_err());
        detail.edge = None; detail.radius = 16.;
        assert!(validate_surfaces(&[surface(), detail]).is_ok());
    }

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
}
