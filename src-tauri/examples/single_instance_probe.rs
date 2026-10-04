//! Isolated native smoke-test harness. Never opens Aurora's databases or playback.
#[path = "../src/single_instance.rs"]
mod single_instance;

use std::{fs::OpenOptions, io::Write, path::Path, sync::Arc};
use tauri::{Listener, Manager, WebviewUrl, WebviewWindowBuilder};

fn record(path: &Path, value: serde_json::Value) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{value}").unwrap();
}

fn main() {
    // Required isolation prevents accidental use of the real application's guard.
    let output = Arc::new(std::path::PathBuf::from(
        std::env::var("AURORA_INSTANCE_PROBE_OUTPUT").expect("probe output path required"),
    ));
    let mut context = tauri::generate_context!();
    context.config_mut().identifier = format!(
        "com.soundtrackgeek.aurora.probe.{}",
        std::env::var("AURORA_INSTANCE_PROBE_ID").expect("isolated probe id required")
    );
    context.config_mut().app.windows.clear();
    let setup_output = Arc::clone(&output);
    tauri::Builder::default()
        .plugin(single_instance::init())
        .setup(move |app| {
            record(
                &setup_output,
                serde_json::json!({ "kind": "setup", "pid": std::process::id() }),
            );
            let window = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External("about:blank".parse()?),
            )
            .title("Aurora single-instance probe")
            .visible(false)
            .build()?;
            let event_output = Arc::clone(&setup_output);
            let event_window = window.clone();
            let event_app = app.handle().clone();
            app.listen(single_instance::SECOND_INSTANCE_EVENT, move |event| {
                let payload = serde_json::from_str::<serde_json::Value>(event.payload()).unwrap();
                let exit = payload["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|arg| arg == "--probe-exit");
                record(
                    &event_output,
                    serde_json::json!({
                        "kind": "launch",
                        "pid": std::process::id(),
                        "payload": payload,
                        "visible": event_window.is_visible().unwrap(),
                        "minimized": event_window.is_minimized().unwrap(),
                        "maximized": event_window.is_maximized().unwrap(),
                        "focused": event_window.is_focused().unwrap(),
                    }),
                );
                if exit {
                    event_app.exit(0);
                }
            });
            match std::env::var("AURORA_INSTANCE_PROBE_MODE").as_deref() {
                Ok("minimized") => {
                    window.show()?;
                    window.minimize()?;
                }
                Ok("maximized") => {
                    window.show()?;
                    window.maximize()?;
                    window.minimize()?;
                }
                _ => {}
            }
            Ok(())
        })
        .build(context)
        .expect("build isolated probe")
        .run(move |app, event| {
            if let tauri::RunEvent::Ready = event {
                let window = app.get_webview_window("main").unwrap();
                record(
                    &output,
                    serde_json::json!({
                        "kind": "ready", "pid": std::process::id(),
                        "visible": window.is_visible().unwrap(),
                        "minimized": window.is_minimized().unwrap(),
                        "maximized": window.is_maximized().unwrap(),
                    }),
                );
            }
        });
}
