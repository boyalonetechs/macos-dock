use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use cairo::{Context, Format, ImageSurface};

const POPUP_W: i32 = 260;
const POPUP_H: i32 = 140;
const CORNER_R: f64 = 14.0;

pub struct SettingsPopup {
    pub window: Window,
    pub visible: bool,
    pub dock_hidden: bool,
    pub menubar_hidden: bool,
    depth: u8,
}

impl SettingsPopup {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        visual: Visualid,
        depth: u8,
        colormap: Colormap,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let win = conn.generate_id()?;
        conn.create_window(
            depth, win, root, 0, 0,
            POPUP_W as u16, POPUP_H as u16, 0,
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
        conn.change_property8(PropMode::REPLACE, win, wm_name, utf8, b"Dock Settings")?;
        conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, b"Dock Settings")?;
        conn.flush()?;

        Ok(Self { window: win, visible: false, depth, dock_hidden: false, menubar_hidden: false })
    }

    pub fn show(&mut self, conn: &RustConnection, screen_w: u16, screen_h: u16) -> Result<(), Box<dyn std::error::Error>> {
        let x = (screen_w as i32 - POPUP_W) / 2;
        let y = (screen_h as i32 - POPUP_H) / 2;
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

    /// Returns true if any toggle was clicked
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::ButtonPress(ev) if ev.event == self.window => {
                let x = ev.event_x as i32;
                let y = ev.event_y as i32;
                let toggle_x = POPUP_W - 50;
                let toggle_w = 44;
                let toggle_h = 24;

                // "Hide Dock" toggle — row at y=42..66
                if x >= toggle_x && x <= toggle_x + toggle_w
                    && y >= 42 && y <= 42 + toggle_h {
                    self.dock_hidden = !self.dock_hidden;
                    return true;
                }
                // "Hide Menu Bar" toggle — row at y=82..106
                if x >= toggle_x && x <= toggle_x + toggle_w
                    && y >= 82 && y <= 82 + toggle_h {
                    self.menubar_hidden = !self.menubar_hidden;
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    pub fn should_hide(&self, event: &Event) -> bool {
        if !self.visible { return false; }
        matches!(event, Event::ButtonPress(ev) if ev.event != self.window)
            || matches!(event, Event::LeaveNotify(ev) if ev.event == self.window)
    }

    pub fn render(&self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        let w = POPUP_W;
        let h = POPUP_H;
        let mut surface = ImageSurface::create(Format::ARgb32, w, h)
            .map_err(|e| format!("Cairo surface: {}", e))?;
        let ctx = Context::new(&surface).map_err(|e| format!("Cairo ctx: {}", e))?;

        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        let wf = w as f64;
        let hf = h as f64;

        // Background
        round_rect(&ctx, 0.0, 0.0, wf, hf, CORNER_R);
        ctx.set_source_rgba(0.12, 0.12, 0.14, 0.94);
        ctx.fill_preserve().ok();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.18);
        ctx.set_line_width(1.0);
        ctx.stroke().ok();

        // Title
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(14.0);
        ctx.move_to(18.0, 28.0);
        ctx.show_text("Settings").ok();

        // "Hide Dock" label
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.90);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(13.0);
        ctx.move_to(18.0, 60.0);
        ctx.show_text("Hide Dock").ok();

        // Toggle switch for "Hide Dock"
        let toggle_x = wf - 50.0;
        let toggle_y = 46.0;
        let toggle_w = 44.0;
        let toggle_h = 24.0;
        let toggle_r = toggle_h / 2.0;

        round_rect(&ctx, toggle_x, toggle_y, toggle_w, toggle_h, toggle_r);
        if self.dock_hidden {
            ctx.set_source_rgba(0.3, 0.7, 0.4, 0.9);
        } else {
            ctx.set_source_rgba(0.4, 0.4, 0.45, 0.8);
        }
        ctx.fill().ok();

        let knob_r = 9.0;
        let knob_x = if self.dock_hidden {
            toggle_x + toggle_w - toggle_r
        } else {
            toggle_x + toggle_r
        };
        let knob_y = toggle_y + toggle_h / 2.0;
        ctx.new_path();
        ctx.arc(knob_x, knob_y, knob_r, 0.0, 2.0 * std::f64::consts::PI);
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.fill().ok();

        // "Hide Menu Bar" label
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.90);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(13.0);
        ctx.move_to(18.0, 100.0);
        ctx.show_text("Hide Menu Bar").ok();

        // Toggle switch for "Hide Menu Bar"
        let toggle2_y = 86.0;
        round_rect(&ctx, toggle_x, toggle2_y, toggle_w, toggle_h, toggle_r);
        if self.menubar_hidden {
            ctx.set_source_rgba(0.3, 0.7, 0.4, 0.9);
        } else {
            ctx.set_source_rgba(0.4, 0.4, 0.45, 0.8);
        }
        ctx.fill().ok();

        let knob2_x = if self.menubar_hidden {
            toggle_x + toggle_w - toggle_r
        } else {
            toggle_x + toggle_r
        };
        let knob2_y = toggle2_y + toggle_h / 2.0;
        ctx.new_path();
        ctx.arc(knob2_x, knob2_y, knob_r, 0.0, 2.0 * std::f64::consts::PI);
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.fill().ok();

        // Push pixels
        drop(ctx);
        let data = surface.data().map_err(|e| format!("surface data: {}", e))?.to_vec();
        let stride = surface.stride() as usize;

        let gc = conn.generate_id()?;
        conn.create_gc(gc, self.window, &CreateGCAux::new())?;
        let bpp = if self.depth == 32 { 4 } else { 3 };
        let mut packed = Vec::with_capacity(w as usize * h as usize * bpp);
        for y in 0..h as usize {
            let row_start = y * stride;
            for x in 0..w as usize {
                let i = row_start + x * 4;
                packed.push(data[i]);
                packed.push(data[i + 1]);
                packed.push(data[i + 2]);
                if bpp == 4 { packed.push(data[i + 3]); }
            }
        }
        conn.put_image(ImageFormat::Z_PIXMAP, self.window, gc,
            w as u16, h as u16, 0, 0, 0, self.depth, &packed)?;
        conn.free_gc(gc)?;
        conn.flush()?;
        Ok(())
    }
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
