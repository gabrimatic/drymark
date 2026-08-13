//! Native `DryMark` tray application.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};

use atomic_write_file::AtomicWriteFile;
use drymark_core::Policy;
use drymark_transaction::{
    CleanOutcome, ClipboardError, ClipboardPort, ClipboardSnapshot, Coordinator,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use time::{OffsetDateTime, macros::format_description};
use zeroize::{Zeroize, Zeroizing};

const DEFAULT_SHORTCUT: &str = "Alt+Shift+V";
const STATE_EVENT: &str = "state-changed";
const MAX_PREFERENCES_BYTES: usize = 16 * 1024;
const REDACTED_PANIC_MESSAGE: &str = "DryMark encountered an internal error.";

/// Metadata-only result sent to the frontend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendResult {
    /// Stable result kind.
    pub kind: &'static str,
    /// Number of removed Unicode scalars.
    pub removed: u32,
    /// Number of contextual scalars intentionally preserved.
    pub observed: u32,
    /// Whether normalization or whitespace canonicalization changed text.
    pub canonicalized: bool,
    /// Whether a fresh plain-text clipboard write cleared formatting layers.
    pub formatting_cleared: bool,
    /// Local display time for the event.
    pub at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FrontendPolicy {
    Preserve,
    Thorough,
}

impl From<FrontendPolicy> for Policy {
    fn from(value: FrontendPolicy) -> Self {
        match value {
            FrontendPolicy::Preserve => Self::PreserveAppearance,
            FrontendPolicy::Thorough => Self::Thorough,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AppStatus {
    Ready,
    Cleaning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ShortcutStatus {
    Registered,
    Conflict,
    #[allow(dead_code)]
    Unsupported,
    PermissionDenied,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PreferencesStatus {
    Saved,
    WriteFailed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrontendState {
    status: AppStatus,
    policy: FrontendPolicy,
    shortcut: String,
    shortcut_display: String,
    shortcut_status: ShortcutStatus,
    preferences_status: PreferencesStatus,
    visual_feedback: bool,
    version: &'static str,
    last_result: Option<FrontendResult>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct Preferences {
    policy: FrontendPolicy,
    shortcut: String,
    visual_feedback: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            policy: FrontendPolicy::Preserve,
            shortcut: DEFAULT_SHORTCUT.to_owned(),
            visual_feedback: true,
        }
    }
}

struct AppData {
    state: Mutex<FrontendState>,
    preferences_path: PathBuf,
    preferences_io: Mutex<()>,
    current_shortcut: Mutex<Option<String>>,
    cleaning: AtomicBool,
    first_launch: bool,
}

struct NativeClipboard {
    app: AppHandle,
}

impl ClipboardPort for NativeClipboard {
    fn read_text(&mut self) -> Result<Option<ClipboardSnapshot>, ClipboardError> {
        match self.app.clipboard().read_text() {
            // The plugin does not expose format enumeration or a revision. A
            // conservative rewrite is therefore required even for clean text.
            Ok(text) => Ok(Some(ClipboardSnapshot::new(text, None, true))),
            Err(error) => {
                let message = Zeroizing::new(error.to_string());
                if is_explicit_no_text_error(message.as_str()) {
                    Ok(None)
                } else {
                    Err(classify_clipboard_error(message.as_str()))
                }
            }
        }
    }

    fn replace_with_plain_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.app.clipboard().write_text(text).map_err(|error| {
            let message = Zeroizing::new(error.to_string());
            classify_clipboard_error(message.as_str())
        })
    }
}

/// Run the native application.
pub fn run() {
    install_redacted_panic_hook();

    let shortcut_plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                schedule_clean(app.clone());
            }
        })
        .build();

    let application = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(shortcut_plugin)
        .invoke_handler(tauri::generate_handler![
            get_state,
            clean_clipboard,
            set_policy,
            set_shortcut,
            set_visual_feedback,
            open_settings,
            quit_app,
        ])
        .setup(setup_application)
        .build(tauri::generate_context!());

    let Ok(application) = application else {
        eprintln!("DryMark could not start.");
        std::process::exit(1);
    };
    application.run(|app, event| {
        if matches!(event, tauri::RunEvent::Ready) && app.state::<AppData>().first_launch {
            show_settings(app);
        }
    });
}

fn setup_application(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let preferences_path = app.path().app_config_dir()?.join("preferences.json");
    let first_launch = !preferences_path.exists();
    let preferences = load_preferences(&preferences_path);
    let preferences_status = if persist_preferences(&preferences_path, &preferences).is_ok() {
        PreferencesStatus::Saved
    } else {
        PreferencesStatus::WriteFailed
    };
    let initial_state = FrontendState {
        status: AppStatus::Ready,
        policy: preferences.policy,
        shortcut: preferences.shortcut.clone(),
        shortcut_display: shortcut_display(&preferences.shortcut, cfg!(target_os = "macos")),
        shortcut_status: ShortcutStatus::Registered,
        preferences_status,
        visual_feedback: preferences.visual_feedback,
        version: env!("CARGO_PKG_VERSION"),
        last_result: None,
    };

    app.manage(AppData {
        state: Mutex::new(initial_state),
        preferences_path,
        preferences_io: Mutex::new(()),
        current_shortcut: Mutex::new(None),
        cleaning: AtomicBool::new(false),
        first_launch,
    });

    match app
        .global_shortcut()
        .register(preferences.shortcut.as_str())
    {
        Ok(()) => {
            *app.state::<AppData>().current_shortcut.lock() = Some(preferences.shortcut.clone());
        }
        Err(error) => {
            update_shortcut_status(app.handle(), shortcut_failure_status(&error.to_string()));
        }
    }

    install_tray(app)?;
    install_window_behavior(app);
    Ok(())
}

fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text("open", "Open DryMark")
        .text("clean", "Remove Watermarks")
        .text("settings", "Settings…")
        .separator()
        .text("quit", "Quit DryMark")
        .build()?;

    let mut builder = TrayIconBuilder::new()
        .tooltip("DryMark — Local LLM watermark remover")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id().0.as_str() {
            "open" => open_tray_window(app),
            "clean" => schedule_clean(app.clone()),
            "settings" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                position,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_tray_window(tray.app_handle(), position);
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn install_window_behavior(app: &tauri::App) {
    for label in ["tray", "settings", "toast"] {
        if let Some(window) = app.get_webview_window(label) {
            let controlled = window.clone();
            let is_tray = label == "tray";
            window.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = controlled.hide();
                }
                WindowEvent::Focused(false) if is_tray => {
                    let _ = controlled.hide();
                }
                _ => {}
            });
        }
    }
}

// Tauri command injection owns these lightweight extractor values.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn get_state(data: State<'_, AppData>) -> FrontendState {
    data.state.lock().clone()
}

#[tauri::command]
fn clean_clipboard(app: AppHandle) {
    schedule_clean(app);
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn set_policy(app: AppHandle, data: State<'_, AppData>, policy: FrontendPolicy) {
    update_preferences_and_emit(&app, &data, |state| {
        state.policy = policy;
    });
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn set_visual_feedback(app: AppHandle, data: State<'_, AppData>, enabled: bool) {
    update_preferences_and_emit(&app, &data, |state| {
        state.visual_feedback = enabled;
    });
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn set_shortcut(app: AppHandle, data: State<'_, AppData>, shortcut: String) {
    if Shortcut::from_str(&shortcut).is_err() {
        update_shortcut_status(&app, ShortcutStatus::Invalid);
        return;
    }

    let mut current_shortcut = data.current_shortcut.lock();
    let previous = current_shortcut.clone();
    if previous.as_deref() == Some(shortcut.as_str()) {
        update_shortcut_status(&app, ShortcutStatus::Registered);
        return;
    }

    if let Err(error) = app.global_shortcut().register(shortcut.as_str()) {
        update_shortcut_status(&app, shortcut_failure_status(&error.to_string()));
        return;
    }

    if let Some(previous) = previous
        && app.global_shortcut().unregister(previous.as_str()).is_err()
    {
        let _ = app.global_shortcut().unregister(shortcut.as_str());
        update_shortcut_status(&app, ShortcutStatus::Conflict);
        return;
    }

    *current_shortcut = Some(shortcut.clone());
    update_preferences_and_emit(&app, &data, |state| {
        state.shortcut.clone_from(&shortcut);
        state.shortcut_display = shortcut_display(&shortcut, cfg!(target_os = "macos"));
        state.shortcut_status = ShortcutStatus::Registered;
    });
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings(&app);
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn schedule_clean(app: AppHandle) {
    let data = app.state::<AppData>();
    if data
        .cleaning
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    {
        let mut state = data.state.lock();
        state.status = AppStatus::Cleaning;
    }
    emit_state(&app);

    tauri::async_runtime::spawn_blocking(move || {
        let policy = app.state::<AppData>().state.lock().policy.into();
        let mut clipboard = NativeClipboard { app: app.clone() };
        let outcome = guarded_clean(&mut clipboard, policy);
        let result = frontend_result(outcome, local_time());

        let should_show = {
            let data = app.state::<AppData>();
            let mut state = data.state.lock();
            state.status = AppStatus::Ready;
            state.last_result = Some(result);
            data.cleaning.store(false, Ordering::Release);
            state.visual_feedback
        };

        emit_state(&app);
        if should_show {
            let notification_app = app.clone();
            let _ = app.run_on_main_thread(move || show_toast(&notification_app));
        }
    });
}

fn guarded_clean<P: ClipboardPort>(clipboard: &mut P, policy: Policy) -> CleanOutcome {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Coordinator::default().clean(clipboard, policy)
    })) {
        Ok(outcome) => outcome,
        Err(mut payload) => {
            if let Some(message) = payload.downcast_mut::<String>() {
                message.zeroize();
            }
            CleanOutcome::WriteVerificationFailed {
                error: ClipboardError::Platform,
            }
        }
    }
}

fn install_redacted_panic_hook() {
    std::panic::set_hook(Box::new(|_info| {
        eprintln!("{REDACTED_PANIC_MESSAGE}");
    }));
}

/// Convert a transaction outcome into a metadata-only frontend result.
#[must_use]
pub fn frontend_result(outcome: CleanOutcome, at: String) -> FrontendResult {
    match outcome {
        CleanOutcome::Cleaned { report } => FrontendResult {
            kind: "cleaned",
            removed: report.total_removed(),
            observed: report.total_observed(),
            canonicalized: report.normalized || report.canonicalized_whitespace > 0,
            formatting_cleared: true,
            at,
        },
        CleanOutcome::AlreadyClean { report } => FrontendResult {
            kind: "already_clean",
            removed: 0,
            observed: report.total_observed(),
            canonicalized: false,
            formatting_cleared: false,
            at,
        },
        CleanOutcome::ClipboardChanged => simple_result("clipboard_changed", at),
        CleanOutcome::Empty => simple_result("empty", at),
        CleanOutcome::NonText => simple_result("non_text", at),
        CleanOutcome::TooLarge { .. } => simple_result("too_large", at),
        CleanOutcome::ReadFailed { .. } => simple_result("read_failed", at),
        CleanOutcome::RecheckFailed { .. } => simple_result("recheck_failed", at),
        CleanOutcome::WriteFailed { .. } => simple_result("write_failed", at),
        CleanOutcome::WriteVerificationFailed { .. } | CleanOutcome::WriteVerificationMismatch => {
            simple_result("write_unverified", at)
        }
    }
}

fn simple_result(kind: &'static str, at: String) -> FrontendResult {
    FrontendResult {
        kind,
        removed: 0,
        observed: 0,
        canonicalized: false,
        formatting_cleared: false,
        at,
    }
}

/// Format a stored shortcut for the current platform without changing it.
#[must_use]
pub fn shortcut_display(shortcut: &str, mac_symbols: bool) -> String {
    shortcut
        .split('+')
        .map(|part| {
            if mac_symbols {
                match part {
                    "Alt" => "⌥",
                    "Shift" => "⇧",
                    "Control" => "⌃",
                    "CommandOrControl" | "Meta" | "Super" => "⌘",
                    value => value,
                }
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn classify_clipboard_error(message: &str) -> ClipboardError {
    let lower = Zeroizing::new(message.to_ascii_lowercase());
    if lower.contains("permission") || lower.contains("denied") {
        ClipboardError::PermissionDenied
    } else if lower.contains("busy")
        || lower.contains("locked")
        || lower.contains("occupied")
        || lower.contains("held by another party")
    {
        ClipboardError::Busy
    } else if lower.contains("not available")
        || lower.contains("not supported")
        || lower.contains("content") && lower.contains("format")
    {
        ClipboardError::Unavailable
    } else if lower.contains("utf") || lower.contains("convert") {
        ClipboardError::InvalidText
    } else {
        ClipboardError::Platform
    }
}

fn is_explicit_no_text_error(message: &str) -> bool {
    let lower = Zeroizing::new(message.to_ascii_lowercase());
    lower.contains("clipboard contents")
        && (lower.contains("requested format") || lower.contains("clipboard is empty"))
}

fn shortcut_failure_status(message: &str) -> ShortcutStatus {
    shortcut_failure_status_for_platform(
        message,
        cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some(),
    )
}

fn shortcut_failure_status_for_platform(message: &str, wayland: bool) -> ShortcutStatus {
    let lower = message.to_ascii_lowercase();
    if lower.contains("permission") || lower.contains("denied") {
        return ShortcutStatus::PermissionDenied;
    }
    if lower.contains("already")
        || lower.contains("registered")
        || lower.contains("conflict")
        || lower.contains("in use")
    {
        return ShortcutStatus::Conflict;
    }
    if wayland {
        return ShortcutStatus::Unsupported;
    }
    ShortcutStatus::Conflict
}

fn update_shortcut_status(app: &AppHandle, status: ShortcutStatus) {
    app.state::<AppData>().state.lock().shortcut_status = status;
    emit_state(app);
}

fn update_preferences_and_emit(
    app: &AppHandle,
    data: &AppData,
    update: impl FnOnce(&mut FrontendState),
) {
    let state = update_and_persist_preferences(data, update);
    let _ = app.emit(STATE_EVENT, state);
}

fn update_and_persist_preferences(
    data: &AppData,
    update: impl FnOnce(&mut FrontendState),
) -> FrontendState {
    // Preference mutations and their disk snapshots form one serialized unit.
    // This prevents a slower earlier write from overtaking a newer setting.
    let _preferences_guard = data.preferences_io.lock();
    let preferences = {
        let mut state = data.state.lock();
        update(&mut state);
        Preferences {
            policy: state.policy,
            shortcut: state.shortcut.clone(),
            visual_feedback: state.visual_feedback,
        }
    };
    let persistence_status = if persist_preferences(&data.preferences_path, &preferences).is_ok() {
        PreferencesStatus::Saved
    } else {
        PreferencesStatus::WriteFailed
    };
    {
        let mut state = data.state.lock();
        state.preferences_status = persistence_status;
        state.clone()
    }
}

fn emit_state(app: &AppHandle) {
    let state = app.state::<AppData>().state.lock().clone();
    let _ = app.emit(STATE_EVENT, state);
}

fn load_preferences(path: &Path) -> Preferences {
    let Ok(file) = File::open(path) else {
        return Preferences::default();
    };
    let mut bytes = Vec::new();
    if file
        .take((MAX_PREFERENCES_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > MAX_PREFERENCES_BYTES
    {
        return Preferences::default();
    }

    let mut preferences = serde_json::from_slice::<Preferences>(&bytes).unwrap_or_default();
    if Shortcut::from_str(&preferences.shortcut).is_err() {
        DEFAULT_SHORTCUT.clone_into(&mut preferences.shortcut);
    }
    preferences
}

fn persist_preferences(path: &Path, preferences: &Preferences) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(preferences).map_err(std::io::Error::other)?;
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(&bytes)?;
    file.commit()
}

fn local_time() -> String {
    let now = match OffsetDateTime::now_local() {
        Ok(value) => value,
        Err(_) => OffsetDateTime::now_utc(),
    };
    match now.format(&format_description!("[hour]:[minute]")) {
        Ok(value) => value,
        Err(_) => "Just now".to_owned(),
    }
}

fn tray_popover_position(
    click: PhysicalPosition<i32>,
    width: i32,
    height: i32,
    work_area: Option<(i32, i32, u32, u32)>,
) -> PhysicalPosition<i32> {
    const GAP: i32 = 14;
    const MARGIN: i32 = 8;

    let width = width.max(0);
    let height = height.max(0);
    let preferred_x = click.x.saturating_sub(width / 2);
    let preferred_below = click.y.saturating_add(GAP);
    let preferred_above = click.y.saturating_sub(height.saturating_add(GAP));

    let Some((work_x, work_y, work_width, work_height)) = work_area else {
        let y = if click.y < 100 {
            preferred_below
        } else {
            preferred_above
        };
        return PhysicalPosition::new(preferred_x, y);
    };

    let work_width = i32::try_from(work_width).unwrap_or(i32::MAX);
    let work_height = i32::try_from(work_height).unwrap_or(i32::MAX);
    let work_right = work_x.saturating_add(work_width);
    let work_bottom = work_y.saturating_add(work_height);

    let min_x = work_x.saturating_add(MARGIN);
    let max_x = work_right.saturating_sub(width).saturating_sub(MARGIN);
    let x = if max_x < min_x {
        min_x
    } else {
        preferred_x.clamp(min_x, max_x)
    };

    let min_y = work_y.saturating_add(MARGIN);
    let max_y = work_bottom.saturating_sub(height).saturating_sub(MARGIN);
    let y = if max_y < min_y {
        min_y
    } else if (min_y..=max_y).contains(&preferred_below) {
        preferred_below
    } else if (min_y..=max_y).contains(&preferred_above) {
        preferred_above
    } else {
        preferred_below.clamp(min_y, max_y)
    };

    PhysicalPosition::new(x, y)
}

fn toast_position(
    work_area: (i32, i32, u32, u32),
    width: i32,
    height: i32,
) -> PhysicalPosition<i32> {
    const MARGIN: i32 = 16;

    let (work_x, work_y, work_width, work_height) = work_area;
    let work_width = i32::try_from(work_width).unwrap_or(i32::MAX);
    let work_height = i32::try_from(work_height).unwrap_or(i32::MAX);
    let min_x = work_x.saturating_add(MARGIN);
    let min_y = work_y.saturating_add(MARGIN);
    let preferred_x = work_x
        .saturating_add(work_width)
        .saturating_sub(width.max(0))
        .saturating_sub(MARGIN);
    let preferred_y = min_y;
    let max_x = work_x
        .saturating_add(work_width)
        .saturating_sub(width.max(0))
        .saturating_sub(MARGIN);
    let max_y = work_y
        .saturating_add(work_height)
        .saturating_sub(height.max(0))
        .saturating_sub(MARGIN);

    PhysicalPosition::new(
        if max_x < min_x {
            min_x
        } else {
            preferred_x.clamp(min_x, max_x)
        },
        if max_y < min_y {
            min_y
        } else {
            preferred_y.clamp(min_y, max_y)
        },
    )
}

fn toggle_tray_window(app: &AppHandle, click: PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window("tray") else {
        return;
    };
    if window.is_visible().ok() == Some(true) {
        let _ = window.hide();
        return;
    }

    position_and_show_tray_window(&window, click);
}

fn open_tray_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("tray") else {
        return;
    };
    if window.is_visible().ok() == Some(true) {
        let _ = window.set_focus();
        return;
    }

    let click = window
        .cursor_position()
        .ok()
        .or_else(|| {
            window.primary_monitor().ok().flatten().map(|monitor| {
                let area = monitor.work_area();
                PhysicalPosition::new(
                    f64::from(area.position.x) + f64::from(area.size.width) / 2.0,
                    f64::from(area.position.y) + 14.0,
                )
            })
        })
        .unwrap_or_else(|| PhysicalPosition::new(400.0, 30.0));
    position_and_show_tray_window(&window, click);
}

fn position_and_show_tray_window(window: &tauri::WebviewWindow, click: PhysicalPosition<f64>) {
    let size = window.outer_size().ok();
    let width = size
        .as_ref()
        .map_or(380_i32, |value| value.width.cast_signed());
    let height = size
        .as_ref()
        .map_or(540_i32, |value| value.height.cast_signed());
    let work_area = window
        .monitor_from_point(click.x, click.y)
        .ok()
        .flatten()
        .map(|monitor| {
            let area = monitor.work_area();
            (
                area.position.x,
                area.position.y,
                area.size.width,
                area.size.height,
            )
        });
    let position = tray_popover_position(click.cast::<i32>(), width, height, work_area);
    let _ = window.set_position(position);
    let _ = window.show();
    let _ = window.set_focus();
}

fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn show_toast(app: &AppHandle) {
    let Some(window) = app.get_webview_window("toast") else {
        return;
    };
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|cursor| window.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let size = window.outer_size().ok();
        let width = size
            .as_ref()
            .map_or(360_i32, |value| value.width.cast_signed());
        let height = size
            .as_ref()
            .map_or(84_i32, |value| value.height.cast_signed());
        let position = toast_position(
            (
                area.position.x,
                area.position.y,
                area.size.width,
                area.size.height,
            ),
            width,
            height,
        );
        let _ = window.set_position(position);
    }
    let _ = window.show();
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    struct PanickingClipboard;

    struct PayloadPanickingClipboard;

    impl ClipboardPort for PanickingClipboard {
        fn read_text(&mut self) -> Result<Option<ClipboardSnapshot>, ClipboardError> {
            std::panic::resume_unwind(Box::new("synthetic clipboard panic"));
        }

        fn replace_with_plain_text(&mut self, _text: &str) -> Result<(), ClipboardError> {
            Ok(())
        }
    }

    impl ClipboardPort for PayloadPanickingClipboard {
        #[allow(clippy::panic)]
        fn read_text(&mut self) -> Result<Option<ClipboardSnapshot>, ClipboardError> {
            std::panic::panic_any(String::from("PRIVATE-PANIC-ZXQ-9182"));
        }

        fn replace_with_plain_text(&mut self, _text: &str) -> Result<(), ClipboardError> {
            Ok(())
        }
    }

    #[test]
    fn clipboard_adapter_panics_become_redacted_unknown_state_failures() {
        assert_eq!(
            guarded_clean(&mut PanickingClipboard, Policy::PreserveAppearance),
            CleanOutcome::WriteVerificationFailed {
                error: ClipboardError::Platform,
            }
        );
    }

    #[test]
    fn panic_hook_never_prints_a_sensitive_payload() -> Result<(), Box<dyn std::error::Error>> {
        const CHILD_ENV: &str = "DRYMARK_REDACTED_PANIC_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            install_redacted_panic_hook();
            assert_eq!(
                guarded_clean(&mut PayloadPanickingClipboard, Policy::PreserveAppearance),
                CleanOutcome::WriteVerificationFailed {
                    error: ClipboardError::Platform,
                }
            );
            return Ok(());
        }

        let output = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "tests::panic_hook_never_prints_a_sensitive_payload",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .output()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "child test failed: {stderr}");
        assert!(!stderr.contains("PRIVATE-PANIC-ZXQ-9182"));
        assert!(stderr.contains(REDACTED_PANIC_MESSAGE));
        Ok(())
    }

    #[test]
    fn tray_popover_stays_inside_monitor_work_area() {
        let work_area = Some((-1_920, 24, 1_920, 1_056));

        assert_eq!(
            tray_popover_position(PhysicalPosition::new(-1_910, 30), 380, 540, work_area),
            PhysicalPosition::new(-1_912, 44)
        );
        assert_eq!(
            tray_popover_position(PhysicalPosition::new(-10, 30), 380, 540, work_area),
            PhysicalPosition::new(-388, 44)
        );
    }

    #[test]
    fn tray_popover_chooses_the_side_with_room_and_clamps_oversized_windows() {
        let work_area = Some((0, 24, 1_440, 876));

        assert_eq!(
            tray_popover_position(PhysicalPosition::new(720, 40), 380, 540, work_area),
            PhysicalPosition::new(530, 54)
        );
        assert_eq!(
            tray_popover_position(PhysicalPosition::new(720, 860), 380, 540, work_area),
            PhysicalPosition::new(530, 306)
        );
        assert_eq!(
            tray_popover_position(PhysicalPosition::new(20, 20), 2_000, 1_200, work_area),
            PhysicalPosition::new(8, 32)
        );
    }

    #[test]
    fn tray_popover_has_a_safe_fallback_without_monitor_metadata() {
        assert_eq!(
            tray_popover_position(PhysicalPosition::new(400, 30), 380, 540, None),
            PhysicalPosition::new(210, 44)
        );
        assert_eq!(
            tray_popover_position(PhysicalPosition::new(400, 800), 380, 540, None),
            PhysicalPosition::new(210, 246)
        );
    }

    #[test]
    fn toast_uses_monitor_work_area_and_handles_negative_coordinates() {
        assert_eq!(
            toast_position((-1_920, 24, 1_920, 1_056), 360, 84),
            PhysicalPosition::new(-376, 40)
        );
        assert_eq!(
            toast_position((0, 24, 320, 200), 360, 240),
            PhysicalPosition::new(16, 40)
        );
    }

    #[test]
    fn clipboard_error_messages_are_reduced_to_stable_categories() {
        assert_eq!(
            classify_clipboard_error("clipboard contents not available in requested format"),
            ClipboardError::Unavailable
        );
        assert_eq!(
            classify_clipboard_error("clipboard is busy"),
            ClipboardError::Busy
        );
        assert_eq!(
            classify_clipboard_error(
                "The native clipboard is not accessible due to being held by another party."
            ),
            ClipboardError::Busy
        );
        assert_eq!(
            classify_clipboard_error(
                "The selected clipboard is not supported with the current system configuration."
            ),
            ClipboardError::Unavailable
        );
        assert_eq!(
            classify_clipboard_error("Text could not be converted to the appropriate format."),
            ClipboardError::InvalidText
        );
        assert_eq!(
            classify_clipboard_error("permission denied"),
            ClipboardError::PermissionDenied
        );
        assert_eq!(
            classify_clipboard_error("clipboard not available: permission denied"),
            ClipboardError::PermissionDenied
        );
        assert_eq!(
            classify_clipboard_error("sensitive raw detail"),
            ClipboardError::Platform
        );
    }

    #[test]
    fn only_explicit_empty_or_non_text_errors_become_no_text_results() {
        assert!(is_explicit_no_text_error(
            "The clipboard contents were not available in the requested format or the clipboard is empty."
        ));
        assert!(is_explicit_no_text_error(
            "clipboard contents not available in requested format"
        ));
        assert!(!is_explicit_no_text_error(
            "The selected clipboard is not supported with the current system configuration."
        ));
        assert!(!is_explicit_no_text_error(
            "clipboard not available: permission denied"
        ));
    }

    #[test]
    fn shortcut_failures_keep_conflicts_distinct_from_wayland_limitations() {
        assert_eq!(
            shortcut_failure_status_for_platform("shortcut already registered", true),
            ShortcutStatus::Conflict
        );
        assert_eq!(
            shortcut_failure_status_for_platform("permission denied", true),
            ShortcutStatus::PermissionDenied
        );
        assert_eq!(
            shortcut_failure_status_for_platform("portal operation unavailable", true),
            ShortcutStatus::Unsupported
        );
        assert_eq!(
            shortcut_failure_status_for_platform("platform registration failed", false),
            ShortcutStatus::Conflict
        );
    }

    #[test]
    fn preferences_can_replace_an_existing_file_on_every_platform()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("preferences.json");
        persist_preferences(&path, &Preferences::default())?;

        let updated = Preferences {
            policy: FrontendPolicy::Thorough,
            shortcut: "Alt+Shift+K".to_owned(),
            visual_feedback: false,
        };
        persist_preferences(&path, &updated)?;

        assert_eq!(load_preferences(&path), updated);
        Ok(())
    }

    #[test]
    fn concurrent_preference_updates_persist_one_current_combined_snapshot()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("preferences.json");
        let data = std::sync::Arc::new(AppData {
            state: Mutex::new(FrontendState {
                status: AppStatus::Ready,
                policy: FrontendPolicy::Preserve,
                shortcut: DEFAULT_SHORTCUT.to_owned(),
                shortcut_display: DEFAULT_SHORTCUT.to_owned(),
                shortcut_status: ShortcutStatus::Registered,
                preferences_status: PreferencesStatus::Saved,
                visual_feedback: true,
                version: env!("CARGO_PKG_VERSION"),
                last_result: None,
            }),
            preferences_path: path.clone(),
            preferences_io: Mutex::new(()),
            current_shortcut: Mutex::new(Some(DEFAULT_SHORTCUT.to_owned())),
            cleaning: AtomicBool::new(false),
            first_launch: false,
        });

        std::thread::scope(|scope| {
            let policy_data = std::sync::Arc::clone(&data);
            scope.spawn(move || {
                update_and_persist_preferences(&policy_data, |state| {
                    state.policy = FrontendPolicy::Thorough;
                });
            });
            let feedback_data = std::sync::Arc::clone(&data);
            scope.spawn(move || {
                update_and_persist_preferences(&feedback_data, |state| {
                    state.visual_feedback = false;
                });
            });
        });

        assert_eq!(
            load_preferences(&path),
            Preferences {
                policy: FrontendPolicy::Thorough,
                shortcut: DEFAULT_SHORTCUT.to_owned(),
                visual_feedback: false,
            }
        );
        Ok(())
    }

    #[test]
    fn malformed_preferences_fall_back_to_safe_defaults() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("preferences.json");
        fs::write(&path, b"{not valid json")?;

        let defaults = load_preferences(&path);
        assert_eq!(defaults, Preferences::default());
        persist_preferences(&path, &defaults)?;
        assert_eq!(load_preferences(&path), Preferences::default());
        Ok(())
    }

    #[test]
    fn preference_loading_repairs_invalid_shortcuts_and_supports_missing_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("preferences.json");
        fs::write(
            &path,
            br#"{"policy":"thorough","shortcut":"not a shortcut","visualFeedback":false}"#,
        )?;

        assert_eq!(
            load_preferences(&path),
            Preferences {
                policy: FrontendPolicy::Thorough,
                shortcut: DEFAULT_SHORTCUT.to_owned(),
                visual_feedback: false,
            }
        );

        fs::write(&path, br#"{"policy":"thorough"}"#)?;
        assert_eq!(
            load_preferences(&path),
            Preferences {
                policy: FrontendPolicy::Thorough,
                ..Preferences::default()
            }
        );
        Ok(())
    }

    #[test]
    fn oversized_preference_files_are_not_materialized_without_a_bound()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("preferences.json");
        fs::write(&path, vec![b' '; MAX_PREFERENCES_BYTES + 1])?;

        assert_eq!(load_preferences(&path), Preferences::default());
        Ok(())
    }
}
