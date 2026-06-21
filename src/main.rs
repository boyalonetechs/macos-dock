mod theme;
mod renderer;
mod x11_window;
mod app;
mod desktop;

use renderer::Renderer;
use theme::MacTheme;
use x11_window::DockWindow;
use x11rb::protocol::Event;

use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = MacTheme::new_dark();
    let dock_height = theme.dock_height;

    let mut dock = DockWindow::new(dock_height as u16)?;
    dock.set_title("macOS Dock")?;

    let mut renderer = Renderer::new(dock.width as i32, dock.height as i32);
    let mut manager = app::AppManager::new();
    let screen_h = dock.screen_h;

    let mut need_resize = false;
    let mut need_redraw = true;

    loop {
        while let Some(event) = dock.next_event()? {
            match event {
                Event::Expose(ev) if ev.window == dock.window => {
                    need_redraw = true;
                }
                Event::MotionNotify(ev) if ev.event == dock.window => {
                    dock.cursor_x = ev.root_x as f64;
                    need_redraw = true;
                }
                Event::LeaveNotify(ev) if ev.event == dock.window => {
                    dock.cursor_x = dock.width as f64 / 2.0;
                    for icon in manager.icons.iter_mut() {
                        icon.zoom = 1.0;
                    }
                    need_redraw = true;
                }
                Event::ButtonPress(ev) if ev.event == dock.window => {
                    let click_x = ev.event_x as f64;
                    for icon in &manager.icons {
                        let w = theme.icon_size as f64 * icon.zoom;
                        if click_x >= icon.x - w/2.0 && click_x <= icon.x + w/2.0 {
                            if icon.item_type == app::DockItemType::Separator { continue; }
                            
                            if icon.item_type == app::DockItemType::Folder && icon.name == "Downloads" {
                                let home = std::env::var("HOME").unwrap_or_default();
                                let _ = std::process::Command::new("xdg-open").arg(format!("{}/Downloads", home)).spawn();
                            } else if icon.item_type == app::DockItemType::Trash {
                                let _ = std::process::Command::new("xdg-open").arg("trash:///").spawn();
                            } else if let Some(idx) = icon.entry_index {
                                if let Some(entry) = manager.entries.get(idx) {
                                    let _ = std::process::Command::new("gtk-launch").arg(&entry.filename).spawn();
                                }
                            } else if icon.name == "Finder" {
                                let home = std::env::var("HOME").unwrap_or_default();
                                let _ = std::process::Command::new("xdg-open").arg(&home).spawn();
                            } else if icon.name == "System Settings" {
                                let _ = std::process::Command::new("gtk-launch").arg("gnome-control-center").spawn();
                            }
                            break;
                        }
                    }
                }
                _ => {}
            }
        }

        let windows = dock.get_running_windows().unwrap_or_default();
        let win_class_getter = |wid: u32| dock.get_window_class(wid);
        let prev_count = manager.icons.len();
        manager.sync_windows(&windows, &win_class_getter);

        if manager.icons.len() != prev_count {
            need_resize = true;
            need_redraw = true;
        }

        if !manager.icons.is_empty() {
            let spacing = theme.icon_spacing as f64;
            let margin = theme.margin;
            let mut actual_width = 0.0;
            for icon in &manager.icons {
                if icon.item_type == app::DockItemType::Separator {
                    actual_width += spacing * 0.5;
                } else {
                    actual_width += spacing;
                }
            }
            
            let dock_w = (actual_width + 2.0 * margin).ceil() as u16;
            let dock_x = ((dock.screen_w - dock_w) / 2) as i16;
            let dock_y = (screen_h as i16) - (dock_height as i16);

            if need_resize && dock_w != dock.width {
                dock.configure(dock_x, dock_y, dock_w, dock_height as u16)?;
                dock.width = dock_w;
                need_redraw = true;
            }
            need_resize = false;

            manager.update_zoom(dock.cursor_x, theme.sigma, theme.max_zoom);
        }

        if need_redraw {
            renderer.resize(dock.width as i32, dock.height as i32);
            renderer.render(&theme, &mut manager);
            let pixels = renderer.copy_data();
            let _ = dock.push_pixels(&pixels, dock.width, dock.height, renderer.stride());
            need_redraw = false;
        }

        thread::sleep(Duration::from_millis(16));
    }
}
