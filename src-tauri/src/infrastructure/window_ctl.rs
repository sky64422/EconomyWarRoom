//! Window show/hide, geometry, and opacity helpers.
//!
//! Note: Tauri 2 has no `Window::set_opacity` API. Opacity is clamped, persisted,
//! and emitted as `opacity-updated` so the frontend can apply CSS opacity.
//! Geometry and always-on-top use native window APIs.

use crate::domain::constants::{clamp_geometry, clamp_opacity, WindowPolicy};
use crate::domain::types::WindowGeometry;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalSize, Size, WebviewWindow,
    Window,
};

pub fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window not found".into())
}

pub fn apply_always_on_top(window: &WebviewWindow, on_top: bool) -> Result<(), String> {
    window.set_always_on_top(on_top).map_err(|e| e.to_string())
}

pub fn apply_geometry(window: &WebviewWindow, geometry: &WindowGeometry) -> Result<(), String> {
    let geometry = clamp_geometry(geometry);
    window
        .set_size(LogicalSize::new(geometry.width, geometry.height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(LogicalPosition::new(geometry.x, geometry.y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Clamp and notify frontend. Native window opacity is not available in Tauri 2.
pub fn apply_opacity(app: &AppHandle, opacity: f64) -> Result<f64, String> {
    let opacity = clamp_opacity(opacity);
    app.emit("opacity-updated", opacity)
        .map_err(|e| e.to_string())?;
    Ok(opacity)
}

pub fn show_window(window: &WebviewWindow) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

pub fn hide_window(window: &WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

/// Kill OS/CSS fringe outside the rounded glass (Windows WebView2).
/// Note: SetWindowRgn clips break DWM transparency — use DWM corner prefs only.
/// (Ported from TokenUsage — Tauri docs: shadow:true draws a 1px white border.)
pub fn apply_clean_glass_edge(window: &WebviewWindow) -> Result<(), String> {
    use tauri::window::Color;
    // Fully transparent surface; avoids default white HWND fill in corners.
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    // OS drop-shadow on transparent windows draws a light ring outside CSS radius.
    let _ = window.set_shadow(false);
    #[cfg(windows)]
    apply_dwm_round_corners(window)?;
    Ok(())
}

/// Win11: prefer system-rounded window so square transparent corners are not shown.
#[cfg(windows)]
fn apply_dwm_round_corners(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_ROUND,
    };

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let pref = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            HWND(hwnd.0),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const DWM_WINDOW_CORNER_PREFERENCE as *const _,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
    }
    Ok(())
}

/// Install OS-level min size (hard wall while dragging). Uses physical pixels.
pub fn apply_content_min_size(
    window: &impl WindowMinSize,
    logical_w: f64,
    logical_h: f64,
) -> Result<(), String> {
    let w = logical_w.max(WindowPolicy::MIN_WIDTH);
    let h = logical_h.max(WindowPolicy::MIN_HEIGHT);
    let scale = window.scale_factor_for_min().map_err(|e| e.to_string())?;
    let pw = (w * scale).ceil() as u32;
    let ph = (h * scale).ceil() as u32;
    window
        .set_min_size_for_content(Some(Size::Physical(PhysicalSize {
            width: pw.max(1),
            height: ph.max(1),
        })))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// If the window is smaller than the content floor, grow it (no shrink).
pub fn ensure_at_least_min_size(
    window: &impl WindowMinSize,
    logical_w: f64,
    logical_h: f64,
) -> Result<(), String> {
    let min_w = logical_w.max(WindowPolicy::MIN_WIDTH);
    let min_h = logical_h.max(WindowPolicy::MIN_HEIGHT);
    let scale = window.scale_factor_for_min().map_err(|e| e.to_string())?;
    let size = window.inner_size_for_min().map_err(|e| e.to_string())?;
    let cur_w = size.width as f64 / scale;
    let cur_h = size.height as f64 / scale;
    if cur_w + 0.5 < min_w || cur_h + 0.5 < min_h {
        window
            .set_size_for_content(Size::Logical(LogicalSize::new(
                cur_w.max(min_w),
                cur_h.max(min_h),
            )))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Clamp a physical resize to stored content min (logical). Used from window events.
pub fn clamp_physical_size_to_content_min(
    window: &impl WindowMinSize,
    physical: PhysicalSize<u32>,
    min_logical_w: f64,
    min_logical_h: f64,
) -> Result<(), String> {
    let scale = window.scale_factor_for_min().map_err(|e| e.to_string())?;
    let min_pw = (min_logical_w.max(WindowPolicy::MIN_WIDTH) * scale).ceil() as u32;
    let min_ph = (min_logical_h.max(WindowPolicy::MIN_HEIGHT) * scale).ceil() as u32;
    if physical.width >= min_pw && physical.height >= min_ph {
        return Ok(());
    }
    // Re-assert OS min first so further drag hits a wall, then snap if already past it.
    let _ = apply_content_min_size(window, min_logical_w, min_logical_h);
    window
        .set_size_for_content(Size::Physical(PhysicalSize {
            width: physical.width.max(min_pw),
            height: physical.height.max(min_ph),
        }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Shared ops for `WebviewWindow` and `Window` (window event handler).
pub trait WindowMinSize {
    fn scale_factor_for_min(&self) -> Result<f64, tauri::Error>;
    fn inner_size_for_min(&self) -> Result<PhysicalSize<u32>, tauri::Error>;
    fn set_min_size_for_content(&self, size: Option<Size>) -> Result<(), tauri::Error>;
    fn set_size_for_content(&self, size: Size) -> Result<(), tauri::Error>;
}

impl WindowMinSize for WebviewWindow {
    fn scale_factor_for_min(&self) -> Result<f64, tauri::Error> {
        self.scale_factor()
    }
    fn inner_size_for_min(&self) -> Result<PhysicalSize<u32>, tauri::Error> {
        self.inner_size()
    }
    fn set_min_size_for_content(&self, size: Option<Size>) -> Result<(), tauri::Error> {
        self.set_min_size(size)
    }
    fn set_size_for_content(&self, size: Size) -> Result<(), tauri::Error> {
        self.set_size(size)
    }
}

impl WindowMinSize for Window {
    fn scale_factor_for_min(&self) -> Result<f64, tauri::Error> {
        self.scale_factor()
    }
    fn inner_size_for_min(&self) -> Result<PhysicalSize<u32>, tauri::Error> {
        self.inner_size()
    }
    fn set_min_size_for_content(&self, size: Option<Size>) -> Result<(), tauri::Error> {
        self.set_min_size(size)
    }
    fn set_size_for_content(&self, size: Size) -> Result<(), tauri::Error> {
        self.set_size(size)
    }
}
