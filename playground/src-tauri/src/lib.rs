use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

// Stores the previous window as an isize.
// Windows: HWND cast to isize
// macOS:   process ID (i32) cast to isize
struct PrevWindow(Mutex<isize>);

#[cfg(target_os = "macos")]
mod macos {
    use objc::{class, msg_send, sel, sel_impl, runtime::Object};

    pub unsafe fn get_frontmost_pid() -> i32 {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return 0;
        }
        msg_send![app, processIdentifier]
    }

    pub unsafe fn activate_pid(pid: i32) {
        let app: *mut Object = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if app.is_null() {
            return;
        }
        // NSApplicationActivateIgnoringOtherApps = 1
        let _: () = msg_send![app, activateWithOptions: 1u64];
    }
}

fn save_prev_window(state: &PrevWindow) {
    #[cfg(target_os = "windows")]
    {
        let hwnd = unsafe { winapi::um::winuser::GetForegroundWindow() };
        *state.0.lock().unwrap() = hwnd as isize;
    }
    #[cfg(target_os = "macos")]
    {
        let pid = unsafe { macos::get_frontmost_pid() };
        *state.0.lock().unwrap() = pid as isize;
    }
}

fn simulate_copy(enigo: &mut Enigo) {
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    let _ = enigo.key(modifier, Direction::Press);
    let _ = enigo.key(Key::Unicode('c'), Direction::Click);
    let _ = enigo.key(modifier, Direction::Release);
}

fn simulate_paste(enigo: &mut Enigo) {
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    let _ = enigo.key(modifier, Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
    let _ = enigo.key(modifier, Direction::Release);
}

fn restore_prev_window(val: isize) {
    #[cfg(target_os = "windows")]
    unsafe {
        if val != 0 {
            winapi::um::winuser::SetForegroundWindow(
                val as winapi::shared::windef::HWND
            );
        }
    }
    #[cfg(target_os = "macos")]
    unsafe {
        if val != 0 {
            macos::activate_pid(val as i32);
        }
    }
}

#[tauri::command]
fn apply_spell(app: AppHandle, state: tauri::State<'_, PrevWindow>) {
    let prev = *state.0.lock().unwrap();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    restore_prev_window(prev);

    std::thread::sleep(std::time::Duration::from_millis(50));

    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        simulate_paste(&mut enigo);
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

                if let Some(state) = app.try_state::<PrevWindow>() {
                    save_prev_window(&state);
                }

                if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
                    simulate_copy(&mut enigo);
                }

                std::thread::sleep(std::time::Duration::from_millis(100));

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
