use tauri_plugin_global_shortcut::GlobalShortcutExt;
use crate::model::Settings;
pub const SHORTCUTS: [&str;3] = ["Ctrl+Alt+M", "Ctrl+Alt+Space", "Ctrl+Shift+F11"];
pub fn configure(app: &tauri::AppHandle, old: &Settings, new: &Settings) -> Result<(), String> {
    if !SHORTCUTS.contains(&new.hotkey.as_str()) { return Err("Недопустимая комбинация клавиш".into()); }
    let shortcuts = app.global_shortcut();
    if new.hotkeys && !shortcuts.is_registered(new.hotkey.as_str()) { shortcuts.register(new.hotkey.as_str()).map_err(|_| format!("{} уже занята. Выберите другую комбинацию.", new.hotkey))?; }
    if old.hotkeys && (!new.hotkeys || old.hotkey != new.hotkey) && shortcuts.is_registered(old.hotkey.as_str()) {
        if let Err(error) = shortcuts.unregister(old.hotkey.as_str()) { if new.hotkeys && old.hotkey != new.hotkey { let _ = shortcuts.unregister(new.hotkey.as_str()); } return Err(error.to_string()); }
    }
    Ok(())
}
