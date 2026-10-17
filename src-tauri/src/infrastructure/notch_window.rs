//! Owns physical geometry and transparent input; frontend never moves the HWND.
use crate::{
    domain::{
        notch::*,
        reveal::{folded_rect, NotchReveal},
    },
    state::AppHandleState,
};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
#[cfg(not(windows))]
use tauri::{PhysicalPosition, PhysicalSize};

const IDLE_POINTER_INTERVAL: Duration = Duration::from_millis(32);
const DRAG_POINTER_INTERVAL: Duration = Duration::from_millis(8);
const DISPLAY_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(uPeriod: u32) -> u32;
    fn timeEndPeriod(uPeriod: u32) -> u32;
}

#[derive(Default)]
pub struct NotchController(Mutex<Runtime>);
#[derive(Default)]
struct Runtime {
    revision: u64,
    expanded: bool,
    height: f64,
    target: Option<f64>,
    layout: Option<NotchLayout>,
    ignoring: bool,
    drag: Option<DragSession>,
    last_drag_id: u64,
    pump_pending: bool,
    reveal: NotchReveal,
    focused: bool,
    #[cfg(windows)]
    timer_period_active: bool,
}

impl Runtime {
    #[cfg(windows)]
    fn start_timer_period(&mut self) {
        if !self.timer_period_active {
            unsafe {
                timeBeginPeriod(1);
            }
            self.timer_period_active = true;
        }
    }

    #[cfg(windows)]
    fn stop_timer_period(&mut self) {
        if self.timer_period_active {
            unsafe {
                timeEndPeriod(1);
            }
            self.timer_period_active = false;
        }
    }

    fn end_drag(&mut self) -> Option<DragSession> {
        #[cfg(windows)]
        self.stop_timer_period();
        self.drag.take()
    }
}

#[cfg(windows)]
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop_timer_period();
    }
}

pub fn get_reveal(app: &AppHandle) -> Result<bool, String> {
    Ok(app
        .state::<NotchController>()
        .0
        .lock()
        .map_err(|_| "Notch lock poisoned")?
        .reveal
        .shown())
}

pub fn set_focus(app: &AppHandle, focused: bool) -> Result<bool, String> {
    let controller = app.state::<NotchController>();
    let mut rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
    rt.focused = focused;
    if focused {
        if let Some(shown) = rt.reveal.open() {
            app.emit("notch-reveal", shown).map_err(|e| e.to_string())?;
        }
    }
    Ok(rt.reveal.shown())
}

pub fn monitors(app: &AppHandle) -> Result<Vec<MonitorArea>, String> {
    let primary = app.primary_monitor().map_err(|e| e.to_string())?;
    let mut monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if let Some(p) = primary {
        monitors.sort_by_key(|m| m.position() != p.position());
    }
    Ok(monitors
        .into_iter()
        .map(|m| {
            let pos = m.position();
            let size = m.size();
            let work = m.work_area();
            MonitorArea {
                name: m
                    .name()
                    .cloned()
                    .unwrap_or_else(|| format!("{},{}", pos.x, pos.y)),
                bounds: Rect {
                    x: pos.x as f64,
                    y: pos.y as f64,
                    width: size.width as f64,
                    height: size.height as f64,
                },
                work: Rect {
                    x: work.position.x as f64,
                    y: work.position.y as f64,
                    width: work.size.width as f64,
                    height: work.size.height as f64,
                },
                scale: m.scale_factor(),
            }
        })
        .collect())
}

pub fn apply(
    app: &AppHandle,
    request: Option<(u64, bool, f64, Option<f64>)>,
) -> Result<NotchLayout, String> {
    let controller = app.state::<NotchController>();
    let mut rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
    if let Some((revision, expanded, height, target)) = request {
        if revision <= rt.revision {
            return rt.layout.clone().ok_or("Notch not ready".into());
        }
        if !height.is_finite() {
            return Err("Invalid detail height".into());
        }
        rt.revision = revision;
        rt.expanded = expanded;
        rt.height = height;
        rt.target = target;
        if expanded {
            if let Some(shown) = rt.reveal.open() {
                app.emit("notch-reveal", shown).map_err(|e| e.to_string())?;
            }
        }
    }
    let state = app.state::<AppHandleState>();
    let settings = state.core.get_state()?.settings;
    let placement = rt
        .drag
        .as_ref()
        .filter(|drag| drag.motion.active)
        .map(|drag| &drag.motion.placement)
        .unwrap_or(&settings.notch);
    let areas = match rt.drag.as_ref() {
        Some(drag) => drag.monitors.clone(),
        None => monitors(app)?,
    };
    let monitor = areas
        .iter()
        .find(|m| Some(&m.name) == placement.monitor_hint.as_ref())
        .or(areas.first())
        .ok_or("No monitor available")?;
    let count = watchlist_slot_count(
        app.state::<AppHandleState>()
            .core
            .get_state()?
            .watchlist
            .len(),
    );
    let layout = calculate_layout_target(
        monitor,
        placement,
        count,
        rt.expanded
            && !rt.drag.as_ref().is_some_and(|d| d.motion.active)
            && state.core.is_visible(),
        rt.height,
        rt.target,
    )?;
    let changed = rt.layout.as_ref() != Some(&layout);
    if changed {
        let win = super::window_ctl::main_window(app)?;
        #[cfg(windows)]
        {
            use windows::Win32::{
                Foundation::HWND,
                UI::WindowsAndMessaging::{
                    SetWindowPos, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOZORDER,
                },
            };
            let hwnd = win.hwnd().map_err(|e| e.to_string())?;
            // One native move+resize keeps the screen-edge contact fixed throughout.
            unsafe {
                SetWindowPos(
                    HWND(hwnd.0),
                    None,
                    layout.window.x as i32,
                    layout.window.y as i32,
                    layout.window.width as i32,
                    layout.window.height as i32,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOCOPYBITS,
                )
                .map_err(|e| e.to_string())?;
            }
        }
        #[cfg(not(windows))]
        {
            win.set_size(PhysicalSize::new(
                layout.window.width as u32,
                layout.window.height as u32,
            ))
            .map_err(|e| e.to_string())?;
            win.set_position(PhysicalPosition::new(
                layout.window.x as i32,
                layout.window.y as i32,
            ))
            .map_err(|e| e.to_string())?;
        }
        let content_changed = match &rt.layout {
            Some(prev) => {
                prev.edge != layout.edge
                    || prev.scale != layout.scale
                    || prev.monitor != layout.monitor
                    || prev.window.width != layout.window.width
                    || prev.window.height != layout.window.height
                    || prev.detail != layout.detail
            }
            None => true,
        };
        if content_changed || rt.drag.is_none() {
            app.emit("notch-layout", &layout)
                .map_err(|e| e.to_string())?;
        }
        rt.layout = Some(layout.clone());
    }
    Ok(layout)
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut last_display_check = Instant::now();
        let mut last_error = String::new();
        loop {
            let is_dragging = {
                let controller = app.state::<NotchController>();
                let rt = controller.0.lock().expect("Notch lock poisoned");
                rt.drag.is_some()
            };
            if !is_dragging && last_display_check.elapsed() >= DISPLAY_CHECK_INTERVAL {
                last_display_check = Instant::now();
                let handle = app.clone();
                if let Err(e) = app.run_on_main_thread(move || {
                    if let Ok(win) = super::window_ctl::main_window(&handle) {
                        if super::topmost::is_out_of_topmost_band(&win) {
                            super::topmost::reassert(&win);
                        }
                    }
                    if let Err(e) = apply(&handle, None) {
                        eprintln!("notch placement: {e}");
                    }
                }) {
                    eprintln!("notch dispatch: {e}");
                }
            }
            let queue_drag = {
                let controller = app.state::<NotchController>();
                let mut rt = controller.0.lock().expect("Notch lock poisoned");
                let queue = rt.drag.is_some() && !rt.pump_pending;
                if queue {
                    rt.pump_pending = true;
                }
                queue
            };
            if queue_drag {
                let handle = app.clone();
                if let Err(e) = app.run_on_main_thread(move || {
                    let res = pump_drag(&handle);
                    handle
                        .state::<NotchController>()
                        .0
                        .lock()
                        .expect("Notch lock poisoned")
                        .pump_pending = false;
                    if let Err(e) = res {
                        eprintln!("notch drag: {e}");
                    }
                }) {
                    app.state::<NotchController>()
                        .0
                        .lock()
                        .expect("Notch lock poisoned")
                        .pump_pending = false;
                    eprintln!("notch drag dispatch: {e}");
                }
            }
            if !is_dragging {
                let result = (|| -> Result<(), String> {
                    let win = super::window_ctl::main_window(&app)?;
                    let pos = win.cursor_position().map_err(|e| e.to_string())?;
                    let controller = app.state::<NotchController>();
                    let mut rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
                    let inside = rt.layout.as_ref().is_some_and(|layout| {
                        if rt.reveal.shown() {
                            layout.hit(pos.x, pos.y)
                        } else {
                            folded_rect(layout).contains(pos.x, pos.y)
                        }
                    });
                    let always_show = app
                        .try_state::<AppHandleState>()
                        .and_then(|s| s.core.get_state().ok())
                        .map(|s| s.settings.always_show_notch)
                        .unwrap_or(false);
                    let held = rt.expanded || rt.focused || rt.drag.is_some() || always_show;
                    if let Some(shown) = rt.reveal.update(started.elapsed(), inside, held) {
                        app.emit("notch-reveal", shown).map_err(|e| e.to_string())?;
                    }
                    // Folding changes input coverage, never HWND placement or size.
                    let ignore = rt.drag.is_none()
                        && rt.layout.as_ref().is_none_or(|layout| {
                            if rt.reveal.shown() {
                                !layout.hit(pos.x, pos.y)
                            } else {
                                !folded_rect(layout).contains(pos.x, pos.y)
                            }
                        });
                    if ignore != rt.ignoring {
                        win.set_ignore_cursor_events(ignore)
                            .map_err(|e| e.to_string())?;
                        rt.ignoring = ignore;
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    if e != last_error {
                        eprintln!("notch input: {e}");
                        last_error = e;
                    }
                }
            }
            let sleep_interval = if is_dragging {
                DRAG_POINTER_INTERVAL
            } else {
                IDLE_POINTER_INTERVAL
            };
            std::thread::sleep(sleep_interval);
        }
    });
}

struct DragSession {
    id: u64,
    motion: NotchDrag,
    monitors: Vec<MonitorArea>,
    last_pos: Option<(f64, f64)>,
}
#[derive(Clone, serde::Serialize)]
pub struct DragNotice {
    id: u64,
    active: bool,
    finished: bool,
    placement: NotchPlacement,
    error: Option<String>,
}

pub fn begin_drag(app: &AppHandle, id: u64) -> Result<(), String> {
    let win = super::window_ctl::main_window(app)?;
    let pos = win.cursor_position().map_err(|e| e.to_string())?;
    let areas = monitors(app)?;
    let settings = app.state::<AppHandleState>().core.get_state()?.settings;
    let count = watchlist_slot_count(
        app.state::<AppHandleState>()
            .core
            .get_state()?
            .watchlist
            .len(),
    );
    let controller = app.state::<NotchController>();
    let mut rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
    if id <= rt.last_drag_id || rt.drag.is_some() {
        return Err("Drag session is stale or already active".into());
    }
    let layout = rt.layout.as_ref().ok_or("Notch not ready")?;
    let mut motion = NotchDrag::new(layout, pos.x, pos.y, count);
    // Keep the actual starting offset even if no movement crosses the threshold.
    motion.placement.offset = settings.notch.offset;
    rt.last_drag_id = id;
    #[cfg(windows)]
    rt.start_timer_period();
    rt.drag = Some(DragSession {
        id,
        motion,
        monitors: areas,
        last_pos: None,
    });
    Ok(())
}

fn update_drag(app: &AppHandle, id: u64) -> Result<(), String> {
    let pos = super::window_ctl::main_window(app)?
        .cursor_position()
        .map_err(|e| e.to_string())?;
    let controller = app.state::<NotchController>();
    let (notice, moved) = {
        let mut rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
        let Some(drag) = rt.drag.as_mut().filter(|d| d.id == id) else {
            return Ok(());
        };
        if drag.last_pos == Some((pos.x, pos.y)) {
            return Ok(());
        }
        drag.last_pos = Some((pos.x, pos.y));
        let was_active = drag.motion.active;
        let monitors = drag.monitors.clone();
        drag.motion.update(&monitors, pos.x, pos.y)?;
        let active = drag.motion.active;
        let placement = drag.motion.placement.clone();
        let notice = (!was_active && active).then_some(DragNotice {
            id,
            active,
            finished: false,
            placement,
            error: None,
        });
        (notice, active)
    };
    if let Some(notice) = notice {
        app.emit("notch-drag", notice).map_err(|e| e.to_string())?;
    }
    if moved {
        apply(app, None)?;
    }
    Ok(())
}

pub fn finish_drag(app: &AppHandle, id: u64, cancel: bool) -> Result<(), String> {
    complete_drag(app, id, cancel, None)
}
fn complete_drag(
    app: &AppHandle,
    id: u64,
    cancel: bool,
    mut error: Option<String>,
) -> Result<(), String> {
    if !cancel {
        if let Err(e) = update_drag(app, id) {
            error = Some(e);
        }
    }
    let controller = app.state::<NotchController>();
    let session = {
        let mut rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
        if !rt.drag.as_ref().is_some_and(|d| d.id == id) {
            return Ok(());
        }
        rt.end_drag().unwrap()
    };
    let core = &app.state::<AppHandleState>().core;
    if !cancel && error.is_none() && session.motion.active {
        if let Ok(current_areas) = monitors(app) {
            if current_areas != session.monitors {
                error = Some("Display configuration changed; drag cancelled".into());
            }
        }
        if error.is_none() {
            if let Err(e) = core.set_notch_placement(session.motion.placement) {
                error = Some(e);
            }
        }
    }
    if let Err(e) = apply(app, None) {
        error = Some(error.map_or(e.clone(), |first| format!("{first}; {e}")));
    }
    app.emit(
        "notch-drag",
        DragNotice {
            id,
            active: session.motion.active,
            finished: true,
            placement: core.get_state()?.settings.notch,
            error,
        },
    )
    .map_err(|e| e.to_string())
}

fn pump_drag(app: &AppHandle) -> Result<(), String> {
    let id = app
        .state::<NotchController>()
        .0
        .lock()
        .map_err(|_| "Notch lock poisoned")?
        .drag
        .as_ref()
        .map(|d| d.id);
    let Some(id) = id else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_LBUTTON,
        };
        if unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } >= 0 {
            return complete_drag(app, id, false, None);
        }
    }
    if let Err(e) = update_drag(app, id) {
        return complete_drag(app, id, true, Some(e));
    }
    Ok(())
}
