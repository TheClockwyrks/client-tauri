//! The desktop binary, which runs the shell library beside it.

// A release build on Windows is a GUI application, which opens no console
// window beside the shell's own.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    test_cabinet_client_tauri_lib::run();
}
