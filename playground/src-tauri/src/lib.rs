use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

struct PrevWindow(Mutex<isize>);

#[tauri::command]
fn apply_spell(app: AppHandle, state: tauri::State<'_, PrevWindow>) {
    let hwnd_val = *state.0.lock().unwrap();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // Restore focus to the original window
    #[cfg(target_os = "windows")]
    unsafe {
        if hwnd_val != 0 {
            winapi::um::winuser::SetForegroundWindow(
                hwnd_val as winapi::shared::windef::HWND
            );
        }
    }

    // Wait for focus to settle before simulating paste
    std::thread::sleep(std::time::Duration::from_millis(50));

    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.key(Key::Control, Direction::Press);
        let _ = enigo.key(Key::Unicode('v'), Direction::Click);
        let _ = enigo.key(Key::Control, Direction::Release);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(PrevWindow(Mutex::new(0)))
        .setup(|app| {
            let shortcut = Shortcut::new(
                Some(Modifiers::CONTROL),
                Code::Space,
            );
            app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }

                // Save the currently focused window before we steal focus
                #[cfg(target_os = "windows")]
                {
                    let hwnd = unsafe { winapi::um::winuser::GetForegroundWindow() };
                    if let Some(state) = app.try_state::<PrevWindow>() {
                        *state.0.lock().unwrap() = hwnd as isize;
                    }
                }

                // Simulate Ctrl+C on the currently focused window
                if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
                    let _ = enigo.key(Key::Control, Direction::Press);
                    let _ = enigo.key(Key::Unicode('c'), Direction::Click);
                    let _ = enigo.key(Key::Control, Direction::Release);
                }

                // Brief pause for clipboard to update
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Show our window
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![apply_spell])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
