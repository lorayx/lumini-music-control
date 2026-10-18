use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize};
use windows::{core::w, Win32::{Foundation::{RECT,POINT}, Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST}, UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, GetForegroundWindow, GetClassNameW, GetClientRect, IsZoomed, IsWindowVisible, WindowFromPoint, GetAncestor, GA_ROOT, SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE}}};

static STRIP_HWND:AtomicIsize=AtomicIsize::new(0);
// Out-of-context accessibility notifications; no code is injected into Explorer.
unsafe extern "system" fn shell_order_changed(_:windows::Win32::UI::Accessibility::HWINEVENTHOOK,_event:u32,_hwnd:windows::Win32::Foundation::HWND,_object:i32,_child:i32,_thread:u32,_time:u32){
    let hwnd=windows::Win32::Foundation::HWND(STRIP_HWND.load(Ordering::Relaxed) as *mut std::ffi::c_void);
    restore_above_shell(hwnd);
}
fn restore_above_shell(hwnd:windows::Win32::Foundation::HWND){
    unsafe{
        if hwnd.is_invalid() || !IsWindowVisible(hwnd).as_bool(){return;}
        let mut rect=RECT::default();if GetWindowRect(hwnd,&mut rect).is_err(){return;}
        let covering=GetAncestor(WindowFromPoint(POINT{x:(rect.left+rect.right)/2,y:rect.bottom-10}),GA_ROOT);
        if FindWindowW(w!("Shell_TrayWnd"),None).ok()==Some(covering){
            let _=SetWindowPos(hwnd,Some(HWND_TOPMOST),0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE);
        }
    }
}
pub fn restore_after_dismiss(app:&tauri::AppHandle){
    if let Some(window)=app.get_webview_window("taskbar"){if let Ok(hwnd)=window.hwnd(){restore_above_shell(hwnd);}}
}
pub struct TaskbarState { pub enabled: AtomicBool, pub offset: AtomicI32, pub width: AtomicI32, pub stop: AtomicBool }
pub fn start(app: &tauri::AppHandle, enabled: bool, offset: i32) {
    app.manage(TaskbarState { enabled: AtomicBool::new(enabled), offset: AtomicI32::new(offset), width: AtomicI32::new(272), stop: AtomicBool::new(false) });
    let app=app.clone();
    std::thread::spawn(move || {
        use windows::Win32::UI::{Accessibility::{SetWinEventHook,UnhookWinEvent},WindowsAndMessaging::{EVENT_SYSTEM_FOREGROUND,EVENT_OBJECT_REORDER,WINEVENT_OUTOFCONTEXT,WINEVENT_SKIPOWNPROCESS,MsgWaitForMultipleObjectsEx,QS_ALLINPUT,MWMO_INPUTAVAILABLE,PeekMessageW,TranslateMessage,DispatchMessageW,MSG,PM_REMOVE}};
        if let Some(window)=app.get_webview_window("taskbar"){if let Ok(hwnd)=window.hwnd(){STRIP_HWND.store(hwnd.0 as isize,Ordering::Relaxed);}}
        let hooks=unsafe{[
            SetWinEventHook(EVENT_SYSTEM_FOREGROUND,EVENT_SYSTEM_FOREGROUND,None,Some(shell_order_changed),0,0,WINEVENT_OUTOFCONTEXT|WINEVENT_SKIPOWNPROCESS),
            SetWinEventHook(EVENT_OBJECT_REORDER,EVENT_OBJECT_REORDER,None,Some(shell_order_changed),0,0,WINEVENT_OUTOFCONTEXT|WINEVENT_SKIPOWNPROCESS)
        ]};
        let mut previous=None;
        let mut previous_theme=None;
        while !app.state::<TaskbarState>().stop.load(Ordering::Relaxed) {
            let light=light_theme();
            if previous_theme!=Some(light){let _=app.emit("taskbar-theme",light);previous_theme=Some(light);}
            let layout=layout(&app);
            if layout!=previous || (layout.is_some() && app.get_webview_window("taskbar").is_some_and(|w|!w.is_visible().unwrap_or(true))) {
                if let Some(window)=app.get_webview_window("taskbar") {
                    if let Some((x,y,width,height))=layout {
                        let _=window.set_size(PhysicalSize::new(width,height));
                        let _=window.set_position(PhysicalPosition::new(x,y));
                        let _=super::window::configure_label(&app,"taskbar");
                        let _=window.show();
                        let _=super::window::configure_label(&app,"taskbar");
                    } else { let _=window.hide(); }
                }
                previous=layout;
            }
            if layout.is_some(){restore_after_dismiss(&app);}
            // Sleep without polling, but dispatch shell notifications immediately.
            unsafe{
                let _=MsgWaitForMultipleObjectsEx(None,1000,QS_ALLINPUT,MWMO_INPUTAVAILABLE);
                let mut message=MSG::default();
                while PeekMessageW(&mut message,None,0,0,PM_REMOVE).as_bool(){let _=TranslateMessage(&message);DispatchMessageW(&message);}
            }
        }
        for hook in hooks{if !hook.is_invalid(){unsafe{let _=UnhookWinEvent(hook);}}}
        STRIP_HWND.store(0,Ordering::Relaxed);
    });
}
pub fn set_content_width(app:&tauri::AppHandle,width:u32)->Result<(),String>{
    if !(100..=600).contains(&width){return Err("Некорректная ширина полоски".into());}
    let state=app.state::<TaskbarState>();
    let width=width.clamp(180,320) as i32;
    if state.width.swap(width,Ordering::Relaxed)==width{return Ok(());}
    if let Some((x,y,width,height))=layout(app){
        if let Some(window)=app.get_webview_window("taskbar"){
            window.set_size(PhysicalSize::new(width,height)).map_err(|e|e.to_string())?;
            window.set_position(PhysicalPosition::new(x,y)).map_err(|e|e.to_string())?;
        }
    }
    Ok(())
}
fn layout(app: &tauri::AppHandle) -> Option<(i32,i32,u32,u32)> {
    let state=app.state::<TaskbarState>();
    if !state.enabled.load(Ordering::Relaxed) {return None;}
    let window=app.get_webview_window("taskbar")?;
    unsafe {
        let bar=FindWindowW(w!("Shell_TrayWnd"),None).ok()?;
        if !IsWindowVisible(bar).as_bool(){return None;}
        let mut rect=RECT::default(); GetWindowRect(bar,&mut rect).ok()?;
        let mut monitor=MONITORINFO {cbSize:std::mem::size_of::<MONITORINFO>() as u32,..Default::default()};
        if !GetMonitorInfoW(MonitorFromWindow(bar,MONITOR_DEFAULTTONEAREST),&mut monitor).as_bool(){return None;}
        let height=rect.bottom.min(monitor.rcMonitor.bottom)-rect.top.max(monitor.rcMonitor.top);
        if height<20 || rect.right-rect.left<600 || rect.bottom-rect.top>150 {return None;}
        let foreground=GetForegroundWindow();
        if !foreground.is_invalid() && foreground!=bar && foreground!=window.hwnd().ok()? {
            let mut name=[0u16;128]; let count=GetClassNameW(foreground,&mut name);
            let class=String::from_utf16_lossy(&name[..count.max(0) as usize]);
            let mut bounds=RECT::default();
            let mut origin=POINT::default();
            if !IsZoomed(foreground).as_bool() && class!="Progman" && class!="WorkerW" && GetClientRect(foreground,&mut bounds).is_ok() && ClientToScreen(foreground,&mut origin).as_bool()
                && origin.x<=monitor.rcMonitor.left && origin.y<=monitor.rcMonitor.top
                && origin.x+bounds.right>=monitor.rcMonitor.right && origin.y+bounds.bottom>=monitor.rcMonitor.bottom {return None;}
        }
        let scale=window.monitor_from_point(rect.left as f64,rect.top as f64).ok().flatten()?.scale_factor();
        let width=(state.width.load(Ordering::Relaxed) as f64*scale).round() as i32;
        let offset=(state.offset.load(Ordering::Relaxed) as f64*scale).round() as i32;
        // Keep the strip in the left half; never change/reserve Explorer's layout.
        let x=rect.left+offset.clamp(8,((rect.right-rect.left)/2-width-24).max(8));
        let h=(40.0*scale).round() as i32;
        Some((x,rect.top+(height-h)/2,width as u32,h as u32))
    }
}

pub fn light_theme()->bool {
    use windows::Win32::System::Registry::{RegGetValueW,HKEY_CURRENT_USER,RRF_RT_REG_DWORD};
    let mut value=0u32;let mut size=4u32;
    unsafe {RegGetValueW(HKEY_CURRENT_USER,w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),w!("SystemUsesLightTheme"),RRF_RT_REG_DWORD,None,Some((&mut value as *mut u32).cast()),Some(&mut size)).is_ok() && value!=0}
}
