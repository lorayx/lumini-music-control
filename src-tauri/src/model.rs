use serde::{Deserialize, Serialize};

pub const TARGET_ID: &str = "ru.yandex.desktop.music";

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Controls { pub play: bool, pub pause: bool, pub next: bool, pub previous: bool, pub seek: bool }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub source_id: String, pub connected: bool, pub title: String, pub artist: String, pub album: String,
    pub cover: Option<String>, pub status: String, pub controls: Controls,
    pub position: f64, pub start: f64, pub end: f64, pub min_seek: f64, pub max_seek: f64,
    pub captured_at: u64, pub error: Option<String>,
}
impl Default for MediaState {
    fn default() -> Self { Self { source_id: String::new(), connected: false, title: String::new(), artist: String::new(), album: String::new(), cover: None, status: "Closed".into(), controls: Controls::default(), position: 0.0, start: 0.0, end: 0.0, min_seek: 0.0, max_seek: 0.0, captured_at: 0, error: None } }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings { pub theme: String, pub hotkeys: bool, pub hotkey: String, pub reduce_motion: bool, pub visualizer: bool, pub kick_reactive: bool, pub taskbar: bool, pub taskbar_offset: i32, pub player_id: String, pub launch_app_id: String, pub player_name: String, pub launch_on_play: bool, pub minimize_on_launch: bool, pub bootstrap_yandex: bool }
impl Default for Settings { fn default() -> Self { Self { theme: "system".into(), hotkeys: false, hotkey: "Ctrl+Alt+M".into(), reduce_motion: false, visualizer: true, kick_reactive: true, taskbar: true, taskbar_offset: 160, player_id: TARGET_ID.into(), launch_app_id: TARGET_ID.into(), player_name: "Яндекс Музыка".into(), launch_on_play: true, minimize_on_launch: true, bootstrap_yandex: true } } }

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackAction { Play, Pause, Next, Previous, Seek(f64), SeekTrack(SeekTarget) }

#[derive(Clone,Debug,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SeekTarget {pub value:f64,pub source_id:String,pub title:String,pub artist:String,pub album:String}
impl SeekTarget{pub fn matches(&self,source:&str,title:&str,artist:&str,album:&str)->bool{!self.title.trim().is_empty()&&self.source_id==source&&self.title==title&&self.artist==artist&&self.album==album}}

pub fn validate_seek(value: f64, min: f64, max: f64) -> Result<i64, String> {
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || value < min || value > max || value < 0.0 || value > (i64::MAX as f64 / 10_000_000.0) { return Err("Позиция вне допустимых границ".into()); }
    Ok((value * 10_000_000.0).round() as i64)
}
pub fn yandex_seek_ticks(value: f64, min: f64, max: f64) -> Result<i64, String> {
    let ticks = validate_seek(value,min,max)?;
    // Confirmed on the installed Yandex Music: 0 returns true but is ignored;
    // 10 ms changes the timeline. Never exceed the published seek range.
    Ok(if ticks == 0 && max >= 0.01 { 100_000 } else { ticks })
}
pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn old_settings_migrate_to_confirmed_yandex_without_changing_preferences(){
        let s:Settings=serde_json::from_str(r#"{"theme":"dark","hotkeys":false,"taskbar":false}"#).unwrap();
        assert_eq!(s.player_id,TARGET_ID);assert_eq!(s.launch_app_id,TARGET_ID);
        assert!(s.launch_on_play&&s.minimize_on_launch);assert!(s.visualizer);assert_eq!(s.theme,"dark");assert!(!s.taskbar);
    }
    #[test] fn seek_rejects_untrusted_values() { for v in [f64::NAN, f64::INFINITY, -1.0, 31.0] { assert!(validate_seek(v, 0.0, 30.0).is_err()); } }
    #[test] fn seek_uses_windows_ticks_and_published_bounds() { assert_eq!(validate_seek(10.25, 2.0, 30.0).unwrap(), 102_500_000); assert!(validate_seek(1.0, 2.0, 30.0).is_err()); assert!(validate_seek(2.0, 4.0, 3.0).is_err()); }
    #[test] fn settings_are_opt_in() { let s = Settings::default(); assert!(!s.hotkeys); assert_eq!(s.theme, "system"); }
    #[test] fn yandex_zero_workaround_respects_bounds() {
        assert_eq!(yandex_seek_ticks(0.0,0.0,30.0).unwrap(),100_000);
        assert_eq!(yandex_seek_ticks(0.0,0.0,0.005).unwrap(),0);
        assert!(yandex_seek_ticks(0.0,1.0,30.0).is_err());
        assert_eq!(yandex_seek_ticks(10.0,0.0,30.0).unwrap(),100_000_000);
    }
}

#[cfg(test)]mod guarded_seek_tests{use super::*;#[test]fn guarded_seek_wire_identity_is_exact(){let a:PlaybackAction=serde_json::from_str(r#"{"seekTrack":{"value":12.5,"sourceId":"player","title":"Song","artist":"Artist","album":"Album"}}"#).unwrap();let PlaybackAction::SeekTrack(t)=a else{panic!("wrong action")};assert!(t.matches("player","Song","Artist","Album"));assert!(!t.matches("other","Song","Artist","Album"));assert!(!t.matches("player","Next","Artist","Album"));assert!(!t.matches("player","Song","Other","Album"));assert!(!t.matches("player","Song","Artist","Other"));assert_eq!(t.value,12.5);}}
