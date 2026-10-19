use std::{sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},os::windows::process::CommandExt,io::Read,process::Stdio};
use serde::{Serialize,Deserialize};
use tauri::{Emitter,Manager};
use windows::{core::HSTRING,Win32::{Foundation::{HWND,LPARAM},UI::{Shell::{ShellExecuteExW,SHELLEXECUTEINFOW,SEE_MASK_NOCLOSEPROCESS,SEE_MASK_FLAG_NO_UI},WindowsAndMessaging::{EnumWindows,GetWindowThreadProcessId,IsWindowVisible,IsIconic,ShowWindowAsync,SW_SHOWMINNOACTIVE,SW_SHOWNORMAL}}}};
use crate::model::{Settings,PlaybackAction,TARGET_ID};
use super::media_session::MediaSessionService;

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct InstalledPlayer {pub name:String,pub app_id:String}
#[derive(Default)]
pub struct Launcher {pub busy:Arc<AtomicBool>,catalog:Mutex<Option<Vec<InstalledPlayer>>>}
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
struct Progress {busy:bool,message:String}
fn progress(app:&tauri::AppHandle,busy:bool,message:&str){let _=app.emit("player-startup",Progress{busy,message:message.into()});}
struct LaunchGuard {busy:Arc<AtomicBool>,active:Arc<AtomicBool>,app:tauri::AppHandle}
impl Drop for LaunchGuard {fn drop(&mut self){self.active.store(false,Ordering::Release);self.busy.store(false,Ordering::Release);progress(&self.app,false,"");}}
#[link(name="kernel32")]
unsafe extern "system" {
 fn GetProcessId(handle:*mut std::ffi::c_void)->u32;
 fn CloseHandle(handle:*mut std::ffi::c_void)->i32;
 fn OpenProcess(access:u32,inherit:i32,pid:u32)->*mut std::ffi::c_void;
 fn QueryFullProcessImageNameW(handle:*mut std::ffi::c_void,flags:u32,path:*mut u16,size:*mut u32)->i32;
}
pub(super) fn process_path(pid:u32)->Option<std::path::PathBuf>{unsafe{
 let handle=OpenProcess(0x1000,0,pid);if handle.is_null(){return None;}
 let mut buffer=vec![0u16;32768];let mut size=buffer.len() as u32;
 let success=QueryFullProcessImageNameW(handle,0,buffer.as_mut_ptr(),&mut size);CloseHandle(handle);
 if success==0 {None}else{Some(std::path::PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize]))) }
}}
struct WindowsFor {pid:u32,found:bool,minimize:bool,path:Option<std::path::PathBuf>}
unsafe extern "system" fn visit(window:HWND,data:LPARAM)->windows::core::BOOL{
 let state=&mut *(data.0 as *mut WindowsFor);let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));
 if state.pid==pid || state.path.as_ref().map(|p|process_path(pid).as_ref()==Some(p)).unwrap_or(false){
  let mut title=[0u16;256];let len=windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(window,&mut title);
  if len==0||!String::from_utf16_lossy(&title[..len as usize]).starts_with("Яндекс Музыка"){return true.into();}
  state.found=true;
  if state.minimize&&IsWindowVisible(window).as_bool(){
   if IsIconic(window).as_bool(){state.minimize=false;return true.into();}
   let mut rect=windows::Win32::Foundation::RECT::default();
   if windows::Win32::UI::WindowsAndMessaging::GetWindowRect(window,&mut rect).is_ok()&&rect.right-rect.left>=400&&rect.bottom-rect.top>=250 {
    if !IsIconic(window).as_bool(){let _=ShowWindowAsync(window,SW_SHOWMINNOACTIVE);}
    state.minimize=false;
   }
  }
 }
 true.into()
}
impl Launcher {
 pub fn yandex_window_exists(&self)->bool{
  let Some(local)=std::env::var_os("LOCALAPPDATA")else{return false;};
  let path=std::path::PathBuf::from(local).join("Programs/YandexMusic/Яндекс Музыка.exe");
  let mut state=WindowsFor{pid:0,found:false,minimize:false,path:Some(path)};
  unsafe{let _=EnumWindows(Some(visit),LPARAM((&mut state as *mut WindowsFor) as isize));}state.found
 }
 pub fn finish_minimized(&self,settings:&Settings){
  if !settings.minimize_on_launch||settings.launch_app_id!=TARGET_ID{return;}
  if let Some(local)=std::env::var_os("LOCALAPPDATA"){
   let path=std::path::PathBuf::from(local).join("Programs/YandexMusic/Яндекс Музыка.exe");
   let mut state=WindowsFor{pid:0,found:false,minimize:true,path:Some(path)};
   unsafe{let _=EnumWindows(Some(visit),LPARAM((&mut state as *mut WindowsFor) as isize));}
  }
 }
 pub fn installed(&self)->Result<Vec<InstalledPlayer>,String>{
  if let Some(rows)=self.catalog.lock().map_err(|_|"Список приложений занят")?.clone(){return Ok(rows);}
  let script="[Console]::OutputEncoding=[Text.UTF8Encoding]::new(); ConvertTo-Json -Compress -InputObject @(Get-StartApps | ForEach-Object { @{name=$_.Name;appId=$_.AppID} })";
  let system=std::env::var_os("SystemRoot").ok_or("Папка Windows не найдена")?;
  let mut child=std::process::Command::new(std::path::PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe"))
   .args(["-NoProfile","-NonInteractive","-Command",script]).creation_flags(0x08000000).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
  let mut stdout=child.stdout.take().ok_or("Нет вывода списка приложений")?;
  let reader=std::thread::spawn(move||{let mut data=Vec::new();let result=stdout.read_to_end(&mut data);result.map(|_|data)});
  let deadline=std::time::Instant::now()+std::time::Duration::from_secs(8);
  let status=loop {if let Some(status)=child.try_wait().map_err(|e|e.to_string())?{break status;}if std::time::Instant::now()>=deadline{let _=child.kill();let _=child.wait();let _=reader.join();return Err("Windows не вернула список приложений за 8 секунд".into());}std::thread::sleep(std::time::Duration::from_millis(50));};
  let output=reader.join().map_err(|_|"Ошибка чтения списка приложений")?.map_err(|e|e.to_string())?;
  if !status.success(){return Err("Не удалось получить установленные приложения Windows".into());}
  let rows:Vec<InstalledPlayer>=serde_json::from_slice(&output).map_err(|e|format!("Список приложений: {e}"))?;
  let rows:Vec<InstalledPlayer>=rows.into_iter().filter(|p|p.app_id!="io.lumini.musiccontrol").collect();
  *self.catalog.lock().map_err(|_|"Список приложений занят")?=Some(rows.clone());Ok(rows)
 }
 pub fn launch(&self,settings:&Settings,minimized:bool)->Result<(),String>{
  if !self.installed()?.iter().any(|p|p.app_id==settings.launch_app_id){return Err("Выбранное приложение не установлено. Выберите его в настройках".into());}
  let known=if settings.launch_app_id==TARGET_ID {
   std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).map(|p|p.join("Programs/YandexMusic/Яндекс Музыка.exe")).filter(|p|p.is_file())
  }else{None};
  if minimized {if let Some(path)=&known {let mut state=WindowsFor{pid:0,found:false,minimize:false,path:Some(path.clone())};unsafe{let _=EnumWindows(Some(visit),LPARAM((&mut state as *mut WindowsFor) as isize));}if state.found{return Ok(());}}}
  let file=HSTRING::from(known.as_ref().map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(||format!("shell:AppsFolder\\{}",settings.launch_app_id)));
  // Chromium documents this switch for assistive-technology clients. It
  // keeps the public accessibility tree available even before the first Play.
  let parameters=HSTRING::from(if known.is_some(){"--force-renderer-accessibility=complete"}else{""});
  let mut info=SHELLEXECUTEINFOW{cbSize:std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,fMask:SEE_MASK_NOCLOSEPROCESS|SEE_MASK_FLAG_NO_UI,lpFile:windows::core::PCWSTR(file.as_ptr()),lpParameters:windows::core::PCWSTR(parameters.as_ptr()),nShow:if minimized{SW_SHOWMINNOACTIVE.0}else{SW_SHOWNORMAL.0},..Default::default()};
  unsafe{ShellExecuteExW(&mut info).map_err(|e|format!("Не удалось запустить плеер: {e}"))?;}
  let pid=if !info.hProcess.is_invalid(){unsafe{let pid=GetProcessId(info.hProcess.0);CloseHandle(info.hProcess.0);pid}}else{0};
  if minimized&&pid!=0&&known.is_some(){let path=known.unwrap();std::thread::spawn(move||{
   for _ in 0..100 {if process_path(pid).as_ref()!=Some(&path){break;}let mut state=WindowsFor{pid,found:false,minimize:true,path:None};unsafe{let _=EnumWindows(Some(visit),LPARAM((&mut state as *mut WindowsFor) as isize));}if !state.minimize{break;}std::thread::sleep(std::time::Duration::from_millis(150));}
  });}
  Ok(())
 }
}
pub async fn start_and_play(app:tauri::AppHandle,media:MediaSessionService,settings:Settings)->Result<(),String>{
 let busy=app.state::<Launcher>().busy.clone();
 if busy.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){return Err("Плеер уже запускается".into());}
 let active=Arc::new(AtomicBool::new(true));
 let _guard=LaunchGuard{busy,active:active.clone(),app:app.clone()};
 let state=media.snapshot.read().map_err(|_|"Состояние недоступно")?.clone();
 let launched=!state.connected||(settings.launch_app_id==TARGET_ID&&!app.state::<Launcher>().yandex_window_exists());
 if launched {
  if !settings.launch_on_play{return Err("Включите запуск плеера по Play в настройках".into());}
  progress(&app,true,"Запускаем плеер…");
  let handle=app.clone();let s=settings.clone();
  tauri::async_runtime::spawn_blocking(move||handle.state::<Launcher>().launch(&s,s.minimize_on_launch)).await.map_err(|e|e.to_string())??;
 }
 progress(&app,true,"Ожидаем воспроизведение…");
 let result=tokio::time::timeout(std::time::Duration::from_secs(40),async {
  let mut attempts=0; let mut last_play=None::<std::time::Instant>; let mut bootstrap_attempts=0; let started=std::time::Instant::now();
  loop {
   if *media.target.read().map_err(|_|"Состояние недоступно")?!=settings.player_id {return Err("Выбран другой плеер".into());}
   let state=media.snapshot.read().map_err(|_|"Состояние недоступно")?.clone();
   if state.source_id==settings.player_id&&state.connected {
    if state.status=="Playing"{if launched{app.state::<Launcher>().finish_minimized(&settings);}return Ok(());}
    if should_retry_play(attempts,last_play.map(|t|t.elapsed().as_millis() as u64),state.controls.play) {
     attempts+=1;last_play=Some(std::time::Instant::now());
     if let Err(error)=media.command(PlaybackAction::Play).await {app.state::<super::diagnostics::DiagnosticsService>().error("PlayerStartupPlay",&error);}
    }
   }
   if state.status!="Playing" && settings.bootstrap_yandex && settings.player_id==TARGET_ID && settings.launch_app_id==TARGET_ID && bootstrap_attempts<1 && started.elapsed().as_secs()>=(5+bootstrap_attempts*4) { bootstrap_attempts+=1; progress(&app,true,"Включаем музыку в плеере…"); let token=active.clone();let snapshot=media.snapshot.clone();if let Ok(Err(error))=tauri::async_runtime::spawn_blocking(move||super::bootstrap::first_play(token,snapshot)).await {app.state::<super::diagnostics::DiagnosticsService>().error("PlayerStartup",&error);} }
   let _=media.tx.try_send(super::media_session::Message::Refresh(false));
   tokio::time::sleep(std::time::Duration::from_millis(400)).await;
  }
 }).await;
 result.unwrap_or_else(|_|Err("Плеер открыт, но не начал воспроизведение. Один раз выберите в нём трек и проверьте вход в аккаунт. Если выбран другой плеер, укажите его медиасессию в настройках".into()))
}


fn should_retry_play(attempts:u32,since_ms:Option<u64>,enabled:bool)->bool {enabled&&attempts<3&&since_ms.map(|ms|ms>=2000).unwrap_or(true)}
#[cfg(test)]mod startup_tests{use super::*;#[test]fn retries_play_until_confirmed_but_does_not_spam(){assert!(should_retry_play(0,None,true));assert!(!should_retry_play(1,Some(100),true));assert!(should_retry_play(1,Some(2000),true));assert!(!should_retry_play(3,Some(9000),true));assert!(!should_retry_play(0,None,false));}}

pub async fn start_wave(app:tauri::AppHandle,media:MediaSessionService,settings:Settings)->Result<(),String>{
 if settings.player_id!=TARGET_ID||settings.launch_app_id!=TARGET_ID{return Err("«Моя волна» доступна только для Яндекс Музыки".into());}
 let busy=app.state::<Launcher>().busy.clone();if busy.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){return Err("Плеер уже запускается".into());}
 let _guard=LaunchGuard{busy,active:Arc::new(AtomicBool::new(true)),app:app.clone()};
 let connected=media.snapshot.read().map_err(|_|"Состояние недоступно")?.connected&&app.state::<Launcher>().yandex_window_exists();
 if !connected {
  if !settings.launch_on_play{return Err("Включите запуск плеера по Play в настройках".into());}
  progress(&app,true,"Запускаем Яндекс Музыку…");let a=app.clone();let s=settings.clone();tauri::async_runtime::spawn_blocking(move||a.state::<Launcher>().launch(&s,s.minimize_on_launch)).await.map_err(|e|e.to_string())??;
 }
 progress(&app,true,"Включаем Мою волну…");
 let until=std::time::Instant::now()+std::time::Duration::from_secs(30);let mut sent=false;let mut last_error=String::new();
 loop{
  if !sent {match tauri::async_runtime::spawn_blocking(super::yandex_controls::start_wave).await.map_err(|e|e.to_string())? {Ok(())=>sent=true,Err(e)=>last_error=e}}
  let playing=media.snapshot.read().map(|s|s.source_id==TARGET_ID&&s.status=="Playing").unwrap_or(false);
  if sent&&playing{if !connected{app.state::<Launcher>().finish_minimized(&settings);}return Ok(());}
  if std::time::Instant::now()>=until{return Err(if last_error.is_empty(){"Яндекс Музыка ещё не начала Мою волну. Проверьте подключение к интернету".into()}else{last_error});}
  let _=media.tx.try_send(super::media_session::Message::Refresh(true));tokio::time::sleep(std::time::Duration::from_millis(500)).await;
 }
}
