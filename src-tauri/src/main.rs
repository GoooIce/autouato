#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::Window;

use vad::{mp4, speech};

mod customer;

// the payload type must implement `Serialize` and `Clone`.
#[derive(Clone, serde::Serialize)]
struct Payload {
    message: String,
}

#[tauri::command]
fn greet(window: Window, path: String) -> String {
    // std::thread::spawn(move || {
    let (pcm, length) = mp4::read(path.as_str()).unwrap();
    let probs = speech::get_speech_probs(pcm, length, 16000);
    let timestamps = speech::get_speech_timestamps(probs, length, 16000);
    customer::service::ffmpeg_pipe(path.as_str(), &timestamps, length).unwrap();
    println!("{:?}", timestamps);

    window
        .emit(
            "event-name",
            Payload {
                message: format!("have {} speech, length {}", timestamps.len(), length),
            },
        )
        .unwrap();
    // });
    format!("Hello from Rust")
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
