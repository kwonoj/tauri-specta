#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use tauri_specta::*;

/// HELLO
/// WORLD
/// !!!!
#[tauri::command]
#[specta::specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}! You've been greeted from Rust!")
}

#[tauri::command]
#[specta::specta]
fn goodbye_world() -> impl Serialize + specta::Type {
    "Goodbye world :("
}

#[tauri::command]
#[specta::specta]
fn has_error() -> Result<&'static str, i32> {
    Err(32)
}

#[tauri::command]
#[specta::specta]
fn generic<T: tauri::Runtime>(_app: tauri::AppHandle<T>) {}

mod nested {
    use super::*;

    #[tauri::command]
    #[specta::specta]
    pub fn some_struct() -> MyStruct {
        MyStruct {
            some_field: "Hello World".into(),
        }
    }

    #[derive(Serialize, specta::Type)] // For Specta support you must add the `specta::Type` derive macro.
    pub struct MyStruct {
        some_field: String,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
pub struct DemoEvent(String);

#[derive(Serialize, Deserialize, Debug, Clone, specta::Type, tauri_specta::Event)]
pub struct EmptyEvent;

fn main() {
    let specta_builder = {
        let specta_builder = ts::builder()
            .commands(tauri_specta::collect_commands![
                hello_world,
                goodbye_world,
                has_error,
                nested::some_struct,
                generic::<tauri::Wry>
            ])
            .events(tauri_specta::collect_events![DemoEvent, EmptyEvent])
            .config(specta::ts::ExportConfig::default().formatter(specta::ts::formatter::prettier));

        #[cfg(debug_assertions)]
        let specta_builder = specta_builder.path("../src/bindings.ts");

        specta_builder.into_plugin()
    };

    tauri::Builder::default()
        .plugin(specta_builder)
        .setup(|app| {
            let handle = app.handle();

            DemoEvent::listen_any(&handle, |event| {
                dbg!(event.payload);
            });

            DemoEvent("Test".to_string()).emit_all(&handle).ok();

            EmptyEvent::listen_any(&handle, {
                let handle = handle.clone();
                move |_| {
                    EmptyEvent.emit_all(&handle).ok();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
