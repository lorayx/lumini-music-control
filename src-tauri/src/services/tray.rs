use tauri::{menu::{Menu, MenuItem}, tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState}};
pub fn create(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app,"show","Открыть LUMINI",true,None::<&str>)?;
    let quit = MenuItem::with_id(app,"quit","Выйти",true,None::<&str>)?;
    let menu = Menu::with_items(app,&[&show,&quit])?;
    TrayIconBuilder::with_id("lumini")
        .icon(app.default_window_icon().expect("bundled icon").clone()).tooltip("LUMINI · Яндекс Музыка")
        .menu(&menu).show_menu_on_left_click(false)
        .on_menu_event(|app,event| match event.id.as_ref() { "show" => { let _ = super::window::show(app,None); }, "quit" => app.exit(0), _=>{} })
        .on_tray_icon_event(|tray,event| { if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, .. } = event {
            let _=super::window::toggle(tray.app_handle(),Some((position.x,position.y)));
        }}).build(app)?;
    Ok(())
}
