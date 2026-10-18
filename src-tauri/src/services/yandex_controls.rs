//! Yandex actions through its accessibility surface. No account tokens or injection.
use serde::{Serialize,Deserialize};
use windows::{core::Interface,Win32::{Foundation::{HWND,LPARAM},System::{Com::{CoCreateInstance,CoInitializeEx,CoUninitialize,CLSCTX_INPROC_SERVER,COINIT_MULTITHREADED},Variant::VARIANT},UI::{Accessibility::*,WindowsAndMessaging::{EnumWindows,GetWindowThreadProcessId,IsWindowVisible,GetWindowTextW,IsIconic,ShowWindowAsync,SW_SHOWNOACTIVATE,SW_SHOWMINNOACTIVE,SW_SHOWNORMAL}}}};
use crate::model::{MediaState,TARGET_ID};
#[derive(Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct YandexFeatures {pub title:String,pub artist:String,pub like_available:bool,pub liked:Option<bool>,pub lyrics_available:bool,pub aria:String}
#[derive(Clone,Deserialize)]
#[serde(rename_all="camelCase")]
pub enum Action{Like,Lyrics}
struct Candidate{path:std::path::PathBuf,window:Option<HWND>}
unsafe extern "system" fn collect(window:HWND,data:LPARAM)->windows::core::BOOL{
 let c=&mut *(data.0 as *mut Candidate);let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));
 if IsWindowVisible(window).as_bool()&&super::launcher::process_path(pid).as_ref()==Some(&c.path){let mut title=[0u16;256];let len=GetWindowTextW(window,&mut title);if String::from_utf16_lossy(&title[..len.max(0) as usize]).starts_with("Яндекс Музыка"){c.window=Some(window);return false.into();}}true.into()
}
struct Com;
impl Com{fn new()->Result<Self,String>{unsafe{CoInitializeEx(None,COINIT_MULTITHREADED).ok().map_err(|e|e.to_string())?;}Ok(Self)}}
impl Drop for Com{fn drop(&mut self){unsafe{CoUninitialize();}}}
struct Exposure{window:HWND,minimized:bool}
impl Drop for Exposure{fn drop(&mut self){if self.minimized{unsafe{let _=ShowWindowAsync(self.window,SW_SHOWMINNOACTIVE);}}}}
struct Ui{automation:IUIAutomation,root:IUIAutomationElement,window:HWND}
impl Ui{
 unsafe fn connect()->Result<Self,String>{
  let path=std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("Нет папки приложений")?).join("Programs/YandexMusic/Яндекс Музыка.exe");let mut c=Candidate{path,window:None};let _=EnumWindows(Some(collect),LPARAM((&mut c as *mut Candidate) as isize));let window=c.window.ok_or("Откройте Яндекс Музыку")?;
  let automation:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER).map_err(err)?;if let Ok(a)=automation.cast::<IUIAutomation2>(){let _=a.SetConnectionTimeout(800);let _=a.SetTransactionTimeout(800);}
  let root=automation.ElementFromHandle(window).map_err(err)?;Ok(Self{automation,root,window})
 }
 unsafe fn find(&self,parent:&IUIAutomationElement,name:&str,kind:Option<UIA_CONTROLTYPE_ID>,scope:TreeScope)->Result<IUIAutomationElement,String>{
  let n=self.automation.CreatePropertyCondition(UIA_NamePropertyId,&VARIANT::from(name)).map_err(err)?;
  let c=if let Some(kind)=kind{let t=self.automation.CreatePropertyCondition(UIA_ControlTypePropertyId,&VARIANT::from(kind.0)).map_err(err)?;self.automation.CreateAndCondition(&n,&t).map_err(err)?}else{n};parent.FindFirst(scope,&c).map_err(err)
 }
 unsafe fn named(&self,parent:&IUIAutomationElement,name:&str)->Result<IUIAutomationElement,String>{self.find(parent,name,None,TreeScope_Descendants)}
 unsafe fn button(&self,parent:&IUIAutomationElement,name:&str)->Result<IUIAutomationElement,String>{self.find(parent,name,Some(UIA_ButtonControlTypeId),TreeScope_Descendants)}
 unsafe fn invoke(&self,el:&IUIAutomationElement)->Result<(),String>{if !el.CurrentIsEnabled().map_err(err)?.as_bool(){return Err("Команда пока недоступна в Яндекс Музыке".into());}el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId).map_err(err)?.Invoke().map_err(err)}
 unsafe fn player(&self,state:&MediaState)->Result<IUIAutomationElement,String>{
  let p=self.named(&self.root,"Плеер")?;
  if state.title.trim().is_empty(){return Err("Нет активного трека".into());}
  // The real player must still contain the requested title. Never like a new
  // track after the metadata used for the click has changed.
  self.named(&p,&state.title).map_err(|_|"Трек уже сменился. Повторите действие".to_string())?;Ok(p)
 }
 unsafe fn read(&self,state:&MediaState)->Result<YandexFeatures,String>{
  let player=self.player(state)?;let like=self.button(&player,"Нравится").ok();
  let aria=like.as_ref().and_then(|b|b.CurrentAriaProperties().ok()).map(|s|s.to_string()).unwrap_or_default();
  let liked=like.as_ref().and_then(|b|b.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId).ok()).and_then(|p|p.CurrentToggleState().ok()).and_then(|s|if s==ToggleState_On{Some(true)}else if s==ToggleState_Off{Some(false)}else{None}).or_else(||pressed(&aria));
  let lyrics_available=self.lyrics_button(&player).ok().is_some_and(|b|b.CurrentIsEnabled().map(|s|s.as_bool()).unwrap_or(false));
  Ok(YandexFeatures{title:state.title.clone(),artist:state.artist.clone(),like_available:like.as_ref().is_some_and(|b|b.CurrentIsEnabled().map(|s|s.as_bool()).unwrap_or(false)),liked,lyrics_available,aria})
 }
 unsafe fn wait_player(&self,state:&MediaState)->Result<(),String>{let end=std::time::Instant::now()+std::time::Duration::from_secs(5);loop{if self.player(state).is_ok(){return Ok(());}if std::time::Instant::now()>=end{return Err("Плеер ещё не предоставил управление этим треком".into());}std::thread::sleep(std::time::Duration::from_millis(120));}}
 unsafe fn lyrics_button(&self,player:&IUIAutomationElement)->Result<IUIAutomationElement,String>{for name in ["Показать текст песни","Включить текстомузыку","Текст песни"]{if let Ok(button)=self.button(player,name){return Ok(button);}}Err("Текст этого трека недоступен".into())}
 unsafe fn perform(&self,action:Action,state:&MediaState,desired:Option<bool>)->Result<(),String>{match action{
  Action::Like=>{self.wait_player(state)?;let wanted=desired.ok_or("Не задано состояние избранного")?;let current=self.read(state)?.liked.ok_or("Яндекс Музыка не сообщила состояние избранного")?;if current==wanted{return Ok(());}let p=self.player(state)?;let button=self.button(&p,"Нравится")?;button.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId).map_err(err)?.Toggle().map_err(err)?;let end=std::time::Instant::now()+std::time::Duration::from_secs(2);loop{if self.read(state)?.liked==Some(wanted){return Ok(());}if std::time::Instant::now()>=end{return Err("Яндекс Музыка ещё не подтвердила изменение избранного".into());}std::thread::sleep(std::time::Duration::from_millis(100));}},
  Action::Lyrics=>{self.wait_player(state)?;let p=self.player(state)?;self.invoke(&self.lyrics_button(&p)?)},

 }}
}
fn err(e:windows::core::Error)->String{format!("Яндекс Музыка: {e}")}
fn pressed(value:&str)->Option<bool>{for pair in value.split(';'){let mut parts=pair.trim().splitn(2,'=');if matches!(parts.next(),Some("pressed")|Some("checked")){return match parts.next(){Some("true")=>Some(true),Some("false")=>Some(false),_=>None};}}None}
static CACHE:std::sync::OnceLock<std::sync::Mutex<Option<(YandexFeatures,std::time::Instant)>>> =std::sync::OnceLock::new();
static ACTION_BUSY:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
struct ActionGuard;
impl Drop for ActionGuard{fn drop(&mut self){ACTION_BUSY.store(false,std::sync::atomic::Ordering::Release);}}
fn remember(value:YandexFeatures)->YandexFeatures{if value.liked.is_some(){if let Ok(mut cache)=CACHE.get_or_init(Default::default).lock(){*cache=Some((value.clone(),std::time::Instant::now()));}}value}
fn fallback(state:&MediaState)->YandexFeatures{
 if let Ok(cache)=CACHE.get_or_init(Default::default).lock(){if let Some((value,time))=cache.as_ref(){if value.title==state.title&&value.artist==state.artist&&time.elapsed().as_secs()<300{return value.clone();}}}
 // Unknown heart is an add-only action until Yandex confirms its real state.
 YandexFeatures{title:state.title.clone(),artist:state.artist.clone(),like_available:!state.title.trim().is_empty(),..Default::default()}
}
pub fn inspect(state:MediaState)->YandexFeatures{
 if state.source_id!=TARGET_ID||!state.connected{return YandexFeatures::default();}
 if ACTION_BUSY.load(std::sync::atomic::Ordering::Acquire){return fallback(&state);}
 let Ok(_com)=Com::new()else{return fallback(&state);};unsafe{Ui::connect().and_then(|ui|ui.read(&state)).map(remember).unwrap_or_else(|_|fallback(&state))}
}
pub fn perform(action:Action,state:MediaState,desired:Option<bool>)->Result<YandexFeatures,String>{
 if state.source_id!=TARGET_ID||!state.connected{return Err("Подключите Яндекс Музыку и включите трек".into());}
 if ACTION_BUSY.compare_exchange(false,true,std::sync::atomic::Ordering::AcqRel,std::sync::atomic::Ordering::Acquire).is_err(){return Err("Дождитесь завершения предыдущей команды".into());}let _guard=ActionGuard;
 let _com=Com::new()?;unsafe{
  let ui=Ui::connect()?;let minimized=IsIconic(ui.window).as_bool();let lyrics=matches!(action,Action::Lyrics);let _restore=Exposure{window:ui.window,minimized:minimized&&!lyrics};if minimized{let _=ShowWindowAsync(ui.window,if lyrics{SW_SHOWNORMAL}else{SW_SHOWNOACTIVATE});std::thread::sleep(std::time::Duration::from_millis(300));}
  ui.perform(action,&state,desired)?;
  Ok(ui.read(&state).map(remember).unwrap_or_default())
 }
}
#[cfg(test)]mod tests{use super::*;#[test]fn reads_explicit_pressed_state(){assert_eq!(pressed("pressed=true;readonly=false"),Some(true));assert_eq!(pressed("checked=false"),Some(false));assert_eq!(pressed(""),None);assert_eq!(pressed("pressed=mixed"),None);}}

#[cfg(test)]mod live_diagnostics{use super::*;#[test]#[ignore="Manual read-only Yandex accessibility diagnostic"]fn live_like_properties(){let _com=Com::new().unwrap();unsafe{let ui=Ui::connect().unwrap();let player=ui.named(&ui.root,"Плеер").unwrap();let button=ui.button(&player,"Нравится").unwrap();let aria=button.CurrentAriaProperties().unwrap().to_string();let toggle=button.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId).ok().and_then(|p|p.CurrentToggleState().ok()).map(|s|s.0);eprintln!("LIKE_PROPERTIES aria={aria:?} toggle={toggle:?}");}}
}

#[cfg(test)]mod cache_tests{use super::*;#[test]fn confirmation_never_leaks_to_another_track(){let state=MediaState{title:"Confirmed song".into(),artist:"Artist".into(),..Default::default()};remember(YandexFeatures{title:state.title.clone(),artist:state.artist.clone(),liked:Some(true),like_available:true,lyrics_available:true,..Default::default()});assert_eq!(fallback(&state).liked,Some(true));let next=MediaState{title:"Next song".into(),..state.clone()};assert_eq!(fallback(&next).liked,None);assert!(!fallback(&next).lyrics_available);let other_artist=MediaState{artist:"Other artist".into(),..state};assert_eq!(fallback(&other_artist).liked,None);}}
