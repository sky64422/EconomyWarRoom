//! Window show/hide, geometry, and opacity helpers.
//!
//! Note: Tauri 2 has no `Window::set_opacity` API. Opacity is clamped, persisted,
//! and emitted as `opacity-updated` so the frontend can apply CSS opacity.
//! Geometry and always-on-top use native window APIs.

use crate::domain::constants::{clamp_geometry, clamp_opacity};
use crate::domain::types::WindowGeometry;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow};

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
