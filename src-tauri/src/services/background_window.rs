//! Make Chromium accessible briefly without activating or visibly exposing it.
//! Launch uses force-renderer-accessibility; window state is restored on all exits.
use windows::Win32::{Foundation::{HWND,COLORREF},UI::WindowsAndMessaging::*};
pub struct BackgroundWindow {window:HWND,minimized:bool,hidden:bool,layer_added:bool}
impl BackgroundWindow {
 pub unsafe fn expose(window:HWND)->Result<Self,String>{
  let minimized=IsIconic(window).as_bool();let hidden=!IsWindowVisible(window).as_bool();
  let mut guard=Self{window,minimized,hidden,layer_added:false};
  if minimized||hidden {
   let style=GetWindowLongPtrW(window,GWL_EXSTYLE);
   if style as u32 & WS_EX_LAYERED.0!=0{return Err("Окно плеера использует собственную прозрачность. Откройте его один раз".into());}
   SetWindowLongPtrW(window,GWL_EXSTYLE,style|WS_EX_LAYERED.0 as isize);guard.layer_added=true;
   SetLayeredWindowAttributes(window,COLORREF(0),1,LWA_ALPHA).map_err(|e|e.to_string())?;
   let _=ShowWindowAsync(window,SW_SHOWNOACTIVATE);std::thread::sleep(std::time::Duration::from_millis(600));
  }
  Ok(guard)
 }
}
impl Drop for BackgroundWindow {fn drop(&mut self){unsafe{
 if !IsWindow(Some(self.window)).as_bool(){return;}
 if self.hidden {let _=ShowWindowAsync(self.window,SW_HIDE);for _ in 0..40{if !IsWindowVisible(self.window).as_bool(){break;}std::thread::sleep(std::time::Duration::from_millis(10));}}
 else if self.minimized {let _=ShowWindowAsync(self.window,SW_SHOWMINNOACTIVE);for _ in 0..40{if IsIconic(self.window).as_bool(){break;}std::thread::sleep(std::time::Duration::from_millis(10));}}
 if self.layer_added{let _=SetLayeredWindowAttributes(self.window,COLORREF(0),255,LWA_ALPHA);let style=GetWindowLongPtrW(self.window,GWL_EXSTYLE);SetWindowLongPtrW(self.window,GWL_EXSTYLE,style & !(WS_EX_LAYERED.0 as isize));}
}}}
