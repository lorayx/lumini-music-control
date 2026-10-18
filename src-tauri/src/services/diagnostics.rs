use std::{io::Write, path::PathBuf, sync::{Arc, Mutex}};

#[derive(Clone)]
pub struct DiagnosticsService { path: PathBuf, lock: Arc<Mutex<()>> }
impl DiagnosticsService {
    pub fn new(path: PathBuf) -> Self { Self { path, lock: Arc::new(Mutex::new(())) } }
    pub fn error(&self, component: &str, message: &str) {
        let Ok(_guard) = self.lock.lock() else { return; };
        if let Some(parent) = self.path.parent() { let _ = std::fs::create_dir_all(parent); }
        if std::fs::metadata(&self.path).map(|m| m.len() > 256 * 1024).unwrap_or(false) { let _ = std::fs::remove_file(self.path.with_extension("old.log")); let _ = std::fs::rename(&self.path, self.path.with_extension("old.log")); }
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) { let _ = writeln!(file, "{} [{}] {}", crate::model::now_ms(), component, message.replace(['\n','\r'], " ").chars().take(1200).collect::<String>()); }
    }
}
