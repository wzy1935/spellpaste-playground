use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use serde::{Deserialize, Serialize};

// ---- App state ----

struct PrevWindow(Mutex<isize>);
struct SpellStore(Mutex<Vec<LoadedSpell>>);
struct CollectionsDir(PathBuf);
struct SelectedText(Mutex<String>);

// ---- Data structures ----

#[derive(Deserialize)]
struct IndexEntry {
    default: String,
}

#[derive(Deserialize)]
struct IndexSettings {
    #[serde(rename = "doPaste", default = "default_true")]
    do_paste: bool,
}

fn default_true() -> bool { true }

#[derive(Deserialize)]
struct SpellDef {
    trigger: String,
    description: Option<String>,
    entry: IndexEntry,
    settings: Option<IndexSettings>,
}

#[derive(Deserialize)]
struct CollectionIndex {
    spells: Vec<SpellDef>,
}

struct LoadedSpell {
    trigger: String,
    description: Option<String>,
    collection_dir: PathBuf,
    entry_cmd: String,
    do_paste: bool,
}

#[derive(Serialize, Clone)]
struct SpellInfo {
    trigger: String,
    description: Option<String>,
}

// ---- macOS platform module ----

#[cfg(target_os = "macos")]
mod macos {
    use objc::{class, msg_send, sel, sel_impl, runtime::Object};
    use std::ffi::c_void;

    type CGEventSourceRef = *mut c_void;
    type CGEventRef = *mut c_void;

    const KCG_EVENT_SOURCE_STATE_PRIVATE: i32 = -1;
    const KCG_HID_EVENT_TAP: u32 = 0;
    const KCG_EVENT_FLAG_MASK_COMMAND: u64 = 0x00100000;
    const KVK_ANSI_C: u16 = 0x08;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceCreate(stateID: i32) -> CGEventSourceRef;
        fn CGEventCreateKeyboardEvent(
            source: CGEventSourceRef,
            virtualKey: u16,
            keyDown: bool,
        ) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventPost(tap: u32, event: CGEventRef);
        fn CFRelease(cf: *mut c_void);
    }

    pub unsafe fn simulate_copy_private_source() {
        let source = CGEventSourceCreate(KCG_EVENT_SOURCE_STATE_PRIVATE);

        let key_down = CGEventCreateKeyboardEvent(source, KVK_ANSI_C, true);
        CGEventSetFlags(key_down, KCG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(KCG_HID_EVENT_TAP, key_down);
        CFRelease(key_down);

        let key_up = CGEventCreateKeyboardEvent(source, KVK_ANSI_C, false);
        CGEventSetFlags(key_up, KCG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(KCG_HID_EVENT_TAP, key_up);
        CFRelease(key_up);

        CFRelease(source);
    }

    pub unsafe fn get_frontmost_pid() -> i32 {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() { return 0; }
        msg_send![app, processIdentifier]
    }

    pub unsafe fn activate_pid(pid: i32) {
        let app: *mut Object = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if app.is_null() { return; }
        let _: () = msg_send![app, activateWithOptions: 1u64];
    }
}

// ---- OS helpers ----

fn get_collections_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let home = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default"));
    #[cfg(not(target_os = "windows"))]
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    home.join(".spellpaste").join("collections")
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

fn simulate_copy(_enigo: &mut Enigo) {
    #[cfg(target_os = "macos")]
    unsafe { macos::simulate_copy_private_source() };

    #[cfg(not(target_os = "macos"))]
    {
        let _ = _enigo.key(Key::Control, Direction::Press);
        let _ = _enigo.key(Key::Unicode('c'), Direction::Click);
        let _ = _enigo.key(Key::Control, Direction::Release);
    }
}

fn simulate_paste(enigo: &mut Enigo) {
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    let _ = enigo.key(modifier, Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
    let _ = enigo.key(modifier, Direction::Release);
}

// ---- Collections directory setup ----

fn ensure_collections_dir(dir: &Path) {
    if dir.exists() { return; }

    let _ = std::fs::create_dir_all(dir);

    let hello_dir = dir.join("hello");
    let _ = std::fs::create_dir(&hello_dir);
    let _ = std::fs::write(
        hello_dir.join("index.json"),
        r#"{
  "spells": [
    {
      "trigger": "hello",
      "description": "Generate \"Hello, World!\"",
      "entry": {
        "default": "echo Hello, World!"
      }
    }
  ]
}
"#,
    );
}

// ---- Collection loading ----

fn load_collections(dir: &Path) -> Vec<LoadedSpell> {
    let mut spells = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return spells };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let Ok(content) = std::fs::read_to_string(path.join("index.json")) else { continue };
        let Ok(index) = serde_json::from_str::<CollectionIndex>(&content) else { continue };
        for def in index.spells {
            spells.push(LoadedSpell {
                trigger: def.trigger,
                description: def.description,
                collection_dir: path.clone(),
                entry_cmd: def.entry.default,
                do_paste: def.settings.map(|s| s.do_paste).unwrap_or(true),
            });
        }
    }
    spells
}

// ---- Spell execution ----

fn execute_spell(entry_cmd: &str, collection_dir: &Path, input: &str) -> Result<String, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[cfg(target_os = "windows")]
    let (shell, flag) = ("cmd", "/C");
    #[cfg(not(target_os = "windows"))]
    let (shell, flag) = ("sh", "-c");

    let mut child = Command::new(shell)
        .arg(flag)
        .arg(entry_cmd)
        .current_dir(collection_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ---- Tauri commands ----

#[tauri::command]
fn get_spells(store: tauri::State<'_, SpellStore>) -> Vec<SpellInfo> {
    store.0.lock().unwrap()
        .iter()
        .map(|s| SpellInfo {
            trigger: s.trigger.clone(),
            description: s.description.clone(),
        })
        .collect()
}

#[tauri::command]
fn refresh_spells(
    store: tauri::State<'_, SpellStore>,
    dir: tauri::State<'_, CollectionsDir>,
) {
    *store.0.lock().unwrap() = load_collections(&dir.0);
}

#[tauri::command]
fn cancel(app: AppHandle, prev_window: tauri::State<'_, PrevWindow>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let prev = *prev_window.0.lock().unwrap();
    restore_prev_window(prev);
}

#[tauri::command]
fn apply_spell(
    trigger: String,
    app: AppHandle,
    prev_window: tauri::State<'_, PrevWindow>,
    store: tauri::State<'_, SpellStore>,
    selected: tauri::State<'_, SelectedText>,
) -> Result<(), String> {
    let (entry_cmd, collection_dir, do_paste) = {
        let spells = store.0.lock().unwrap();
        let spell = spells.iter()
            .find(|s| s.trigger == trigger)
            .ok_or_else(|| format!("Spell '{}' not found", trigger))?;
        (spell.entry_cmd.clone(), spell.collection_dir.clone(), spell.do_paste)
    };

    let input = selected.0.lock().unwrap().clone();

    let output = execute_spell(&entry_cmd, &collection_dir, &input)?;

    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(output.trim_end_matches('\n'));
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    let prev = *prev_window.0.lock().unwrap();
    restore_prev_window(prev);
    std::thread::sleep(std::time::Duration::from_millis(50));

    if do_paste {
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            simulate_paste(&mut enigo);
        }
    }

    Ok(())
}

// ---- Entry point ----

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let collections_dir = get_collections_dir();
    ensure_collections_dir(&collections_dir);
    let initial_spells = load_collections(&collections_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(PrevWindow(Mutex::new(0)))
        .manage(SpellStore(Mutex::new(initial_spells)))
        .manage(CollectionsDir(collections_dir))
        .manage(SelectedText(Mutex::new(String::new())))
        .setup(|app| {
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::Space);
            app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed { return; }

                if let Some(state) = app.try_state::<PrevWindow>() {
                    save_prev_window(&state);
                }

                let before = arboard::Clipboard::new()
                    .and_then(|mut c| c.get_text())
                    .unwrap_or_default();

                if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
                    simulate_copy(&mut enigo);
                }

                std::thread::sleep(std::time::Duration::from_millis(100));

                let after = arboard::Clipboard::new()
                    .and_then(|mut c| c.get_text())
                    .unwrap_or_default();

                // If clipboard didn't change, nothing was selected → use empty string
                let selected = if after != before { after } else { String::new() };
                if let Some(state) = app.try_state::<SelectedText>() {
                    *state.0.lock().unwrap() = selected;
                }

                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            })?;

            let refresh_item = MenuItem::with_id(app, "refresh", "Refresh Spells", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&refresh_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "refresh" => {
                        if let (Some(store), Some(dir)) = (
                            app.try_state::<SpellStore>(),
                            app.try_state::<CollectionsDir>(),
                        ) {
                            *store.0.lock().unwrap() = load_collections(&dir.0);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_spells, apply_spell, refresh_spells, cancel])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
