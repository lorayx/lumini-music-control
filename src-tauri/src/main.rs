#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod model;
mod services;
use model::{MediaState, PlaybackAction, Settings};
use services::{media_session::{MediaSessionService, Message}, diagnostics::DiagnosticsService, settings::SettingsService};
use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
async fn get_yandex_features(service:State<'_,MediaSessionService>)->Result<services::yandex_controls::YandexFeatures,String>{
 let state=service.snapshot.read().map_err(|_|"Состояние недоступно")?.clone();tauri::async_runtime::spawn_blocking(move||services::yandex_controls::inspect(state)).await.map_err(|e|e.to_string())
}
#[tauri::command]
async fn yandex_action(action:services::yandex_controls::Action,title:String,artist:String,desired:Option<bool>,service:State<'_,MediaSessionService>)->Result<services::yandex_controls::YandexFeatures,String>{
 let state=service.snapshot.read().map_err(|_|"Состояние недоступно")?.clone();if state.title!=title||state.artist!=artist{return Err("Трек уже сменился. Повторите действие".into());}
 tauri::async_runtime::spawn_blocking(move||services::yandex_controls::perform(action,state,desired)).await.map_err(|e|e.to_string())?
}
#[tauri::command]
fn get_audio_reactivity(app:tauri::AppHandle)->services::audio_reactivity::AudioLevel {app.state::<services::audio_reactivity::AudioService>().latest.read().map(|s|s.clone()).unwrap_or_default()}
#[tauri::command]
fn get_media(service: State<'_, MediaSessionService>) -> Result<MediaState,String> { service.snapshot.read().map(|s| s.clone()).map_err(|_| "Состояние недоступно".into()) }
#[tauri::command]
async fn playback(app:tauri::AppHandle, action: PlaybackAction, service: State<'_,MediaSessionService>, settings:State<'_,SettingsService>) -> Result<(),String> {
    if matches!(action,PlaybackAction::Play) {services::launcher::start_and_play(app,service.inner().clone(),settings.load()?).await} else {service.command(action).await}
}
#[tauri::command]
async fn get_players(app:tauri::AppHandle)->Result<Vec<services::launcher::InstalledPlayer>,String>{
    tauri::async_runtime::spawn_blocking(move||app.state::<services::launcher::Launcher>().installed()).await.map_err(|e|e.to_string())?
}
#[tauri::command]
async fn get_active_players(media:State<'_,MediaSessionService>)->Result<Vec<String>,String>{media.active_players().await}
#[tauri::command]
fn show_from_taskbar(app: tauri::AppHandle) -> Result<(),String> {
    let w=app.get_webview_window("taskbar").ok_or("Полоска не найдена")?;
    let p=w.outer_position().map_err(|e|e.to_string())?;
    let s=w.outer_size().map_err(|e|e.to_string())?;
    let anchor_y=p.y as f64+(s.height as f64-40.0*w.scale_factor().unwrap_or(1.0)).max(0.0);
    services::window::show(&app,Some((p.x as f64+s.width as f64/2.0,anchor_y)))
}
#[tauri::command]
fn hide_panel(app: tauri::AppHandle) { services::window::hide(&app,false); }
#[tauri::command]
fn get_taskbar_light_theme()->bool{services::taskbar::light_theme()}
#[tauri::command]
fn set_taskbar_width(app:tauri::AppHandle,width:u32)->Result<(),String>{services::taskbar::set_content_width(&app,width)}
#[tauri::command]
fn set_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(),String> { services::window::resize(&app,expanded) }
#[tauri::command]
fn get_settings(service: State<'_,SettingsService>) -> Result<Settings,String> { service.load() }
#[tauri::command]
fn set_settings(app: tauri::AppHandle, service: State<'_,SettingsService>, settings: Settings) -> Result<(),String> {
    let old = service.load()?;
    if old.player_id!=settings.player_id && app.state::<services::launcher::Launcher>().busy.load(std::sync::atomic::Ordering::Acquire) {return Err("Дождитесь завершения запуска плеера".into());}
    services::hotkey::configure(&app,&old,&settings)?;
    if let Err(error) = service.save(&settings) { let _=services::hotkey::configure(&app,&settings,&old); return Err(error); }
    if old.player_id!=settings.player_id {app.state::<MediaSessionService>().select(&app,settings.player_id.clone());}
    let state=app.state::<services::taskbar::TaskbarState>();
    state.enabled.store(settings.taskbar,std::sync::atomic::Ordering::Relaxed);
    state.offset.store(settings.taskbar_offset,std::sync::atomic::Ordering::Relaxed);
    app.state::<services::audio_reactivity::AudioService>().configure(&settings);
    let _=app.emit("settings-changed",&settings);
    Ok(())
}
#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool,String> { app.autolaunch().is_enabled().map_err(|e| e.to_string()) }
#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(),String> { if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() }.map_err(|e| e.to_string()) }
#[tauri::command]
async fn open_player(app:tauri::AppHandle, settings:State<'_,SettingsService>) -> Result<(),String> {
    let settings=settings.load()?;
    tauri::async_runtime::spawn_blocking(move||app.state::<services::launcher::Launcher>().launch(&settings,false)).await.map_err(|e|e.to_string())?
}
#[tauri::command]
fn motion_allowed() -> bool { windows::UI::ViewManagement::UISettings::new().and_then(|s| s.AnimationsEnabled()).unwrap_or(false) }

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app,_,_| { let _=services::window::show(app,None); }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app,_,event| { if event.state()==tauri_plugin_global_shortcut::ShortcutState::Pressed { let _=services::window::show(app,None); } }).build())
        .invoke_handler(tauri::generate_handler![get_yandex_features,yandex_action,get_audio_reactivity,get_media,playback,show_from_taskbar,hide_panel,get_taskbar_light_theme,set_taskbar_width,set_expanded,get_settings,set_settings,get_autostart,set_autostart,open_player,get_players,get_active_players,motion_allowed])
        .setup(|app| {
            app.manage(services::window::WindowManager::default());
            let data=app.path().app_config_dir()?;
            let diagnostic=DiagnosticsService::new(app.path().app_log_dir()?.join("errors.log"));
            let settings=SettingsService { path: data.join("settings.json") };
            match settings.load() { Ok(s)=>{ if let Err(e)=services::hotkey::configure(app.handle(),&Settings::default(),&s) { diagnostic.error("HotkeyService",&e); } }, Err(e)=>diagnostic.error("SettingsService",&e) }
            let initial=settings.load().unwrap_or_default();
            app.manage(services::launcher::Launcher::default());
            app.manage(settings);
            app.manage(MediaSessionService::start(app.handle().clone(),diagnostic.clone(),initial.player_id.clone()));
            app.manage(services::audio_reactivity::AudioService::start(app.handle(),&initial));
            app.manage(diagnostic);
            services::tray::create(app.handle())?;
            for label in ["main","taskbar"] {
                if let Some(w)=app.get_webview_window(label) {w.set_decorations(false)?;w.set_shadow(false)?;w.set_skip_taskbar(true)?;}
                services::window::configure_label(app.handle(),label)?;
            }
            services::window::install_taskbar_frame(app.handle())?;
            if let Some(w)=app.get_webview_window("taskbar") {
                w.on_window_event(|event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();}});
            }
            services::taskbar::start(app.handle(),initial.taskbar,initial.taskbar_offset);
            if let Some(w)=app.get_webview_window("main") {
                w.on_window_event({ let app=app.handle().clone(); move |event| match event {
                    tauri::WindowEvent::CloseRequested { api,.. }=> { api.prevent_close(); services::window::hide(&app,false); },
                    tauri::WindowEvent::Focused(focused)=> { if *focused {let _=app.emit("panel-visible",true);} else {services::window::hide(&app,true);} }, _=>{}
                }});
            }
            Ok(())
        }).build(tauri::generate_context!()).expect("LUMINI startup failed");
    app.run(|app,event| { if let tauri::RunEvent::Exit = event { if let Some(audio)=app.try_state::<services::audio_reactivity::AudioService>(){audio.stop.store(true,std::sync::atomic::Ordering::Release);} if let Some(state)=app.try_state::<services::taskbar::TaskbarState>() {state.stop.store(true,std::sync::atomic::Ordering::Relaxed); } if let Some(media)=app.try_state::<MediaSessionService>() { let _=media.tx.try_send(Message::Stop); } } });
}
