#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::api::process::{Command, CommandEvent};

// command with sidecar
#[tauri::command]
fn command_with_sidecar(window: tauri::Window) {
    // `new_sidecar()` expects just the filename, NOT the whole path like in JavaScript
    let (mut rx, mut child) = Command::new_sidecar("hello")
        .expect("failed to create `my-sidecar` binary command")
        .spawn()
        .expect("Failed to spawn sidecar");
    // let mut outValue: String;

    tauri::async_runtime::spawn(async move {
        // read events such as stdout
        while let Some(event) = rx.recv().await {
            if let CommandEvent::Stdout(line) = event {
                window
                    .emit("message", Some(format!("'{}'", line)))
                    .expect("failed to emit event");
                // &outValue = line;

                // write to stdin
                child.write(format!("'{}'", line).as_bytes()).unwrap();
            }
        }
    });
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![command_with_sidecar])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
