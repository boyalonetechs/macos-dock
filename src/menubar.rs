use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use cairo::{Context, Format, ImageSurface};
use std::fs;

pub const BAR_HEIGHT: u16 = 28;

// ── Menu Bar ─────────────────────────────────────────────────────

pub struct MenuBar {
    pub window: Window,
    pub depth: u8,
    pub screen_w: u16,
}

#[derive(Clone, Copy, PartialEq)]
pub enum MenuBarAction {
    None,
    AppleMenu,
    ControlCenter,
    Spotlight,
    Battery,
    Calendar,
}

impl MenuBar {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        visual: Visualid,
        depth: u8,
        colormap: Colormap,
        screen_w: u16,
        _screen_h: u16,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let win = conn.generate_id()?;
        conn.create_window(
            depth, win, root, 0, 0,
            screen_w, BAR_HEIGHT, 0,
            WindowClass::INPUT_OUTPUT, visual,
            &CreateWindowAux::new()
                .background_pixel(0)
                .border_pixel(0)
                .colormap(colormap)
                .override_redirect(0)
                .event_mask(
                    EventMask::EXPOSURE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::ENTER_WINDOW
                    | EventMask::LEAVE_WINDOW,
                ),
        )?;

        let wm_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
        let dock_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DOCK")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[dock_type])?;

        let desktop = conn.intern_atom(false, b"_NET_WM_DESKTOP")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, desktop, AtomEnum::CARDINAL, &[0xFFFFFFFF])?;

        let state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let sticky = conn.intern_atom(false, b"_NET_WM_STATE_STICKY")?.reply()?.atom;
        let above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, state, AtomEnum::ATOM, &[sticky, above])?;

        let strut_partial = conn.intern_atom(false, b"_NET_WM_STRUT_PARTIAL")?.reply()?.atom;
        let bar = BAR_HEIGHT as u32;
        conn.change_property32(
            PropMode::REPLACE, win, strut_partial, AtomEnum::CARDINAL,
            &[0, 0, bar, 0, 0, 0, 0, 0, 0, screen_w as u32, bar, screen_w as u32],
        )?;
        let strut = conn.intern_atom(false, b"_NET_WM_STRUT")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, strut, AtomEnum::CARDINAL, &[0, 0, bar, 0])?;

        let wm_name = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let utf8 = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
        conn.change_property8(PropMode::REPLACE, win, wm_name, utf8, b"macOS Menu Bar")?;
        conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, b"macOS Menu Bar")?;

        let skip = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?.reply()?.atom;
        let state2 = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, state2, AtomEnum::ATOM, &[sticky, above, skip])?;

        let blur_atom = conn.intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, blur_atom, AtomEnum::CARDINAL, &[])?;

        conn.map_window(win)?;
        conn.flush()?;

        Ok(Self { window: win, depth, screen_w })
    }

    pub fn handle_event(&self, event: &Event) -> MenuBarAction {
        match event {
            Event::ButtonPress(ev) if ev.event == self.window => {
                let x = ev.event_x as i32;
                let w = self.screen_w as i32;

                if x >= 4 && x <= 30 { return MenuBarAction::AppleMenu; }
                if x >= w - 280 && x < w - 140 { return MenuBarAction::Calendar; }
                if x >= w - 135 && x < w - 100 { return MenuBarAction::Spotlight; }
                if x >= w - 95 && x < w - 65 { return MenuBarAction::ControlCenter; }
                if x >= w - 42 && x < w - 8 { return MenuBarAction::Battery; }
            }
            _ => {}
        }
        MenuBarAction::None
    }

    pub fn render(&self, conn: &RustConnection, active_app_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let w = self.screen_w as i32;
        let h = BAR_HEIGHT as i32;
        let mut surface = ImageSurface::create(Format::ARgb32, w, h)
            .map_err(|e| format!("Cairo surface: {}", e))?;
        let ctx = Context::new(&surface).map_err(|e| format!("Cairo ctx: {}", e))?;

        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        // Near-transparent body — compositor blur fills the visual
        ctx.rectangle(0.0, 0.0, w as f64, h as f64);
        ctx.set_source_rgba(0.08, 0.08, 0.10, 0.06);
        ctx.fill().ok();

        // Bottom border
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.12);
        ctx.set_line_width(1.0);
        ctx.move_to(0.0, h as f64 - 0.5);
        ctx.line_to(w as f64, h as f64 - 0.5);
        ctx.stroke().ok();

        let y_center = h as f64 / 2.0;

        // ── Apple logo ──────────────────────────────────────────
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.new_path();
        ctx.arc(15.0, y_center + 1.5, 5.0, 0.0, 2.0 * std::f64::consts::PI);
        ctx.fill().ok();
        ctx.set_operator(cairo::Operator::Clear);
        ctx.new_path();
        ctx.arc(19.5, y_center - 0.5, 2.5, 0.0, 2.0 * std::f64::consts::PI);
        ctx.fill().ok();
        ctx.set_operator(cairo::Operator::Over);
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.set_line_width(1.3);
        ctx.new_path();
        ctx.move_to(15.0, y_center - 3.5);
        ctx.curve_to(16.5, y_center - 6.0, 19.0, y_center - 6.5, 18.5, y_center - 3.5);
        ctx.stroke().ok();

        // Active app name
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(13.0);
        ctx.move_to(30.0, y_center + 4.5);
        ctx.show_text(active_app_name).ok();

        // Menu items
        let menu_items = ["File", "Edit", "View", "Go", "Window", "Help"];
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(13.0);
        let mut x_pos = 30.0 + measure_text_width(&ctx, active_app_name) + 20.0;
        for item in &menu_items {
            ctx.move_to(x_pos, y_center + 4.5);
            ctx.show_text(item).ok();
            x_pos += measure_text_width(&ctx, item) + 20.0;
        }

        // ── Right side ─────────────────────────────────────────
        let time_str = read_datetime();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(12.5);
        ctx.move_to(w as f64 - 270.0, y_center + 4.5);
        ctx.show_text(&time_str).ok();

        draw_magnifier(&ctx, w as f64 - 118.0, y_center);
        draw_tahoe_control_center(&ctx, w as f64 - 80.0, y_center);

        let battery_info = read_battery();
        draw_battery(&ctx, w as f64 - 25.0, y_center, &battery_info);

        drop(ctx);

        let data = surface.data().map_err(|e| format!("surface data: {}", e))?.to_vec();
        let stride = surface.stride() as usize;

        let gc = conn.generate_id()?;
        conn.create_gc(gc, self.window, &CreateGCAux::new())?;
        let mut packed = Vec::with_capacity(w as usize * h as usize * 4);
        for row in 0..h as usize {
            let row_start = row * stride;
            for col in 0..w as usize {
                let i = row_start + col * 4;
                packed.push(data[i]);
                packed.push(data[i + 1]);
                packed.push(data[i + 2]);
                packed.push(data[i + 3]);
            }
        }
        conn.put_image(ImageFormat::Z_PIXMAP, self.window, gc,
            w as u16, h as u16, 0, 0, 0, self.depth, &packed)?;
        conn.free_gc(gc)?;
        conn.flush()?;
        Ok(())
    }
}

// ── Control Center Popup ─────────────────────────────────────────

const CC_W: i32 = 320;
const CC_H: i32 = 340;

pub struct ControlCenterPopup {
    pub window: Window,
    pub visible: bool,
    depth: u8,
}

struct CcToggle {
    label: &'static str,
    enabled: bool,
}

impl ControlCenterPopup {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        visual: Visualid,
        depth: u8,
        colormap: Colormap,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let win = create_popup_window(conn, root, visual, depth, colormap, CC_W, CC_H, b"Control Center")?;
        Ok(Self { window: win, visible: false, depth })
    }

    pub fn show(&mut self, conn: &RustConnection, screen_w: u16, _screen_h: u16, anchor_x: i32) -> Result<(), Box<dyn std::error::Error>> {
        let x = (anchor_x - CC_W / 2).max(4).min(screen_w as i32 - CC_W - 4);
        let y = BAR_HEIGHT as i32 + 4;
        conn.configure_window(self.window, &ConfigureWindowAux::new()
            .x(x).y(y).stack_mode(StackMode::ABOVE))?;
        conn.map_window(self.window)?;
        conn.flush()?;
        self.visible = true;
        self.render(conn)?;
        Ok(())
    }

    pub fn hide(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        conn.unmap_window(self.window)?;
        conn.flush()?;
        self.visible = false;
        Ok(())
    }

    pub fn should_hide(&self, event: &Event) -> bool {
        if !self.visible { return false; }
        matches!(event, Event::ButtonPress(ev) if ev.event != self.window)
    }

    pub fn render(&self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        render_dark_popup(conn, self.window, self.depth, CC_W, CC_H, |ctx, w_f, _h_f| {
            // Title
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            ctx.set_font_size(13.0);
            ctx.move_to(18.0, 28.0);
            ctx.show_text("Control Center").ok();

            let toggles = [
                CcToggle { label: "Wi-Fi", enabled: true },
                CcToggle { label: "Bluetooth", enabled: true },
                CcToggle { label: "AirDrop", enabled: false },
                CcToggle { label: "Focus", enabled: false },
                CcToggle { label: "Display", enabled: true },
                CcToggle { label: "Sound", enabled: true },
            ];

            let tile_w = (w_f - 18.0 * 2.0 - 10.0) / 2.0;
            let tile_h = 48.0;
            let start_y = 44.0;

            for (i, t) in toggles.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                let tx = 18.0 + col as f64 * (tile_w + 10.0);
                let ty = start_y + row as f64 * (tile_h + 10.0);

                // Tile background
                round_rect(ctx, tx, ty, tile_w, tile_h, 10.0);
                if t.enabled {
                    ctx.set_source_rgba(0.22, 0.48, 0.88, 0.95);
                } else {
                    ctx.set_source_rgba(0.25, 0.25, 0.28, 0.9);
                }
                ctx.fill().ok();

                // Label
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
                ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                ctx.set_font_size(12.0);
                ctx.move_to(tx + 12.0, ty + 22.0);
                ctx.show_text(t.label).ok();

                // Sub-label
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.55);
                ctx.set_font_size(10.0);
                ctx.move_to(tx + 12.0, ty + 38.0);
                let sub = if t.enabled { "On" } else { "Off" };
                ctx.show_text(sub).ok();
            }

            // Slider row at bottom
            let slider_y = start_y + 3.0 * (tile_h + 10.0) + 4.0;
            // Brightness slider
            round_rect(ctx, 18.0, slider_y, w_f - 36.0, 32.0, 10.0);
            ctx.set_source_rgba(0.25, 0.25, 0.28, 0.9);
            ctx.fill().ok();
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.6);
            ctx.set_font_size(10.0);
            ctx.move_to(28.0, slider_y + 20.0);
            ctx.show_text("Display Brightness").ok();
            // Slider track
            let track_y = slider_y + 26.0;
            round_rect(ctx, 28.0, track_y, w_f - 56.0, 4.0, 2.0);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.2);
            ctx.fill().ok();
            // Slider fill
            round_rect(ctx, 28.0, track_y, (w_f - 56.0) * 0.7, 4.0, 2.0);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7);
            ctx.fill().ok();
            // Knob
            ctx.new_path();
            ctx.arc(28.0 + (w_f - 56.0) * 0.7, track_y + 2.0, 5.0, 0.0, 2.0 * std::f64::consts::PI);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            ctx.fill().ok();
        })
    }
}

// ── Battery Popup ────────────────────────────────────────────────

const BAT_POPUP_W: i32 = 280;
const BAT_POPUP_H: i32 = 220;

pub struct BatteryPopup {
    pub window: Window,
    pub visible: bool,
    depth: u8,
}

impl BatteryPopup {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        visual: Visualid,
        depth: u8,
        colormap: Colormap,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let win = create_popup_window(conn, root, visual, depth, colormap, BAT_POPUP_W, BAT_POPUP_H, b"Battery")?;
        Ok(Self { window: win, visible: false, depth })
    }

    pub fn show(&mut self, conn: &RustConnection, screen_w: u16, _screen_h: u16) -> Result<(), Box<dyn std::error::Error>> {
        let x = screen_w as i32 - BAT_POPUP_W - 8;
        let y = BAR_HEIGHT as i32 + 4;
        conn.configure_window(self.window, &ConfigureWindowAux::new()
            .x(x).y(y).stack_mode(StackMode::ABOVE))?;
        conn.map_window(self.window)?;
        conn.flush()?;
        self.visible = true;
        self.render(conn)?;
        Ok(())
    }

    pub fn hide(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        conn.unmap_window(self.window)?;
        conn.flush()?;
        self.visible = false;
        Ok(())
    }

    pub fn should_hide(&self, event: &Event) -> bool {
        if !self.visible { return false; }
        matches!(event, Event::ButtonPress(ev) if ev.event != self.window)
    }

    pub fn render(&self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        render_dark_popup(conn, self.window, self.depth, BAT_POPUP_W, BAT_POPUP_H, |ctx, w_f, _h_f| {
            let info = read_battery();

            // Title
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            ctx.set_font_size(13.0);
            ctx.move_to(18.0, 28.0);
            ctx.show_text("Battery").ok();

            // Big battery icon
            let bat_x = w_f / 2.0 - 30.0;
            let bat_y = 48.0;
            let bat_w = 60.0;
            let bat_h = 28.0;

            // Battery outline
            round_rect(ctx, bat_x, bat_y, bat_w, bat_h, 5.0);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.3);
            ctx.set_line_width(1.5);
            ctx.stroke().ok();

            // Nub
            ctx.rectangle(bat_x + bat_w, bat_y + 7.0, 4.0, bat_h - 14.0);
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.3);
            ctx.fill().ok();

            // Fill
            let fill_w = (bat_w - 4.0) * (info.percentage as f64 / 100.0);
            let fill_color = if info.percentage > 20 {
                (0.3, 0.85, 0.4)
            } else if info.percentage > 10 {
                (0.9, 0.7, 0.1)
            } else {
                (0.9, 0.25, 0.2)
            };
            round_rect(ctx, bat_x + 2.0, bat_y + 2.0, fill_w, bat_h - 4.0, 3.0);
            ctx.set_source_rgba(fill_color.0, fill_color.1, fill_color.2, 0.9);
            ctx.fill().ok();

            // Percentage text
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            ctx.set_font_size(18.0);
            ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            let pct = format!("{}%", info.percentage);
            let te = ctx.text_extents(&pct).unwrap_or(cairo::TextExtents::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
            ctx.move_to(w_f / 2.0 - te.width() / 2.0, bat_y + bat_h + 30.0);
            ctx.show_text(&pct).ok();

            // Status line
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.6);
            ctx.set_font_size(11.0);
            ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            let status = if info.charging {
                "Power Source: Adapter"
            } else {
                "Power Source: Battery"
            };
            ctx.move_to(18.0, bat_y + bat_h + 52.0);
            ctx.show_text(status).ok();

            // Separator
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.1);
            ctx.set_line_width(1.0);
            ctx.move_to(18.0, bat_y + bat_h + 66.0);
            ctx.line_to(w_f - 18.0, bat_y + bat_h + 66.0);
            ctx.stroke().ok();

            // Battery health
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7);
            ctx.set_font_size(11.0);
            ctx.move_to(18.0, bat_y + bat_h + 82.0);
            ctx.show_text("Battery Health: Normal").ok();

            // Time remaining
            ctx.move_to(18.0, bat_y + bat_h + 100.0);
            ctx.show_text("Time Remaining: Calculating...").ok();
        })
    }
}

// ── Helpers ──────────────────────────────────────────────────────

fn create_popup_window(
    conn: &RustConnection,
    root: Window,
    visual: Visualid,
    depth: u8,
    colormap: Colormap,
    w: i32, h: i32,
    title: &[u8],
) -> Result<Window, Box<dyn std::error::Error>> {
    let win = conn.generate_id()?;
    conn.create_window(
        depth, win, root, 0, 0,
        w as u16, h as u16, 0,
        WindowClass::INPUT_OUTPUT, visual,
        &CreateWindowAux::new()
            .background_pixel(0)
            .border_pixel(0)
            .colormap(colormap)
            .override_redirect(1)
            .event_mask(
                EventMask::EXPOSURE
                | EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::POINTER_MOTION
                | EventMask::LEAVE_WINDOW,
            ),
    )?;

    let wm_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
    let dialog = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DIALOG")?.reply()?.atom;
    conn.change_property32(PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[dialog])?;

    let state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
    conn.change_property32(PropMode::REPLACE, win, state, AtomEnum::ATOM, &[above])?;

    let wm_name = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
    let utf8 = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
    conn.change_property8(PropMode::REPLACE, win, wm_name, utf8, title)?;
    conn.flush()?;
    Ok(win)
}

fn render_dark_popup(
    conn: &RustConnection,
    window: Window,
    depth: u8,
    w: i32, h: i32,
    draw: impl FnOnce(&Context, f64, f64),
) -> Result<(), Box<dyn std::error::Error>> {
    let mut surface = ImageSurface::create(Format::ARgb32, w, h)
        .map_err(|e| format!("Cairo surface: {}", e))?;
    let ctx = Context::new(&surface).map_err(|e| format!("Cairo ctx: {}", e))?;

    ctx.set_operator(cairo::Operator::Clear);
    ctx.paint().ok();
    ctx.set_operator(cairo::Operator::Over);

    let w_f = w as f64;
    let h_f = h as f64;

    // Dark frosted background
    round_rect(&ctx, 0.0, 0.0, w_f, h_f, 14.0);
    ctx.set_source_rgba(0.12, 0.12, 0.14, 0.94);
    ctx.fill_preserve().ok();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
    ctx.set_line_width(1.0);
    ctx.stroke().ok();

    draw(&ctx, w_f, h_f);

    drop(ctx);

    let data = surface.data().map_err(|e| format!("surface data: {}", e))?.to_vec();
    let stride = surface.stride() as usize;

    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new())?;
    let mut packed = Vec::with_capacity(w as usize * h as usize * 4);
    for row in 0..h as usize {
        let row_start = row * stride;
        for col in 0..w as usize {
            let i = row_start + col * 4;
            packed.push(data[i]);
            packed.push(data[i + 1]);
            packed.push(data[i + 2]);
            packed.push(data[i + 3]);
        }
    }
    conn.put_image(ImageFormat::Z_PIXMAP, window, gc,
        w as u16, h as u16, 0, 0, 0, depth, &packed)?;
    conn.free_gc(gc)?;
    conn.flush()?;
    Ok(())
}

fn round_rect(ctx: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    ctx.new_path();
    ctx.move_to(x + r, y);
    ctx.line_to(x + w - r, y);
    ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.line_to(x + w, y + h - r);
    ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.line_to(x + r, y + h);
    ctx.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    ctx.line_to(x, y + r);
    ctx.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    ctx.close_path();
}

// ── Battery icon (inline for bar) ────────────────────────────────

struct BatteryInfo {
    percentage: u32,
    charging: bool,
}

fn read_battery() -> BatteryInfo {
    let paths = [
        "/sys/class/power_supply/BAT0",
        "/sys/class/power_supply/BAT1",
        "/sys/class/power_supply/BAT2",
    ];
    for base in &paths {
        let cap_path = format!("{}/capacity", base);
        let status_path = format!("{}/status", base);
        if let (Ok(cap_str), Ok(status_str)) = (
            fs::read_to_string(&cap_path),
            fs::read_to_string(&status_path),
        ) {
            if let Ok(pct) = cap_str.trim().parse::<u32>() {
                let charging = status_str.trim().contains("Charging") || status_str.trim().contains("Full");
                return BatteryInfo { percentage: pct.min(100), charging };
            }
        }
    }
    BatteryInfo { percentage: 100, charging: false }
}

fn draw_battery(ctx: &Context, x: f64, y_center: f64, info: &BatteryInfo) {
    let bw = 18.0;
    let bh = 10.0;
    let bx = x - bw;
    let by = y_center - bh / 2.0;

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.75);
    ctx.set_line_width(1.0);
    round_rect(ctx, bx, by, bw, bh, 2.0);
    ctx.stroke().ok();

    ctx.rectangle(bx + bw, by + 2.5, 2.0, bh - 5.0);
    ctx.fill().ok();

    let fill_w = (bw - 3.0) * (info.percentage as f64 / 100.0);
    if info.charging || info.percentage > 20 {
        ctx.set_source_rgba(0.3, 0.85, 0.4, 0.9);
    } else if info.percentage > 10 {
        ctx.set_source_rgba(0.9, 0.7, 0.1, 0.9);
    } else {
        ctx.set_source_rgba(0.9, 0.25, 0.2, 0.9);
    }
    ctx.rectangle(bx + 1.5, by + 1.5, fill_w, bh - 3.0);
    ctx.fill().ok();
}

// ── Tahoe Control Center icon ────────────────────────────────────

fn draw_tahoe_control_center(ctx: &Context, x: f64, y_center: f64) {
    let w = 18.0;
    let h = 14.0;
    let rx = x - w / 2.0;
    let ry = y_center - h / 2.0;
    let r = 3.5;

    // Rounded rect
    ctx.new_path();
    ctx.move_to(rx + r, ry);
    ctx.line_to(rx + w - r, ry);
    ctx.arc(rx + w - r, ry + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.line_to(rx + w, ry + h - r);
    ctx.arc(rx + w - r, ry + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.line_to(rx + r, ry + h);
    ctx.arc(rx + r, ry + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    ctx.line_to(rx, ry + r);
    ctx.arc(rx + r, ry + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    ctx.close_path();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
    ctx.fill().ok();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.90);
    ctx.set_line_width(1.5);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Top bar + knob
    let bar_y1 = ry + 4.0;
    let bar_x1 = rx + 3.0;
    let bar_x2 = rx + w - 3.0;
    ctx.new_path();
    ctx.move_to(bar_x1, bar_y1);
    ctx.line_to(bar_x2, bar_y1);
    ctx.stroke().ok();
    ctx.new_path();
    ctx.arc(bar_x1 + (bar_x2 - bar_x1) * 0.65, bar_y1, 2.2, 0.0, 2.0 * std::f64::consts::PI);
    ctx.fill().ok();

    // Bottom bar + knob
    let bar_y2 = ry + h - 4.0;
    ctx.new_path();
    ctx.move_to(bar_x1, bar_y2);
    ctx.line_to(bar_x2, bar_y2);
    ctx.stroke().ok();
    ctx.new_path();
    ctx.arc(bar_x1 + (bar_x2 - bar_x1) * 0.35, bar_y2, 2.2, 0.0, 2.0 * std::f64::consts::PI);
    ctx.fill().ok();

    ctx.set_line_cap(cairo::LineCap::Butt);
}

// ── Spotlight magnifier ──────────────────────────────────────────

fn draw_magnifier(ctx: &Context, x: f64, y_center: f64) {
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    ctx.set_line_width(1.3);
    ctx.new_path();
    ctx.arc(x, y_center - 1.0, 4.5, 0.0, 2.0 * std::f64::consts::PI);
    ctx.stroke().ok();
    ctx.move_to(x + 3.2, y_center + 2.2);
    ctx.line_to(x + 6.5, y_center + 5.5);
    ctx.stroke().ok();
}

// ── Text helper ──────────────────────────────────────────────────

fn measure_text_width(ctx: &Context, text: &str) -> f64 {
    if let Ok(extents) = ctx.text_extents(text) {
        extents.width()
    } else {
        0.0
    }
}

// ── Date/Time ────────────────────────────────────────────────────

fn read_datetime() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    let mut y = 1970i32;
    let mut remaining = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days = [
        (31, "Jan"), (28, "Feb"), (31, "Mar"), (30, "Apr"),
        (31, "May"), (30, "Jun"), (31, "Jul"), (31, "Aug"),
        (30, "Sep"), (31, "Oct"), (30, "Nov"), (31, "Dec"),
    ];
    for (i, &(d, _)) in month_days.iter().enumerate() {
        let dim = if i == 1 && is_leap(y) { 29 } else { d };
        if remaining < dim as u64 { break; }
        remaining -= dim as u64;
    }
    let day = remaining as u32 + 1;

    let dow = days % 7;
    let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let dow_name = day_names[(dow as usize + 4) % 7];

    format!("{:>3}  {:>2} {:>2}:{:02}", dow_name, day, hours, minutes)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}
