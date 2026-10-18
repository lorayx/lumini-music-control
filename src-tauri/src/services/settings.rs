use crate::model::Settings;
use std::path::PathBuf;
pub struct SettingsService { pub path: PathBuf }
impl SettingsService {
    pub fn load(&self) -> Result<Settings, String> {
        match std::fs::read(&self.path) { Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()), Err(e) => Err(e.to_string()) }
    }
    pub fn save(&self, settings: &Settings) -> Result<(), String> {
        if !["system", "light", "dark"].contains(&settings.theme.as_str()) { return Err("Неизвестная тема".into()); }
        if !super::hotkey::SHORTCUTS.contains(&settings.hotkey.as_str()) { return Err("Недопустимая комбинация клавиш".into()); }
        if !(8..=1200).contains(&settings.taskbar_offset) { return Err("Отступ должен быть от 8 до 1200".into()); }
        for id in [&settings.player_id,&settings.launch_app_id] {
            if id.is_empty() || id.len()>1024 || id.chars().any(char::is_control) {return Err("Некорректный идентификатор плеера".into());}
        }
        if settings.player_name.trim().is_empty() || settings.player_name.len()>256 {return Err("Некорректное название плеера".into());}
        if let Some(parent) = self.path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        let temporary = self.path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(temporary, &self.path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_and_invalid_theme_protect_existing_settings() {
        let path = std::env::temp_dir().join(format!("lumini-test-{}-{}.json", std::process::id(), crate::model::now_ms()));
        let service = SettingsService { path: path.clone() };
        assert!(!service.load().unwrap().hotkeys);
        let settings = Settings { theme: "dark".into(), hotkeys: true, reduce_motion: true, ..Settings::default() };
        service.save(&settings).unwrap();
        assert!(service.load().unwrap().hotkeys);
        assert_eq!(service.load().unwrap().theme, "dark");
        assert!(service.save(&Settings { theme: "untrusted".into(), ..settings }).is_err());
        assert_eq!(service.load().unwrap().theme, "dark");
        assert!(service.save(&Settings{player_id:"bad\nplayer".into(),..Settings::default()}).is_err());
        assert_eq!(service.load().unwrap().theme,"dark");
        std::fs::write(&path, b"not JSON").unwrap();
        assert!(service.load().is_err());
        let _ = std::fs::remove_file(path);
    }
}
