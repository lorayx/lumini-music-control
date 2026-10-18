//! Optional first Play through Windows accessibility, scoped to the confirmed
//! Yandex executable and its player region. No focus, coordinates or keyboard.
use windows::{core::{Interface,BSTR},Win32::{Foundation::{HWND,LPARAM},System::{Com::{CoCreateInstance,CoInitializeEx,CoUninitialize,CLSCTX_INPROC_SERVER,COINIT_MULTITHREADED},Variant::VARIANT},UI::{Accessibility::*,WindowsAndMessaging::{EnumWindows,GetWindowThreadProcessId,IsWindowVisible,GetWindowTextW,IsIconic,ShowWindowAsync,SW_SHOWNOACTIVATE,SW_SHOWMINNOACTIVE}}}};
use super::launcher::process_path;
struct Candidate {path:std::path::PathBuf,windows:Vec<HWND>}
struct RestoreMinimized {window:HWND,restore:bool}
impl Drop for RestoreMinimized {fn drop(&mut self){if self.restore{unsafe{let _=ShowWindowAsync(self.window,SW_SHOWMINNOACTIVE);}}}}
unsafe extern "system" fn collect(window:HWND,data:LPARAM)->windows::core::BOOL{
 let state=&mut *(data.0 as *mut Candidate);let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));
 if IsWindowVisible(window).as_bool()&&process_path(pid).as_ref()==Some(&state.path){let mut text=[0u16;256];if GetWindowTextW(window,&mut text)>0{state.windows.push(window);}}
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
   if snapshot.read().map(|s|s.connected&&s.status=="Playing").unwrap_or(false){return Ok(());}
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
   // Chromium may withhold its accessibility tree while minimized. Briefly
   // expose the chosen window without activating it, then restore minimization.
   let minimized=IsIconic(window).as_bool();let _restore=RestoreMinimized{window,restore:minimized};
   if minimized {let _=ShowWindowAsync(window,SW_SHOWNOACTIVATE);std::thread::sleep(std::time::Duration::from_millis(400));}
   let deadline=std::time::Instant::now()+std::time::Duration::from_secs(12);
   // Keep one exposure for the entire attempt. Do not minimize/restore on
   // every accessibility retry while Chromium is still loading its player.
   loop {
    if !active.load(std::sync::atomic::Ordering::Acquire){return Err("Запуск отменён".into());}
    if snapshot.read().map(|s|s.connected&&s.status=="Playing").unwrap_or(false){return Ok(());}
    let found=(||->windows::core::Result<bool>{
     let root=automation.ElementFromHandle(window)?;
     let player=root.FindFirst(TreeScope_Descendants,&region)?;
     let button=player.FindFirst(TreeScope_Descendants,&condition)?;
     if button.CurrentName()?!=BSTR::from("Воспроизведение")||!button.CurrentIsEnabled()?.as_bool(){return Ok(false);}
     let invoke:IUIAutomationInvokePattern=button.GetCurrentPatternAs(UIA_InvokePatternId)?;
     if !active.load(std::sync::atomic::Ordering::Acquire){return Ok(false);}
     invoke.Invoke()?;Ok(true)
    })();
    if matches!(found,Ok(true)){return Ok(());}
    if std::time::Instant::now()>=deadline{break;}
    std::thread::sleep(std::time::Duration::from_millis(350));
   }
  }
  Err("В плеере пока недоступна кнопка первого воспроизведения".into())
 })();
 CoUninitialize();result
}}
