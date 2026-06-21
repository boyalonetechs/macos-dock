mod theme;
mod renderer;
mod x11_window;
mod app;
mod desktop;
mod popup;

use renderer::Renderer;
use theme::MacTheme;
use x11_window::DockWindow;
use popup::ResizerPopup;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::Rectangle;

use std::thread;
use std::time::Duration;

fn headroom(theme: &MacTheme) -> i32 {
    ((0.7 * theme.icon_size as f64 * (theme.max_zoom - 1.0)) + theme.padding_y).ceil() as i32
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // FIX 1: was new_dark(), renamed to new_liquid_glass()
    let mut theme = MacTheme::new_liquid_glass();
    let zoom_headroom = headroom(&theme);
    let full_height = theme.dock_height() + zoom_headroom;

    let mut dock = DockWindow::new(full_height as u16)?;
    dock.set_title("macOS Dock")?;

    let mut renderer = Renderer::new(dock.width as i32, full_height);
    let mut manager = app::AppManager::new();
    let screen_h = dock.screen_h;

    // Create the resizer popup (shares the same connection via the dock)
    let mut popup = ResizerPopup::new(
        &dock.conn,
        dock.root,
        dock.visual,
        dock.depth,
        dock.colormap,
        theme.icon_size as f64,
    )?;

    let mut need_resize = true;
    let mut need_redraw = true;

    loop {
        while let Some(event) = dock.next_event()? {
            // Let popup handle its own events first
            if popup.visible {
                if popup.should_hide(&event) {
                    popup.hide(&dock.conn)?;
                    continue;
                }
                if popup.handle_event(&event) {
                    // Update theme with new icon size from slider
                    let new_size = popup.icon_size.round() as i32;
                    if new_size != theme.icon_size {
                        theme.icon_size = new_size;
                        theme.icon_spacing = new_size + 10;
                        need_resize = true;
                        need_redraw = true;
                    }
                    popup.render(&dock.conn)?;
                    continue;
                }
            }

            match event {
                Event::Expose(ev) if ev.window == dock.window => {
                    need_redraw = true;
                }
                Event::MotionNotify(ev) if ev.event == dock.window => {
                    dock.cursor_x = ev.event_x as f64;
                    need_redraw = true;
                }
                Event::LeaveNotify(ev) if ev.event == dock.window => {
                    dock.cursor_x = -10000.0;
                    need_redraw = true;
                }
                Event::ButtonPress(ev) if ev.event == dock.window => {
                    let click_x = ev.event_x as f64;
                    let mut hit_icon = false;

                    for icon in &manager.icons {
                        let w = theme.icon_size as f64 * icon.zoom;
                        if click_x >= icon.x - w/2.0 && click_x <= icon.x + w/2.0 {
                            if icon.item_type == app::DockItemType::Separator { continue; }
                            hit_icon = true;

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

                    // If clicked empty space on dock, show the resizer popup
                    if !hit_icon {
                        popup.icon_size = theme.icon_size as f64;
                        popup.show(&dock.conn, ev.root_x, ev.root_y)?;
                        popup.render(&dock.conn)?;
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
            // FIX 2: explicit f64 annotation so .ceil() resolves without ambiguity
            let padding_x: f64 = theme.padding_x;
            let bottom_margin = theme.bottom_margin;
            let zoom_headroom = headroom(&theme);
            let bg_height = theme.dock_height();
            let full_height = bg_height + zoom_headroom;
            let mut actual_width: f64 = 0.0;
            for icon in &manager.icons {
                if icon.item_type == app::DockItemType::Separator {
                    actual_width += spacing * 0.5;
                } else {
                    actual_width += spacing;
                }
            }

            let dock_w = (actual_width + 2.0 * padding_x).ceil() as u16;
            let dock_x = ((dock.screen_w - dock_w) / 2) as i16;
            let dock_y = (screen_h as i16) - (full_height as i16) - bottom_margin as i16;

            if need_resize && (dock_w != dock.width || full_height != dock.height as i32) {
                dock.configure(dock_x, dock_y, dock_w, full_height as u16)?;
                dock.width = dock_w;
                dock.height = full_height as u16;
                // Set input shape so only the background area accepts clicks
                dock.set_input_shape(&Rectangle {
                    x: 0,
                    y: zoom_headroom as i16,
                    width: dock_w,
                    height: bg_height as u16,
                });
                need_redraw = true;
            }
            need_resize = false;

            manager.update_zoom(dock.cursor_x, theme.sigma, theme.max_zoom);
        }

        // Smooth interpolation for zoom animation
        let mut zoom_changed = false;
        for icon in &mut manager.icons {
            if (icon.zoom - icon.target_zoom).abs() > 0.005 {
                icon.zoom += (icon.target_zoom - icon.zoom) * 0.25;
                zoom_changed = true;
            } else {
                icon.zoom = icon.target_zoom;
            }
        }
        if zoom_changed {
            need_redraw = true;
        }

        if need_redraw {
            let zh = headroom(&theme);
            let fh = theme.dock_height() + zh;
            renderer.resize(dock.width as i32, fh, zh);
            let (pixels, stride) = renderer.render(&theme, &mut manager);
            let _ = dock.push_pixels(&pixels, dock.width, fh as u16, stride);
            need_redraw = false;
        }

        thread::sleep(Duration::from_millis(16));
    }
}