use crate::model::{yandex_seek_ticks, PlaybackAction};
use windows::Media::Control::GlobalSystemMediaTransportControlsSession;
use super::metadata::wait;

pub async fn execute(session: &GlobalSystemMediaTransportControlsSession, action: PlaybackAction) -> Result<(), String> {
    let action=match action {
        PlaybackAction::SeekTrack(target)=>{
            let source=session.SourceAppUserModelId().map_err(|e|e.to_string())?.to_string();
            let current=wait(session.TryGetMediaPropertiesAsync().map_err(|e|e.to_string())?).await?;
            if !target.matches(&source,&current.Title().map_err(|e|e.to_string())?.to_string(),&current.Artist().map_err(|e|e.to_string())?.to_string(),&current.AlbumTitle().map_err(|e|e.to_string())?.to_string()){return Err("Трек уже сменился. Перемотка отменена".into());}
            PlaybackAction::Seek(target.value)
        },other=>other
    };
    let controls = session.GetPlaybackInfo().and_then(|p| p.Controls()).map_err(|e| e.to_string())?;
    let unsupported = || "Эта команда сейчас недоступна в выбранном плеере".to_string();
    let op = match action {
        PlaybackAction::SeekTrack(_)=>unreachable!("guarded seek normalized above"),
        PlaybackAction::Play => { if !controls.IsPlayEnabled().unwrap_or(false) { return Err(unsupported()); } session.TryPlayAsync() },
        PlaybackAction::Pause => { if !controls.IsPauseEnabled().unwrap_or(false) { return Err(unsupported()); } session.TryPauseAsync() },
        PlaybackAction::Next => { if !controls.IsNextEnabled().unwrap_or(false) { return Err(unsupported()); } session.TrySkipNextAsync() },
        PlaybackAction::Previous => { if !controls.IsPreviousEnabled().unwrap_or(false) { return Err(unsupported()); } session.TrySkipPreviousAsync() },
        PlaybackAction::Seek(value) => {
            if !controls.IsPlaybackPositionEnabled().unwrap_or(false) { return Err(unsupported()); }
            let t = session.GetTimelineProperties().map_err(|e| e.to_string())?;
            let seek=if session.SourceAppUserModelId().map_err(|e|e.to_string())?.to_string()==crate::model::TARGET_ID {yandex_seek_ticks} else {crate::model::validate_seek};
            let ticks = seek(value, t.MinSeekTime().map_err(|e| e.to_string())?.Duration as f64 / 1e7, t.MaxSeekTime().map_err(|e| e.to_string())?.Duration as f64 / 1e7)?;
            session.TryChangePlaybackPositionAsync(ticks)
        }
    }.map_err(|e| e.to_string())?;
    if wait(op).await? { Ok(()) } else { Err("Плеер отклонил команду".into()) }
}
