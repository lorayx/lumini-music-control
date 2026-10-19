use std::sync::{atomic::{AtomicBool, Ordering}, Mutex};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize};
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos,
        GWL_EXSTYLE, WS_EX_TOOLWINDOW, WS_EX_APPWINDOW,
        SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_NOACTIVATE},
};

#[link(name="dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(hwnd: *mut std::ffi::c_void, attribute: u32, value: *const std::ffi::c_void, size: u32) -> i32;
}

#[derive(Default)]
pub struct WindowManager {
    expanded: AtomicBool,
    anchor: Mutex<Option<(f64, f64)>>,
    dismissed: Mutex<Option<Instant>>,
}

#[derive(Clone, Copy, Debug)]
struct Area { left: i32, top: i32, right: i32, bottom: i32 }

fn placement(area: Area, anchor: (f64,f64), size: (i32,i32), gap: i32) -> (i32,i32) {
    let (width,height)=size;
    let (ax,ay)=(anchor.0 as i32,anchor.1 as i32);
    let distances=[(ay-area.top).abs(),(area.bottom-ay).abs(),(ax-area.left).abs(),(area.right-ax).abs()];
    let edge=distances.iter().enumerate().min_by_key(|(_,d)| *d).map(|(i,_)| i).unwrap_or(1);
    let (x,y)=match edge {
        0=>(ax-width/2,area.top+gap),
        1=>(ax-width/2,area.bottom-height-gap),
        2=>(area.left+gap,ay-height/2),
        _=>(area.right-width-gap,ay-height/2),
    };
    (x.clamp(area.left+gap,(area.right-width-gap).max(area.left+gap)),
     y.clamp(area.top+gap,(area.bottom-height-gap).max(area.top+gap)))
}

/// Change only our HWND with documented styles. No Explorer hooks.
pub fn configure(app: &tauri::AppHandle) -> Result<(),String> {
    configure_label(app,"main")
}
pub fn configure_label(app: &tauri::AppHandle, label: &str) -> Result<(),String> {
    let window=app.get_webview_window(label).ok_or("Панель не найдена")?;
    let hwnd=window.hwnd().map_err(|e|e.to_string())?;
    unsafe {
        let extended=(GetWindowLongPtrW(hwnd,GWL_EXSTYLE) as u32|WS_EX_TOOLWINDOW.0)&!WS_EX_APPWINDOW.0;
        // Tao owns the borderless client calculation. Editing WS_CAPTION behind
        // its back caused stale native captions to paint through transparent UI.
        SetWindowLongPtrW(hwnd,GWL_EXSTYLE,extended as isize);
        let policy=1u32; // DWMWA_NCRENDERING_POLICY / DWMNCRP_DISABLED
        let result=DwmSetWindowAttribute(hwnd.0,2,(&policy as *const u32).cast(),4);
        if result<0 {return Err(format!("DWM non-client policy: {result:#x}"));}
        let border=0xfffffffeu32; // DWMWA_BORDER_COLOR / DWMWA_COLOR_NONE
        let _=DwmSetWindowAttribute(hwnd.0,34,(&border as *const u32).cast(),4);
        SetWindowPos(hwnd,None,0,0,0,0,SWP_FRAMECHANGED|SWP_NOMOVE|SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE).map_err(|e|e.to_string())?;
        let actual=GetWindowLongPtrW(hwnd,GWL_EXSTYLE) as u32;
        if actual&WS_EX_APPWINDOW.0!=0 || actual&WS_EX_TOOLWINDOW.0==0 {
            return Err("Не удалось включить режим служебной панели".into());
        }
    }
    Ok(())
}

fn anchor(app: &tauri::AppHandle, click: Option<(f64,f64)>) -> Result<(f64,f64),String> {
    let tray_point=app.tray_by_id("lumini").and_then(|tray|tray.rect().ok().flatten()).and_then(|rect|{
        let p=rect.position.to_physical::<f64>(1.0);
        let s=rect.size.to_physical::<f64>(1.0);
        if s.width>0.0&&s.height>0.0 {Some((p.x+s.width/2.0,p.y+s.height/2.0))} else {None}
    });
    let state=app.state::<WindowManager>();
    if let Some(point)=click.or(tray_point) {
        *state.anchor.lock().map_err(|_|"Ошибка положения панели")?=Some(point);
        return Ok(point);
    }
    let saved=*state.anchor.lock().map_err(|_|"Ошибка положения панели")?;
    if let Some(point)=saved{return Ok(point);}
    let window=app.get_webview_window("main").ok_or("Панель не найдена")?;
    let monitor=window.primary_monitor().map_err(|e|e.to_string())?.ok_or("Монитор не найден")?;
    Ok(((monitor.position().x+monitor.size().width as i32-24) as f64,
        (monitor.position().y+monitor.size().height as i32-8) as f64))
}

pub fn show(app: &tauri::AppHandle, click: Option<(f64,f64)>) -> Result<(),String> {
    let window=app.get_webview_window("main").ok_or("Панель не найдена")?;
    let point=anchor(app,click)?;
    let monitor=window.monitor_from_point(point.0,point.1).map_err(|e|e.to_string())?
        .or(window.primary_monitor().map_err(|e|e.to_string())?).ok_or("Монитор не найден")?;
    let scale=monitor.scale_factor();
    let mut info=MONITORINFO{cbSize:std::mem::size_of::<MONITORINFO>() as u32,..Default::default()};
    unsafe{let monitor=MonitorFromPoint(POINT{x:point.0 as i32,y:point.1 as i32},MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor,&mut info).as_bool(){return Err("Рабочая область монитора недоступна".into());}}
    let area=Area{left:info.rcWork.left,top:info.rcWork.top,right:info.rcWork.right,bottom:info.rcWork.bottom};
    let gap=(8.0*scale).round() as i32;
    let expanded=app.state::<WindowManager>().expanded.load(Ordering::Relaxed);
    let width=((440.0*scale).round() as i32).min((area.right-area.left-gap*2).max(1));
    let height=(((if expanded{640.0}else{430.0})*scale).round() as i32).min((area.bottom-area.top-gap*2).max(1));
    let (x,y)=placement(area,point,(width,height),gap);
    configure(app)?;
    window.set_size(PhysicalSize::new(width as u32,height as u32)).map_err(|e|e.to_string())?;
    window.set_position(PhysicalPosition::new(x,y)).map_err(|e|e.to_string())?;
    window.show().map_err(|e|e.to_string())?;
    configure(app)?;
    if let Ok(mut dismissed)=app.state::<WindowManager>().dismissed.lock(){*dismissed=None;}
    window.set_focus().map_err(|e|e.to_string())?;
    let _=app.emit("panel-visible",true);
    Ok(())
}

pub fn hide(app: &tauri::AppHandle, lost_focus: bool) {
    let state=app.state::<WindowManager>();
    if let Some(window)=app.get_webview_window("main") {
        if lost_focus&&window.is_visible().unwrap_or(false){if let Ok(mut last)=state.dismissed.lock(){*last=Some(Instant::now());}}
        let _=window.hide();
    }
    state.expanded.store(false,Ordering::Relaxed);
    let _=app.emit("panel-visible",false);
    let _=app.emit("panel-reset",());
    super::taskbar::restore_after_dismiss(app);
}

pub fn toggle(app: &tauri::AppHandle, click: Option<(f64,f64)>) -> Result<(),String> {
    let window=app.get_webview_window("main").ok_or("Панель не найдена")?;
    let just_dismissed=app.state::<WindowManager>().dismissed.lock().ok().and_then(|v|*v)
        .map(|time|time.elapsed()<Duration::from_millis(250)).unwrap_or(false);
    if window.is_visible().unwrap_or(false)||just_dismissed {hide(app,false);Ok(())} else {show(app,click)}
}

pub fn resize(app: &tauri::AppHandle, expanded: bool) -> Result<(),String> {
    app.state::<WindowManager>().expanded.store(expanded,Ordering::Relaxed);
    let point=*app.state::<WindowManager>().anchor.lock().map_err(|_|"Ошибка положения панели")?;
    show(app,point)
}

#[cfg(test)]
mod tests {
    use super::*;
    const WORK:Area=Area{left:0,top:0,right:1920,bottom:1032};
    #[test] fn bottom_tray_stays_above_taskbar(){assert_eq!(placement(WORK,(1840.0,1056.0),(384,250),8),(1528,774));}
    #[test] fn resizing_keeps_bottom_edge_and_reopening_does_not_drift(){
        let small=placement(WORK,(1840.0,1056.0),(384,250),8);
        let large=placement(WORK,(1840.0,1056.0),(384,640),8);
        assert_eq!(small.1+250,large.1+640);
        assert_eq!(small,placement(WORK,(1840.0,1056.0),(384,250),8));
    }
    #[test] fn supports_taskbar_on_other_edges(){
        let top=Area{top:48,..WORK};assert_eq!(placement(top,(1800.0,24.0),(384,250),8).1,56);
        let left=Area{left:48,..WORK};assert_eq!(placement(left,(24.0,800.0),(384,250),8).0,56);
        let right=Area{right:1872,..WORK};assert_eq!(placement(right,(1900.0,800.0),(384,250),8).0,1480);
    }
    #[test] fn accepts_negative_monitor_coordinates(){
        let work=Area{left:-1920,top:-200,right:0,bottom:832};
        assert_eq!(placement(work,(-60.0,856.0),(768,500),16),(-784,316));
    }
}

// Installed once on the owning UI thread, and only on our taskbar HWND.
// Transparent content must never expose the native caption on activation.
pub fn install_taskbar_frame(app:&tauri::AppHandle)->Result<(),String>{
    use windows::Win32::UI::{Shell::SetWindowSubclass,WindowsAndMessaging::{GWL_STYLE,WS_CAPTION,WS_THICKFRAME,WS_SYSMENU,WS_MINIMIZEBOX,WS_MAXIMIZEBOX}};
    let window=app.get_webview_window("taskbar").ok_or("Полоска не найдена")?;
    let hwnd=window.hwnd().map_err(|e|e.to_string())?;
    unsafe {
        if !SetWindowSubclass(hwnd,Some(taskbar_frame),1,0).as_bool(){return Err("Не удалось настроить прозрачное окно".into());}
        let style=GetWindowLongPtrW(hwnd,GWL_STYLE) as u32;
        SetWindowLongPtrW(hwnd,GWL_STYLE,(style & !(WS_CAPTION.0|WS_THICKFRAME.0|WS_SYSMENU.0|WS_MINIMIZEBOX.0|WS_MAXIMIZEBOX.0)) as isize);
        SetWindowPos(hwnd,None,0,0,0,0,SWP_FRAMECHANGED|SWP_NOMOVE|SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE).map_err(|e|e.to_string())?;
    }
    Ok(())
}
unsafe extern "system" fn taskbar_frame(hwnd:windows::Win32::Foundation::HWND,message:u32,wparam:windows::Win32::Foundation::WPARAM,lparam:windows::Win32::Foundation::LPARAM,_id:usize,_data:usize)->windows::Win32::Foundation::LRESULT {
    use windows::Win32::{Foundation::LRESULT,UI::{Shell::{DefSubclassProc,RemoveWindowSubclass},WindowsAndMessaging::{WM_NCCALCSIZE,WM_NCPAINT,WM_NCACTIVATE,WM_NCHITTEST,WM_MOUSEACTIVATE,WM_NCDESTROY,WM_STYLECHANGING,GWL_STYLE,STYLESTRUCT,WS_CAPTION,WS_THICKFRAME,WS_SYSMENU,WS_MINIMIZEBOX,WS_MAXIMIZEBOX,HTCLIENT,MA_NOACTIVATE}}};
    match message {
        WM_STYLECHANGING if wparam.0 as i32==GWL_STYLE.0 => unsafe {
            let result=DefSubclassProc(hwnd,message,wparam,lparam);
            if let Some(style)=(lparam.0 as *mut STYLESTRUCT).as_mut(){style.styleNew &= !(WS_CAPTION.0|WS_THICKFRAME.0|WS_SYSMENU.0|WS_MINIMIZEBOX.0|WS_MAXIMIZEBOX.0);}
            result
        },
        WM_NCCALCSIZE|WM_NCPAINT=>LRESULT(0),
        WM_NCACTIVATE=>LRESULT(1),
        WM_NCHITTEST=>LRESULT(HTCLIENT as isize),
        WM_MOUSEACTIVATE=>LRESULT(MA_NOACTIVATE as isize),
        WM_NCDESTROY=>unsafe{let _=RemoveWindowSubclass(hwnd,Some(taskbar_frame),1);DefSubclassProc(hwnd,message,wparam,lparam)},
        _=>unsafe{DefSubclassProc(hwnd,message,wparam,lparam)}
    }
}
