use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use cairo::{Context, Format, ImageSurface};
use crate::theme::{MacTheme, ThemeMode};

const POPUP_W: i32 = 280;
const POPUP_H: i32 = 196;
const CORNER_R: f64 = 14.0;

const CONTROL_X: f64 = 20.0;
const CONTROL_Y: f64 = 62.0;
const CONTROL_W: f64 = 240.0;
const CONTROL_H: f64 = 32.0;

const TOGGLE_W: f64 = 44.0;
const TOGGLE_H: f64 = 24.0;
const TOGGLE_X: f64 = POPUP_W as f64 - 20.0 - TOGGLE_W;
const DOCK_TOGGLE_Y: f64 = 116.0;
const MENUBAR_TOGGLE_Y: f64 = 156.0;

pub struct SettingsPopup {
    pub window: Window,
    pub visible: bool,
    pub dock_hidden: bool,
    pub menubar_hidden: bool,
    pub mode: ThemeMode,
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

        Ok(Self {
            window: win,
            visible: false,
            dock_hidden: false,
            menubar_hidden: false,
            mode: ThemeMode::Dark,
            depth,
        })
    }

    pub fn show(
        &mut self,
        conn: &RustConnection,
        screen_w: u16,
        screen_h: u16,
        theme: &MacTheme,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let x = (screen_w as i32 - POPUP_W) / 2;
        let y = (screen_h as i32 - POPUP_H) / 2;
        conn.configure_window(self.window, &ConfigureWindowAux::new()
            .x(x).y(y).stack_mode(StackMode::ABOVE))?;
        conn.map_window(self.window)?;
        conn.flush()?;
        self.visible = true;
        self.render(conn, theme)?;
        Ok(())
    }

    pub fn hide(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        conn.unmap_window(self.window)?;
        conn.flush()?;
        self.visible = false;
        Ok(())
    }

    /// Returns true if a control changed.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::ButtonPress(ev) if ev.event == self.window => {
                let x = ev.event_x as f64;
                let y = ev.event_y as f64;

                if x >= CONTROL_X && x <= CONTROL_X + CONTROL_W
                    && y >= CONTROL_Y && y <= CONTROL_Y + CONTROL_H {
                    let picked = if x < CONTROL_X + CONTROL_W / 2.0 {
                        ThemeMode::Light
                    } else {
                        ThemeMode::Dark
                    };
                    let changed = self.mode != picked;
                    self.mode = picked;
                    return changed;
                }

                if x >= TOGGLE_X && x <= TOGGLE_X + TOGGLE_W {
                    if y >= DOCK_TOGGLE_Y && y <= DOCK_TOGGLE_Y + TOGGLE_H {
                        self.dock_hidden = !self.dock_hidden;
                        return true;
                    }
                    if y >= MENUBAR_TOGGLE_Y && y <= MENUBAR_TOGGLE_Y + TOGGLE_H {
                        self.menubar_hidden = !self.menubar_hidden;
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub fn should_hide(&self, event: &Event) -> bool {
        if !self.visible { return false; }
        matches!(event, Event::ButtonPress(ev) if ev.event != self.window)
    }

    pub fn render(&self, conn: &RustConnection, theme: &MacTheme) -> Result<(), Box<dyn std::error::Error>> {
        let w = POPUP_W;
        let h = POPUP_H;
        let mut surface = ImageSurface::create(Format::ARgb32, w, h)
            .map_err(|e| format!("Cairo surface: {}", e))?;
        let ctx = Context::new(&surface).map_err(|e| format!("Cairo ctx: {}", e))?;

        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        let wf = w as f64;

        // Panel
        round_rect(&ctx, 0.0, 0.0, wf, h as f64, CORNER_R);
        ctx.set_source_rgba(theme.panel_bg.0, theme.panel_bg.1, theme.panel_bg.2, theme.panel_bg.3);
        ctx.fill_preserve().ok();
        ctx.set_source_rgba(theme.panel_border.0, theme.panel_border.1, theme.panel_border.2, theme.panel_border.3);
        ctx.set_line_width(1.0);
        ctx.stroke().ok();

        // Title
        ctx.set_source_rgba(theme.text_primary.0, theme.text_primary.1, theme.text_primary.2, theme.text_primary.3);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(15.0);
        ctx.move_to(20.0, 31.0);
        ctx.show_text("Dock & Menu Bar").ok();

        ctx.set_source_rgba(theme.text_muted.0, theme.text_muted.1, theme.text_muted.2, theme.text_muted.3);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(11.0);
        ctx.move_to(20.0, 54.0);
        ctx.show_text("Appearance").ok();

        // Segmented Light / Dark control
        round_rect(&ctx, CONTROL_X, CONTROL_Y, CONTROL_W, CONTROL_H, 8.0);
        ctx.set_source_rgba(theme.control_bg.0, theme.control_bg.1, theme.control_bg.2, theme.control_bg.3);
        ctx.fill().ok();

        let seg_w = (CONTROL_W - 4.0) / 2.0;
        let selected_x = match self.mode {
            ThemeMode::Light => CONTROL_X + 2.0,
            ThemeMode::Dark => CONTROL_X + 2.0 + seg_w,
        };
        round_rect(&ctx, selected_x, CONTROL_Y + 2.0, seg_w, CONTROL_H - 4.0, 7.0);
        ctx.set_source_rgba(
            theme.control_selected.0,
            theme.control_selected.1,
            theme.control_selected.2,
            theme.control_selected.3,
        );
        ctx.fill_preserve().ok();
        ctx.set_source_rgba(theme.panel_border.0, theme.panel_border.1, theme.panel_border.2, theme.panel_border.3);
        ctx.set_line_width(1.0);
        ctx.stroke().ok();

        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(12.5);
        draw_segment_label(&ctx, "Light", CONTROL_X + 2.0 + seg_w / 2.0, CONTROL_Y, self.mode == ThemeMode::Light, theme);
        draw_segment_label(&ctx, "Dark", CONTROL_X + 2.0 + seg_w * 1.5, CONTROL_Y, self.mode == ThemeMode::Dark, theme);

        // Divider between appearance and the visibility switches
        ctx.set_source_rgba(theme.panel_border.0, theme.panel_border.1, theme.panel_border.2, theme.panel_border.3);
        ctx.set_line_width(1.0);
        ctx.move_to(20.0, 108.0);
        ctx.line_to(wf - 20.0, 108.0);
        ctx.stroke().ok();

        // Hide Dock
        ctx.set_source_rgba(theme.text_primary.0, theme.text_primary.1, theme.text_primary.2, theme.text_primary.3);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(13.0);
        ctx.move_to(20.0, DOCK_TOGGLE_Y + 16.5);
        ctx.show_text("Hide Dock").ok();
        draw_toggle(&ctx, TOGGLE_X, DOCK_TOGGLE_Y, self.dock_hidden, theme);

        // Hide Menu Bar
        ctx.move_to(20.0, MENUBAR_TOGGLE_Y + 16.5);
        ctx.show_text("Hide Menu Bar").ok();
        draw_toggle(&ctx, TOGGLE_X, MENUBAR_TOGGLE_Y, self.menubar_hidden, theme);

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

fn draw_segment_label(ctx: &Context, label: &str, center_x: f64, control_y: f64, selected: bool, theme: &MacTheme) {
    let color = if selected { theme.text_primary } else { theme.text_secondary };
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
    let ext = ctx.text_extents(label).unwrap();
    let baseline = control_y + CONTROL_H / 2.0 + ext.height() / 2.0 - 1.0;
    ctx.move_to(center_x - ext.width() / 2.0, baseline);
    ctx.show_text(label).ok();
}

fn draw_toggle(ctx: &Context, x: f64, y: f64, on: bool, theme: &MacTheme) {
    let r = TOGGLE_H / 2.0;
    round_rect(ctx, x, y, TOGGLE_W, TOGGLE_H, r);
    let color = if on { theme.accent } else { theme.toggle_off };
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
    ctx.fill().ok();

    let knob_x = if on { x + TOGGLE_W - r } else { x + r };
    ctx.new_path();
    ctx.arc(knob_x, y + r, 9.5, 0.0, 2.0 * std::f64::consts::PI);
    ctx.set_source_rgba(theme.knob.0, theme.knob.1, theme.knob.2, theme.knob.3);
    ctx.fill().ok();
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
