#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindowBuilder,
};

mod commands;

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        win.show().ok();
        win.set_focus().ok();
        return;
    }
    let _win = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Minutes")
        .inner_size(480.0, 640.0)
        .min_inner_size(380.0, 480.0)
        .center()
        .focused(true)
        .build();
}

fn show_note_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("note") {
        win.show().ok();
        win.set_focus().ok();
        return;
    }
    let _win = WebviewWindowBuilder::new(app, "note", WebviewUrl::App("note.html".into()))
        .title("Add Note")
        .inner_size(360.0, 200.0)
        .resizable(false)
        .always_on_top(true)
        .center()
        .focused(true)
        .build();
}

/// Update tray to reflect recording state
pub fn update_tray_state(app: &tauri::AppHandle, is_recording: bool) {
    if let Some(tray) = app.tray_by_id("minutes-tray") {
        let icon_bytes: &[u8] = if is_recording {
            include_bytes!("../icons/icon-recording.png")
        } else {
            include_bytes!("../icons/icon.png")
        };
        if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
            tray.set_icon(Some(icon)).ok();
            tray.set_icon_as_template(!is_recording).ok();
        }
        tray.set_tooltip(Some(if is_recording {
            "Minutes — Recording..."
        } else {
            "Minutes"
        }))
        .ok();
    }
}

fn main() {
    let recording = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::new(AtomicBool::new(false));
    let processing = Arc::new(AtomicBool::new(false));
    let processing_stage = Arc::new(Mutex::new(None));
    let latest_output = Arc::new(Mutex::new(None));
    let completion_notifications_enabled = Arc::new(AtomicBool::new(true));
    let global_hotkey_enabled = Arc::new(AtomicBool::new(false));
    let global_hotkey_shortcut =
        Arc::new(Mutex::new(commands::default_hotkey_shortcut().to_string()));
    let hotkey_runtime = Arc::new(Mutex::new(commands::HotkeyRuntime::default()));
    let discard_short_hotkey_capture = Arc::new(AtomicBool::new(false));
    let recording_clone = recording.clone();
    let stop_clone = stop_flag.clone();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    commands::handle_global_hotkey_event(app, event.state());
                })
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .manage(commands::AppState {
            recording: recording.clone(),
            stop_flag: stop_flag.clone(),
            processing: processing.clone(),
            processing_stage: processing_stage.clone(),
            latest_output: latest_output.clone(),
            completion_notifications_enabled: completion_notifications_enabled.clone(),
            global_hotkey_enabled: global_hotkey_enabled.clone(),
            global_hotkey_shortcut: global_hotkey_shortcut.clone(),
            hotkey_runtime: hotkey_runtime.clone(),
            discard_short_hotkey_capture: discard_short_hotkey_capture.clone(),
        })
        .setup(move |app| {
            let initial_recording = minutes_core::pid::status().recording;

            // Create main window on launch
            show_main_window(app.handle());

            // Tray menu
            let open_item = MenuItem::with_id(app, "open", "Open Minutes", true, None::<&str>)?;
            let sep0 = MenuItem::with_id(app, "sep0", "──────────", false, None::<&str>)?;
            let record_item = MenuItem::with_id(
                app,
                "record",
                "Start Recording",
                !initial_recording,
                None::<&str>,
            )?;
            let record_item_ref = record_item.clone();
            let quick_thought_item = MenuItem::with_id(
                app,
                "quick-thought",
                "Quick Thought",
                !initial_recording,
                None::<&str>,
            )?;
            let quick_thought_item_ref = quick_thought_item.clone();
            let stop_item = MenuItem::with_id(
                app,
                "stop",
                "Stop Recording",
                initial_recording,
                None::<&str>,
            )?;
            let stop_item_ref = stop_item.clone();
            let sep = MenuItem::with_id(app, "sep1", "──────────", false, None::<&str>)?;
            let note_item = MenuItem::with_id(app, "note", "Add Note...", true, None::<&str>)?;
            let list_item =
                MenuItem::with_id(app, "list", "Open Meetings Folder", true, None::<&str>)?;
            let paste_summary_item = MenuItem::with_id(
                app,
                "paste-summary",
                "Paste Latest Summary",
                true,
                None::<&str>,
            )?;
            let paste_transcript_item = MenuItem::with_id(
                app,
                "paste-transcript",
                "Paste Latest Transcript",
                true,
                None::<&str>,
            )?;
            let sep2 = MenuItem::with_id(app, "sep2", "──────────", false, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Minutes", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &open_item,
                    &sep0,
                    &record_item,
                    &quick_thought_item,
                    &stop_item,
                    &sep,
                    &note_item,
                    &list_item,
                    &paste_summary_item,
                    &paste_transcript_item,
                    &sep2,
                    &quit_item,
                ],
            )?;

            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                .expect("load tray icon");

            let _tray = TrayIconBuilder::with_id("minutes-tray")
                .icon(icon)
                .icon_as_template(true)
                .menu(&menu)
                .tooltip("Minutes")
                .on_menu_event(move |app, event| {
                    let recording = recording_clone.clone();
                    let stop = stop_clone.clone();
                    let rec_item = record_item_ref.clone();
                    let quick_item = quick_thought_item_ref.clone();
                    let stp_item = stop_item_ref.clone();
                    match event.id.as_ref() {
                        "open" => {
                            show_main_window(app);
                        }
                        "record" => {
                            if commands::recording_active(&recording) {
                                return;
                            }
                            rec_item.set_text("Recording...").ok();
                            rec_item.set_enabled(false).ok();
                            quick_item.set_enabled(false).ok();
                            stp_item.set_enabled(true).ok();
                            update_tray_state(app, true);
                            let app_handle = app.clone();
                            let app_done = app.clone();
                            let rec = recording.clone();
                            let sf = stop.clone();
                            let processing = processing.clone();
                            let processing_stage = processing_stage.clone();
                            let latest_output = latest_output.clone();
                            let completion_notifications_enabled =
                                completion_notifications_enabled.clone();
                            let ri = rec_item.clone();
                            let si = stp_item.clone();
                            std::thread::spawn(move || {
                                commands::start_recording(
                                    app_handle,
                                    rec,
                                    sf,
                                    processing,
                                    processing_stage,
                                    latest_output,
                                    completion_notifications_enabled,
                                    None,
                                    None,
                                    minutes_core::CaptureMode::Meeting,
                                );
                                ri.set_text("Start Recording").ok();
                                ri.set_enabled(true).ok();
                                quick_item.set_enabled(true).ok();
                                si.set_enabled(false).ok();
                                update_tray_state(&app_done, false);
                            });
                        }
                        "quick-thought" => {
                            if commands::recording_active(&recording) {
                                return;
                            }
                            rec_item.set_enabled(false).ok();
                            quick_item.set_text("Quick Thought…").ok();
                            quick_item.set_enabled(false).ok();
                            stp_item.set_enabled(true).ok();
                            update_tray_state(app, true);
                            let app_handle = app.clone();
                            let app_done = app.clone();
                            let rec = recording.clone();
                            let sf = stop.clone();
                            let processing = processing.clone();
                            let processing_stage = processing_stage.clone();
                            let latest_output = latest_output.clone();
                            let completion_notifications_enabled =
                                completion_notifications_enabled.clone();
                            let ri = rec_item.clone();
                            let qi = quick_item.clone();
                            let si = stp_item.clone();
                            std::thread::spawn(move || {
                                commands::start_recording(
                                    app_handle,
                                    rec,
                                    sf,
                                    processing,
                                    processing_stage,
                                    latest_output,
                                    completion_notifications_enabled,
                                    None,
                                    None,
                                    minutes_core::CaptureMode::QuickThought,
                                );
                                ri.set_text("Start Recording").ok();
                                ri.set_enabled(true).ok();
                                qi.set_text("Quick Thought").ok();
                                qi.set_enabled(true).ok();
                                si.set_enabled(false).ok();
                                update_tray_state(&app_done, false);
                            });
                        }
                        "stop" => {
                            if commands::request_stop(&recording, &stop).is_ok() {
                                rec_item.set_text("Stopping...").ok();
                                rec_item.set_enabled(false).ok();
                                quick_item.set_text("Quick Thought").ok();
                                quick_item.set_enabled(false).ok();
                                stp_item.set_enabled(false).ok();
                                let app_done = app.clone();
                                let ri = rec_item.clone();
                                let qi = quick_item.clone();
                                let si = stp_item.clone();
                                std::thread::spawn(move || {
                                    if commands::wait_for_recording_shutdown(
                                        std::time::Duration::from_secs(120),
                                    ) {
                                        ri.set_text("Start Recording").ok();
                                        ri.set_enabled(true).ok();
                                        qi.set_text("Quick Thought").ok();
                                        qi.set_enabled(true).ok();
                                        si.set_enabled(false).ok();
                                        update_tray_state(&app_done, false);
                                    }
                                });
                            }
                        }
                        "note" => {
                            show_note_window(app);
                        }
                        "list" => {
                            let meetings_dir =
                                dirs::home_dir().unwrap_or_default().join("meetings");
                            let _ = std::process::Command::new("open").arg(meetings_dir).spawn();
                        }
                        "paste-summary" | "paste-transcript" => {
                            let target_app = commands::frontmost_application_name();
                            let kind = if event.id.as_ref() == "paste-summary" {
                                "summary"
                            } else {
                                "transcript"
                            };
                            match commands::paste_latest_artifact(
                                &latest_output,
                                kind,
                                target_app.as_deref(),
                            ) {
                                Ok(message) => {
                                    commands::show_user_notification(
                                        &format!("Paste latest {}", kind),
                                        &message,
                                    );
                                }
                                Err(err) => {
                                    commands::show_user_notification(
                                        &format!("Paste latest {}", kind),
                                        &err,
                                    );
                                }
                            }
                        }
                        "quit" => {
                            if commands::recording_active(&recording) {
                                if commands::request_stop(&recording, &stop).is_err() {
                                    return;
                                }
                                // Wait in a background thread so we do not block the tray event loop.
                                // Exiting happens only after the recording pipeline actually finishes.
                                std::thread::spawn(|| {
                                    commands::wait_for_recording_shutdown_forever();
                                    std::process::exit(0);
                                });
                            } else {
                                std::process::exit(0);
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            update_tray_state(app.handle(), initial_recording);

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide main window on close instead of quitting (app stays in tray)
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    window.hide().ok();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::cmd_status,
            commands::cmd_list_meetings,
            commands::cmd_search,
            commands::cmd_add_note,
            commands::cmd_start_recording,
            commands::cmd_stop_recording,
            commands::cmd_open_file,
            commands::cmd_clear_latest_output,
            commands::cmd_set_completion_notifications,
            commands::cmd_global_hotkey_settings,
            commands::cmd_set_global_hotkey,
            commands::cmd_permission_center,
            commands::cmd_recovery_items,
            commands::cmd_retry_recovery,
            commands::cmd_get_meeting_detail,
            commands::cmd_needs_setup,
            commands::cmd_download_model,
            commands::cmd_upcoming_meetings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running minutes app");
}
