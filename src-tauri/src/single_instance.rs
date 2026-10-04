//! Duplicate launches activate the primary window and hand their launch context to it.
use serde::Serialize;
use tauri::{Emitter, Manager, Runtime, plugin::TauriPlugin};

pub(crate) const SECOND_INSTANCE_EVENT: &str = "app://second-instance";

#[derive(Clone, Serialize)]
pub(crate) struct LaunchContext {
    pub args: Vec<String>,
    pub cwd: String,
}

pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_single_instance::init(|app, args, cwd| {
        let app_handle = app.clone();
        // macOS delivers the callback from a worker. Window activation belongs
        // on the UI thread on every platform.
        if let Err(error) = app.run_on_main_thread(move || {
            if let Some(window) = app_handle.get_webview_window("main") {
                // Attempt every step even if the previous one failed. Unminimize
                // preserves maximized state and the restored window geometry.
                if let Err(error) = window.show() {
                    eprintln!("Aurora could not show its existing window: {error}");
                }
                if let Err(error) = window.unminimize() {
                    eprintln!("Aurora could not restore its existing window: {error}");
                }
                if let Err(error) = window.set_focus() {
                    eprintln!("Aurora could not focus its existing window: {error}");
                }
            }
            // Forward rather than interpret: future file/deep-link handlers can
            // resolve relative arguments against this launching process's cwd.
            if let Err(error) = app_handle.emit(SECOND_INSTANCE_EVENT, LaunchContext { args, cwd })
            {
                eprintln!("Aurora could not forward the second launch: {error}");
            }
        }) {
            eprintln!("Aurora could not schedule activation of its existing window: {error}");
        }
    })
}
