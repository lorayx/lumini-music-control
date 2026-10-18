//! Process-scoped WASAPI loopback. Only scalar envelopes leave this worker;
//! PCM buffers are released immediately, never recorded or sent to the WebView.
use std::sync::{Arc,Mutex,RwLock,atomic::{AtomicBool,Ordering},mpsc};
use std::time::{Duration,Instant};
use tauri::{Emitter,Manager};
use serde::Serialize;
use windows::{core::{implement,Interface,HRESULT,w},Win32::{Foundation::{HWND,LPARAM,HANDLE,CloseHandle},System::{Com::{IAgileObject,IAgileObject_Impl,CoInitializeEx,CoUninitialize,COINIT_MULTITHREADED,BLOB,StructuredStorage::*},Variant::VT_BLOB,Threading::{CreateEventW,WaitForSingleObject}},Media::Audio::*,UI::WindowsAndMessaging::{EnumWindows,GetWindowThreadProcessId,GetWindowTextW,IsWindowVisible}}};
use crate::model::{Settings,TARGET_ID,now_ms};
use super::{media_session::MediaSessionService,audio_analysis::KickDetector};

#[derive(Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct AudioLevel {pub active:bool,pub pid:u32,pub kick:f32,pub bass:f32,pub sub:f32,pub mid:f32,pub treble:f32,pub packets:u64,pub hits:u64,pub timestamp:u64,pub error:Option<String>}
pub struct AudioService {pub enabled:Arc<AtomicBool>,pub stop:Arc<AtomicBool>,pub latest:Arc<RwLock<AudioLevel>>}
impl AudioService {
 pub fn configure(&self,s:&Settings){self.enabled.store(s.visualizer&&s.kick_reactive&&!s.reduce_motion,Ordering::Release);}
 pub fn start(app:&tauri::AppHandle,settings:&Settings)->Self{
  let service=Self{enabled:Arc::new(AtomicBool::new(false)),stop:Arc::new(AtomicBool::new(false)),latest:Arc::new(RwLock::new(AudioLevel::default()))};service.configure(settings);
  let enabled=service.enabled.clone();let stop=service.stop.clone();let latest=service.latest.clone();let app=app.clone();
  std::thread::spawn(move||unsafe{
   if CoInitializeEx(None,COINIT_MULTITHREADED).is_err(){return;}
   let media=app.state::<MediaSessionService>().inner().clone();
   while !stop.load(Ordering::Acquire){
    let wanted=enabled.load(Ordering::Acquire)&&media.snapshot.read().map(|s|s.connected&&s.status=="Playing"&&s.source_id==TARGET_ID).unwrap_or(false);
    if !wanted{if latest.read().map(|s|s.active||s.error.is_some()).unwrap_or(false){publish(&app,&latest,AudioLevel::default());}std::thread::sleep(Duration::from_millis(150));continue;}
    let Some(pid)=player_pid() else{std::thread::sleep(Duration::from_millis(500));continue;};
    if let Err(error)=capture(&app,&media,pid,&enabled,&stop,&latest){publish(&app,&latest,AudioLevel{error:Some(error),timestamp:now_ms(),..Default::default()});for _ in 0..50{if stop.load(Ordering::Acquire)||!enabled.load(Ordering::Acquire){break;}std::thread::sleep(Duration::from_millis(100));}}
   }
   CoUninitialize();
  });service
 }
}
fn publish(app:&tauri::AppHandle,latest:&RwLock<AudioLevel>,value:AudioLevel){if let Ok(mut state)=latest.write(){*state=value.clone();}let _=app.emit("audio-reactivity",value);}
struct WindowSearch{path:std::path::PathBuf,pid:u32}
unsafe extern "system" fn find_player(window:HWND,data:LPARAM)->windows::core::BOOL{
 let search=&mut *(data.0 as *mut WindowSearch);let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));
 if IsWindowVisible(window).as_bool()&&super::launcher::process_path(pid).as_ref()==Some(&search.path){let mut title=[0u16;256];let length=GetWindowTextW(window,&mut title);if length>0&&String::from_utf16_lossy(&title[..length as usize]).starts_with("Яндекс Музыка"){search.pid=pid;return false.into();}}
 true.into()
}
fn player_pid()->Option<u32>{let path=std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Programs/YandexMusic/Яндекс Музыка.exe");let mut search=WindowSearch{path,pid:0};unsafe{let _=EnumWindows(Some(find_player),LPARAM((&mut search as *mut WindowSearch) as isize));}if search.pid==0{None}else{Some(search.pid)}}

struct Activated(usize);
impl Activated{unsafe fn into_client(mut self)->IAudioClient{let ptr=std::mem::replace(&mut self.0,0);IAudioClient::from_raw(ptr as *mut std::ffi::c_void)}}
impl Drop for Activated{fn drop(&mut self){if self.0!=0{unsafe{drop(IAudioClient::from_raw(self.0 as *mut std::ffi::c_void));}}}}
#[implement(IActivateAudioInterfaceCompletionHandler,IAgileObject)]
struct Activation {reply:Mutex<Option<mpsc::SyncSender<Result<Activated,String>>>>,_params:Box<AUDIOCLIENT_ACTIVATION_PARAMS>}
impl IAgileObject_Impl for Activation_Impl{}
impl IActivateAudioInterfaceCompletionHandler_Impl for Activation_Impl{
 fn ActivateCompleted(&self,operation:windows::core::Ref<IActivateAudioInterfaceAsyncOperation>)->windows::core::Result<()>{
  let result=(||unsafe{let mut status=HRESULT(0);let mut unknown=None;operation.ok()?.GetActivateResult(&mut status,&mut unknown)?;status.ok()?;unknown.ok_or_else(||windows::core::Error::from_hresult(HRESULT(0x80004005u32 as i32)))?.cast::<IAudioClient>()})().map_err(|e|e.to_string());
  if let Ok(mut sender)=self.reply.lock(){if let Some(sender)=sender.take(){let _=sender.send(result.map(|client|Activated(client.into_raw() as usize)));}}
  Ok(())
 }
}
struct Capture{client:IAudioClient,reader:IAudioCaptureClient,event:HANDLE}
impl Drop for Capture{fn drop(&mut self){unsafe{let _=self.client.Stop();let _=CloseHandle(self.event);}}}
unsafe fn activate(pid:u32)->Result<Capture,String>{
 let params=Box::new(AUDIOCLIENT_ACTIVATION_PARAMS{ActivationType:AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,Anonymous:AUDIOCLIENT_ACTIVATION_PARAMS_0{ProcessLoopbackParams:AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS{TargetProcessId:pid,ProcessLoopbackMode:PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE}}});
 let ptr=(&*params as *const AUDIOCLIENT_ACTIVATION_PARAMS) as *mut u8;
 // BLOB borrows params owned by the completion handler, not COM-allocated memory.
 let mut prop=std::mem::ManuallyDrop::new(PROPVARIANT::default());prop.Anonymous.Anonymous=std::mem::ManuallyDrop::new(PROPVARIANT_0_0{vt:VT_BLOB,Anonymous:PROPVARIANT_0_0_0{blob:BLOB{cbSize:std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,pBlobData:ptr}},..Default::default()});
 let (send,receive)=mpsc::sync_channel(1);let handler:IActivateAudioInterfaceCompletionHandler=Activation{reply:Mutex::new(Some(send)),_params:params}.into();
 let _operation=ActivateAudioInterfaceAsync(w!("VAD\\Process_Loopback"),&IAudioClient::IID,Some(&*prop),&handler).map_err(|e|e.to_string())?;
 // Both the completion callback and this worker belong to the COM MTA.
 let client=(receive.recv_timeout(Duration::from_secs(5)).map_err(|_|"Windows не включила анализ звука за 5 секунд")??).into_client();
 let format=WAVEFORMATEX{wFormatTag:1,nChannels:2,nSamplesPerSec:44100,nAvgBytesPerSec:176400,nBlockAlign:4,wBitsPerSample:16,cbSize:0};
 client.Initialize(AUDCLNT_SHAREMODE_SHARED,AUDCLNT_STREAMFLAGS_LOOPBACK|AUDCLNT_STREAMFLAGS_EVENTCALLBACK|AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,0,0,&format,None).map_err(|e|e.to_string())?;
 let reader:IAudioCaptureClient=client.GetService().map_err(|e|e.to_string())?;
 let event=CreateEventW(None,false,false,None).map_err(|e|e.to_string())?;
 let capture=Capture{client,reader,event};capture.client.SetEventHandle(event).map_err(|e|e.to_string())?;capture.client.Start().map_err(|e|e.to_string())?;Ok(capture)
}
unsafe fn capture(app:&tauri::AppHandle,media:&MediaSessionService,pid:u32,enabled:&AtomicBool,stop:&AtomicBool,latest:&RwLock<AudioLevel>)->Result<(),String>{
 let stream=activate(pid)?;let mut detector=KickDetector::default();let mut track=media.snapshot.read().map(|m|(m.title.clone(),m.artist.clone(),m.album.clone())).unwrap_or_default();let mut packets=0;let mut published_hits=0;let mut last_emit=Instant::now()-Duration::from_secs(1);let mut last_pid_check=Instant::now();
 while enabled.load(Ordering::Acquire)&&!stop.load(Ordering::Acquire){
  if !media.snapshot.read().map(|s|s.connected&&s.status=="Playing"&&s.source_id==TARGET_ID).unwrap_or(false){break;}
  if last_pid_check.elapsed()>Duration::from_secs(1){if player_pid()!=Some(pid){break;}last_pid_check=Instant::now();}
  let changed=media.snapshot.read().map(|m|m.title!=track.0||m.artist!=track.1||m.album!=track.2).unwrap_or(false);
  if changed { if let Ok(m)=media.snapshot.read(){track=(m.title.clone(),m.artist.clone(),m.album.clone());}detector=KickDetector::default();published_hits=0; }
  let before_packets=packets;let waited=Instant::now();
  let _=WaitForSingleObject(stream.event,100);
  while stream.reader.GetNextPacketSize().map_err(|e|e.to_string())?>0{
   let mut data=std::ptr::null_mut();let mut frames=0;let mut flags=0;stream.reader.GetBuffer(&mut data,&mut frames,&mut flags,None,None).map_err(|e|e.to_string())?;
   if frames>0&&frames<=441000{
    if flags&AUDCLNT_BUFFERFLAGS_SILENT.0 as u32!=0||data.is_null(){for _ in 0..frames{detector.sample(0.0);}}
    else {for frame in 0..frames as usize{let left=std::ptr::read_unaligned(data.add(frame*4).cast::<i16>()) as f32/32768.0;let right=std::ptr::read_unaligned(data.add(frame*4+2).cast::<i16>()) as f32/32768.0;detector.sample_stereo(left,right);}}
   }
   stream.reader.ReleaseBuffer(frames).map_err(|e|e.to_string())?;packets+=1;
  }
  if packets==before_packets{detector.idle(waited.elapsed().as_secs_f32());}
  // Fresh attacks bypass the periodic envelope refresh; do not queue a beat.
  if detector.hits!=published_hits||last_emit.elapsed()>=Duration::from_millis(16){published_hits=detector.hits;publish(app,latest,AudioLevel{active:true,pid,kick:detector.pulse,bass:detector.level,sub:detector.sub,mid:detector.mid,treble:detector.treble,packets,hits:detector.hits,timestamp:now_ms(),error:None});last_emit=Instant::now();}
 }
 publish(app,latest,AudioLevel::default());Ok(())
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]#[ignore="Manual diagnostic: requires the selected Yandex Music to be playing"]
 fn live_process_capture(){unsafe{
  CoInitializeEx(None,COINIT_MULTITHREADED).ok().unwrap();
  let result=(||->Result<(),String>{
   let pid=player_pid().ok_or("Нет окна Яндекс Музыки")?;let stream=activate(pid)?;let mut d=KickDetector::default();let mut packets=0;let mut frames_total=0u64;let mut peak=0.0f32;
   let start=Instant::now();while start.elapsed()<Duration::from_secs(8){let _=WaitForSingleObject(stream.event,100);while stream.reader.GetNextPacketSize().map_err(|e|e.to_string())?>0{let mut data=std::ptr::null_mut();let mut frames=0;let mut flags=0;stream.reader.GetBuffer(&mut data,&mut frames,&mut flags,None,None).map_err(|e|e.to_string())?;if !data.is_null()&&flags&AUDCLNT_BUFFERFLAGS_SILENT.0 as u32==0{for frame in 0..frames as usize{let l=std::ptr::read_unaligned(data.add(frame*4).cast::<i16>()) as f32/32768.0;let r=std::ptr::read_unaligned(data.add(frame*4+2).cast::<i16>()) as f32/32768.0;d.sample_stereo(l,r);peak=peak.max(d.pulse);}}stream.reader.ReleaseBuffer(frames).map_err(|e|e.to_string())?;packets+=1;frames_total+=frames as u64;}}
   eprintln!("LIVE_AUDIO_PROBE pid={pid} packets={packets} frames={frames_total} hits={} peak={peak}",d.hits);
   if frames_total==0||peak<=0.0{return Err("Поток не содержит подтверждённых низкочастотных атак".into());}Ok(())
  })();CoUninitialize();result.unwrap();
 }}

}
