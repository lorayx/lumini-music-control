use std::sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}};
use tauri::Emitter;
use tokio::sync::{mpsc, oneshot};
use windows::{Foundation::TypedEventHandler, Media::Control::*};
use crate::model::{now_ms, Controls, MediaState, PlaybackAction};
use super::{diagnostics::DiagnosticsService, metadata, playback};

pub enum Message { Refresh(bool), Command(PlaybackAction, oneshot::Sender<Result<(), String>>), List(oneshot::Sender<Vec<String>>), Stop }
#[derive(Clone)]
pub struct MediaSessionService { pub tx: mpsc::Sender<Message>, pub snapshot: Arc<RwLock<MediaState>>, pub target: Arc<RwLock<String>> }
struct Subscription { session: GlobalSystemMediaTransportControlsSession, metadata: i64, playback: i64, timeline: i64, dirty: Arc<AtomicBool> }
impl Drop for Subscription {
    fn drop(&mut self) { let _ = self.session.RemoveMediaPropertiesChanged(self.metadata); let _ = self.session.RemovePlaybackInfoChanged(self.playback); let _ = self.session.RemoveTimelinePropertiesChanged(self.timeline); }
}
impl Subscription {
    fn new(session: GlobalSystemMediaTransportControlsSession, tx: &mpsc::Sender<Message>) -> Result<Self, String> {
        let mut sub = Self { session, metadata: 0, playback: 0, timeline: 0, dirty: Arc::new(AtomicBool::new(true)) };
        let sender = tx.clone();
        let dirty = sub.dirty.clone();
        sub.metadata = sub.session.MediaPropertiesChanged(&TypedEventHandler::new(move |_,_| { dirty.store(true,Ordering::Release); let _ = sender.try_send(Message::Refresh(true)); Ok(()) })).map_err(|e| e.to_string())?;
        let sender = tx.clone();
        sub.playback = sub.session.PlaybackInfoChanged(&TypedEventHandler::new(move |_,_| { let _ = sender.try_send(Message::Refresh(false)); Ok(()) })).map_err(|e| e.to_string())?;
        let sender = tx.clone();
        sub.timeline = sub.session.TimelinePropertiesChanged(&TypedEventHandler::new(move |_,_| { let _ = sender.try_send(Message::Refresh(false)); Ok(()) })).map_err(|e| e.to_string())?;
        Ok(sub)
    }
}
fn publish(service: &MediaSessionService, app: &tauri::AppHandle, state: MediaState) {
    if let Ok(mut snapshot) = service.snapshot.write() { *snapshot = state.clone(); }
    // The WebView receives real state only; no network or synthetic track data.
    let _ = app.emit("media-state", state);
}
async fn snapshot(sub: &Subscription, mut state: MediaState, read_metadata: bool) -> Result<MediaState, String> {
    state.connected = true; state.error = None;
    if read_metadata {
        match metadata::read(&sub.session).await {
            Ok(m) => { state.title = m.title; state.artist = m.artist; state.album = m.album; state.cover = m.cover; state.error = m.warning; },
            Err(e) => { state.title.clear(); state.artist.clear(); state.album.clear(); state.cover = None; state.error = Some(e); }
        }
    }
    let p = sub.session.GetPlaybackInfo().map_err(|e| e.to_string())?;
    state.status = match p.PlaybackStatus().map_err(|e| e.to_string())? {
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => "Playing",
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => "Paused",
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => "Stopped",
        _ => "Opened",
    }.into();
    let c = p.Controls().map_err(|e| e.to_string())?;
    state.controls = Controls { play: c.IsPlayEnabled().unwrap_or(false), pause: c.IsPauseEnabled().unwrap_or(false), next: c.IsNextEnabled().unwrap_or(false), previous: c.IsPreviousEnabled().unwrap_or(false), seek: c.IsPlaybackPositionEnabled().unwrap_or(false) };
    let t = sub.session.GetTimelineProperties().map_err(|e| e.to_string())?;
    state.position = t.Position().map_err(|e| e.to_string())?.Duration as f64 / 1e7;
    state.start = t.StartTime().map_err(|e| e.to_string())?.Duration as f64 / 1e7;
    state.end = t.EndTime().map_err(|e| e.to_string())?.Duration as f64 / 1e7;
    state.min_seek = t.MinSeekTime().map_err(|e| e.to_string())?.Duration as f64 / 1e7;
    state.max_seek = t.MaxSeekTime().map_err(|e| e.to_string())?.Duration as f64 / 1e7;
    // Timeline.Position is relative to LastUpdatedTime, not the time of this read.
    let updated = t.LastUpdatedTime().map_err(|e| e.to_string())?.UniversalTime;
    state.captured_at = ((updated / 10_000) - 11_644_473_600_000).max(0) as u64;
    if state.captured_at == 0 || state.captured_at > now_ms() { state.captured_at = now_ms(); }
    Ok(state)
}

impl MediaSessionService {
    pub fn start(app: tauri::AppHandle, diagnostics: DiagnosticsService, target: String) -> Self {
        let (tx, mut rx) = mpsc::channel(32);
        let service = Self { tx, snapshot: Arc::new(RwLock::new(MediaState::default())), target: Arc::new(RwLock::new(target)) };
        let shared = service.clone();
        std::thread::Builder::new().name("lumini-media".into()).spawn(move || {
            use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
            if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                diagnostics.error("WinRT", &error.to_string());
                publish(&shared, &app, MediaState { error: Some(error.to_string()), ..MediaState::default() });
                return;
            }
            let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("media runtime");
            runtime.block_on(async {
                loop {
                    let manager = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync().map_err(|e| e.to_string()) {
                        Ok(op) => metadata::wait(op).await,
                        Err(e) => Err(e),
                    };
                    let manager = match manager {
                        Ok(m) => m,
                        Err(error) => {
                            diagnostics.error("MediaSessionService", &error);
                            publish(&shared, &app, MediaState { error: Some(error), ..MediaState::default() });
                            tokio::select! { _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => continue, message = rx.recv() => { match message { Some(Message::Stop) | None => break, Some(Message::Command(_, reply)) => { let _ = reply.send(Err("Медиасервис недоступен".into())); }, _ => {} } } }
                            continue;
                        }
                    };
                    let sender = shared.tx.clone();
                    let token = match manager.SessionsChanged(&TypedEventHandler::new(move |_,_| { let _ = sender.try_send(Message::Refresh(true)); Ok(()) })) {
                        Ok(token) => token,
                        Err(error) => {
                            diagnostics.error("SessionsChanged", &error.to_string());
                            publish(&shared, &app, MediaState { error: Some(error.to_string()), ..MediaState::default() });
                            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                            continue;
                        }
                    };
                    let mut current: Option<Subscription> = None;
                    let mut next = Some(Message::Refresh(true));
                    let mut stop = false;
                    while let Some(message) = if next.is_some() { next.take() } else { rx.recv().await } {
                        if matches!(message, Message::Stop) { stop = true; break; }
                        if let Message::List(reply)=message {
                            let mut ids=Vec::new();
                            if let Ok(sessions)=manager.GetSessions(){for session in sessions {if let Ok(id)=session.SourceAppUserModelId(){let id=id.to_string();if !ids.contains(&id){ids.push(id);}}}}
                            let _=reply.send(ids);continue;
                        }
                        let result: Result<MediaState, String> = async {
                            let target=shared.target.read().map_err(|_|"Ошибка выбора плеера")?.clone();
                            let sessions = manager.GetSessions().map_err(|e| e.to_string())?;
                            let mut matches = Vec::new();
                            for session in sessions { if session.SourceAppUserModelId().map_err(|e| e.to_string())?.to_string() == target { matches.push(session); } }
                            if matches.len() != 1 {
                                current = None;
                                if let Message::Command(_, reply) = message { let _ = reply.send(Err("Нет однозначной сессии выбранного плеера".into())); }
                                return Ok(MediaState { error: if matches.len() > 1 { Some("Найдено несколько сессий выбранного плеера".into()) } else { None }, ..MediaState::default() });
                            }
                            let changed = current.as_ref().map(|s| s.session != matches[0]).unwrap_or(true);
                            if changed { current = None; current = Some(Subscription::new(matches.remove(0), &shared.tx)?); }
                            let sub = current.as_ref().ok_or("Сессия исчезла")?;
                            let read_metadata = match message {
                                Message::Command(action, reply) => { if !reply.is_closed() { let result = playback::execute(&sub.session, action).await; let _ = reply.send(result); } true },
                                Message::Refresh(full) => full || changed,
                                Message::Stop | Message::List(_) => false,
                            };
                            let read_metadata = sub.dirty.swap(false,Ordering::AcqRel) || read_metadata;
                            let prior = if changed { MediaState::default() } else { shared.snapshot.read().map_err(|_| "Ошибка состояния")?.clone() };
                            snapshot(sub, prior, read_metadata).await.map(|mut s| {s.source_id=target;s})
                        }.await;
                        match result { Ok(state) => publish(&shared, &app, state), Err(error) => { diagnostics.error("MediaSessionService", &error); publish(&shared, &app, MediaState { error: Some(error), ..MediaState::default() }); } }
                    }
                    drop(current);
                    let _ = manager.RemoveSessionsChanged(token);
                    if stop { break; }
                }
            });
            unsafe { RoUninitialize(); }
        }).expect("media worker");
        service
    }
    pub fn select(&self, app:&tauri::AppHandle, target:String) {
        if let Ok(mut selected)=self.target.write(){*selected=target;}
        publish(self,app,MediaState::default());
        let _=self.tx.try_send(Message::Refresh(true));
    }
    pub async fn active_players(&self)->Result<Vec<String>,String>{
        let (reply,receiver)=oneshot::channel();
        self.tx.try_send(Message::List(reply)).map_err(|_|"Медиасервис занят")?;
        tokio::time::timeout(std::time::Duration::from_secs(4),receiver).await.map_err(|_|"Медиасервис не ответил")?.map_err(|_|"Медиасервис остановлен".into())
    }
    pub async fn command(&self, action: PlaybackAction) -> Result<(), String> {
        let (reply, receiver) = oneshot::channel();
        self.tx.try_send(Message::Command(action, reply)).map_err(|_| "Медиасервис занят, повторите команду")?;
        tokio::time::timeout(std::time::Duration::from_secs(12), receiver).await.map_err(|_| "Команда не получила ответа")?.map_err(|_| "Медиасервис остановлен")?
    }
}
