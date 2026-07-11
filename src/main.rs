mod theme;
mod renderer;
mod x11_window;
mod app;
mod desktop;
mod popup;
mod appgrid;

use renderer::Renderer;
use theme::MacTheme;
use x11_window::DockWindow;
use popup::ResizerPopup;
use appgrid::AppGrid;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::Rectangle;

use std::thread;
use std::time::Duration;

/// Headroom = transparent band above the pill where zoomed icons pop into.
/// Icons grow bottom-anchored, lifting by (zoom-1)*icon_size*0.5,
/// so headroom only needs to cover that upward lift.
fn headroom(theme: &MacTheme) -> i32 {
    let extra = theme.icon_size as f64 * (theme.max_zoom - 1.0) * 0.5;
    extra.ceil() as i32
}

/// Compute exact dock pixel width matching the renderer's layout.
/// Uses unzoomed icon size so the dock never shakes on hover.
fn compute_dock_width(theme: &MacTheme, manager: &app::AppManager) -> u16 {
    let edge_gap = (theme.icon_spacing - theme.icon_size) as f64;
    let sep_width = 16.0_f64;
    let padding_x: f64 = theme.padding_x;

    let mut content_width = -edge_gap;
    for icon in &manager.icons {
        if icon.item_type == app::DockItemType::Separator {
            content_width += sep_width + edge_gap;
        } else {
            content_width += theme.icon_size as f64 + edge_gap;
        }
    }
    (content_width + 2.0 * padding_x).ceil().max(80.0) as u16
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut theme = MacTheme::new_liquid_glass();
    let zoom_headroom = headroom(&theme);
    let bg_height = theme.dock_height();
    let full_height = bg_height + zoom_headroom;

    // Start with a sensible initial width — will be corrected on first resize tick
    let initial_width: u16 = 600;
    let mut dock = DockWindow::new(initial_width, full_height as u16)?;
    dock.set_title("macOS Dock")?;

    let mut renderer = Renderer::new(initial_width as i32, full_height);
    let mut manager = app::AppManager::new();
    let screen_h = dock.screen_h;

    let grid_entries: Vec<desktop::DesktopEntry> = manager.all_entries().to_vec();
    let mut app_grid = AppGrid::new(
        &dock.conn,
        dock.root,
        dock.visual,
        dock.depth,
        dock.colormap,
        dock.screen_w,
        dock.screen_h,
        grid_entries,
    )?;

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
            // App grid takes priority when visible
            if app_grid.visible {
                if app_grid.should_hide(&event) {
                    app_grid.hide(&dock.conn)?;
                    continue;
                }
                if let Some(file_path) = app_grid.handle_event(&event) {
                    let _ = std::process::Command::new("gio")
                        .args(["launch", &file_path]).spawn();
                    app_grid.hide(&dock.conn)?;
                    continue;
                }
                if matches!(event, Event::Expose(ev) if ev.window == app_grid.window) {
                    app_grid.render(&dock.conn)?;
                    continue;
                }
            }

            if popup.visible {
                if popup.should_hide(&event) {
                    popup.hide(&dock.conn)?;
                    continue;
                }
                if popup.handle_event(&event) {
                    let new_size = popup.icon_size.round() as i32;
                    if new_size != theme.icon_size {
                        theme.icon_size = new_size;
                        theme.icon_spacing = new_size + 8;
                        // Keep pill shape: radius = half of new dock height
                        theme.corner_radius = (theme.dock_height() as f64 / 2.0).floor();
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
                    let click_y = ev.event_y as f64;

                    if click_y < zoom_headroom as f64 {
                        continue;
                    }

                    match ev.detail {
                        1 => { // Left click
                            for icon in &manager.icons {
                                let w = theme.icon_size as f64 * icon.zoom;
                                if click_x >= icon.x - w / 2.0 && click_x <= icon.x + w / 2.0 {
                                    if icon.item_type == app::DockItemType::Separator { continue; }
                                    if icon.item_type == app::DockItemType::Launcher {
                                        app_grid.toggle(&dock.conn)?;
                                        break;
                                    }
                                    if icon.item_type == app::DockItemType::Folder && icon.name == "Downloads" {
                                        let home = std::env::var("HOME").unwrap_or_default();
                                        let _ = std::process::Command::new("xdg-open")
                                            .arg(format!("{}/Downloads", home)).spawn();
                                    } else if icon.item_type == app::DockItemType::Trash {
                                        let _ = std::process::Command::new("xdg-open")
                                            .arg("trash:///").spawn();
                                    } else if let Some(idx) = icon.entry_index {
                                        if let Some(entry) = manager.entries.get(idx) {
                                            let _ = std::process::Command::new("gio")
                                                .args(["launch", &entry.file_path.to_string_lossy()]).spawn();
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                        3 => { // Right click
                            popup.icon_size = theme.icon_size as f64;
                            popup.show(&dock.conn, ev.root_x, ev.root_y)?;
                            popup.render(&dock.conn)?;
                        }
                        _ => {}
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
            let zh = headroom(&theme);
            let bg_h = theme.dock_height();
            let full_h = bg_h + zh;

            let dock_w = compute_dock_width(&theme, &manager);
            let dock_x = ((dock.screen_w as i32 - dock_w as i32) / 2) as i16;
            // Pill sits flush at screen bottom (bottom_margin lifts it slightly)
            let dock_y = screen_h as i16 - full_h as i16 - theme.bottom_margin as i16;

            if need_resize || dock_w != dock.width || full_h != dock.height as i32 {
                dock.configure(dock_x, dock_y, dock_w, full_h as u16)?;
                dock.width = dock_w;
                dock.height = full_h as u16;

                // Input shape covers only the pill, not the headroom above it
                dock.set_input_shape(&Rectangle {
                    x: 0,
                    y: zh as i16,
                    width: dock_w,
                    height: bg_h as u16,
                });
                need_redraw = true;
            }
            need_resize = false;

            manager.update_zoom(dock.cursor_x, theme.sigma, theme.max_zoom);
        }

        // Smooth zoom lerp
        let mut zoom_changed = false;
        for icon in &mut manager.icons {
            if (icon.zoom - icon.target_zoom).abs() > 0.005 {
                icon.zoom += (icon.target_zoom - icon.zoom) * 0.45;
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

        thread::sleep(Duration::from_millis(12));
    }
}