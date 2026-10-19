//! Optional first Play through Windows accessibility, scoped to the confirmed
//! Yandex executable and its player region. No focus, coordinates or keyboard.
use windows::{core::{Interface,BSTR},Win32::{Foundation::{HWND,LPARAM},System::{Com::{CoCreateInstance,CoInitializeEx,CoUninitialize,CLSCTX_INPROC_SERVER,COINIT_MULTITHREADED},Variant::VARIANT},UI::{Accessibility::*,WindowsAndMessaging::{EnumWindows,GetWindowThreadProcessId,GetWindowTextW,}}}};
use super::launcher::process_path;
struct Candidate {path:std::path::PathBuf,windows:Vec<HWND>}
unsafe extern "system" fn collect(window:HWND,data:LPARAM)->windows::core::BOOL{
 let state=&mut *(data.0 as *mut Candidate);let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));
 if process_path(pid).as_ref()==Some(&state.path){let mut text=[0u16;256];let len=GetWindowTextW(window,&mut text);if len>0&&String::from_utf16_lossy(&text[..len as usize]).starts_with("Яндекс Музыка"){state.windows.push(window);}}
 true.into()
}
pub fn first_play(active:std::sync::Arc<std::sync::atomic::AtomicBool>,snapshot:std::sync::Arc<std::sync::RwLock<crate::model::MediaState>>)->Result<(),String>{unsafe{
 CoInitializeEx(None,COINIT_MULTITHREADED).ok().map_err(|e|e.to_string())?;
 let result=(||{
  let path=std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("Нет папки приложений")?).join("Programs/YandexMusic/Яндекс Музыка.exe");
  let mut candidates=Candidate{path,windows:Vec::new()};
  let discovery=std::time::Instant::now()+std::time::Duration::from_secs(6);
  loop{
   if !active.load(std::sync::atomic::Ordering::Acquire){return Err("Запуск отменён".into());}
   if snapshot.read().map(|s|s.source_id==crate::model::TARGET_ID&&s.connected&&s.status=="Playing").unwrap_or(false){return Ok(());}
   let _=EnumWindows(Some(collect),LPARAM((&mut candidates as *mut Candidate) as isize));
   if !candidates.windows.is_empty()||std::time::Instant::now()>=discovery{break;}
   std::thread::sleep(std::time::Duration::from_millis(250));
  }
  let automation:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER).map_err(|e|e.to_string())?;
  if let Ok(options)=automation.cast::<IUIAutomation2>(){let _=options.SetConnectionTimeout(1000);let _=options.SetTransactionTimeout(1000);}
  let region=automation.CreatePropertyCondition(UIA_NamePropertyId,&VARIANT::from("Плеер")).map_err(|e|e.to_string())?;
  let name=automation.CreatePropertyCondition(UIA_NamePropertyId,&VARIANT::from("Воспроизведение")).map_err(|e|e.to_string())?;
  let button_type=automation.CreatePropertyCondition(UIA_ControlTypePropertyId,&VARIANT::from(UIA_ButtonControlTypeId.0)).map_err(|e|e.to_string())?;
  let condition=automation.CreateAndCondition(&name,&button_type).map_err(|e|e.to_string())?;
  for window in candidates.windows.into_iter().take(1) {
   if !active.load(std::sync::atomic::Ordering::Acquire){return Err("Запуск отменён".into());}
   let _exposure=super::background_window::BackgroundWindow::expose(window)?;
   let mut last_invoke=None::<std::time::Instant>;
   let deadline=std::time::Instant::now()+std::time::Duration::from_secs(12);
   // Keep one exposure for the entire attempt. Do not minimize/restore on
   // every accessibility retry while Chromium is still loading its player.
   loop {
    if !active.load(std::sync::atomic::Ordering::Acquire){return Err("Запуск отменён".into());}
    if snapshot.read().map(|s|s.source_id==crate::model::TARGET_ID&&s.connected&&s.status=="Playing").unwrap_or(false){return Ok(());}
    let can_invoke=last_invoke.map(|t|t.elapsed().as_secs()>=3).unwrap_or(true);
    let found=(||->windows::core::Result<bool>{
     let root=automation.ElementFromHandle(window)?;
     let player=root.FindFirst(TreeScope_Descendants,&region)?;
     let button=player.FindFirst(TreeScope_Descendants,&condition)?;
     if button.CurrentName()?!=BSTR::from("Воспроизведение")||!button.CurrentIsEnabled()?.as_bool(){return Ok(false);}
     let invoke:IUIAutomationInvokePattern=button.GetCurrentPatternAs(UIA_InvokePatternId)?;
     if !active.load(std::sync::atomic::Ordering::Acquire){return Ok(false);}
     if can_invoke {invoke.Invoke()?;Ok(true)}else{Ok(false)}
    })();
    if matches!(found,Ok(true)){last_invoke=Some(std::time::Instant::now());}
    if std::time::Instant::now()>=deadline{break;}
    std::thread::sleep(std::time::Duration::from_millis(350));
   }
  }
  Err("В плеере пока недоступна кнопка первого воспроизведения".into())
 })();
 CoUninitialize();result
}}

