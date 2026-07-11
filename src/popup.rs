use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use cairo::{Context, Format, ImageSurface};

const POPUP_W: u16 = 280;
const POPUP_H: u16 = 60;
const SLIDER_MARGIN: f64 = 24.0;
const KNOB_RADIUS: f64 = 10.0;
const MIN_ICON_SIZE: f64 = 32.0;
const MAX_ICON_SIZE: f64 = 80.0;

pub struct ResizerPopup {
    pub window: Window,
    pub visible: bool,
    pub icon_size: f64,
    depth: u8,
    dragging: bool,
}

impl ResizerPopup {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        visual: Visualid,
        depth: u8,
        colormap: Colormap,
        initial_icon_size: f64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let win = conn.generate_id()?;

        conn.create_window(
            depth,
            win,
            root,
            0, 0,
            POPUP_W, POPUP_H,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new()
                .background_pixel(0x00000001)
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

        // Set window type to POPUP_MENU so it floats above the dock
        let wm_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
        let popup_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_POPUP_MENU")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[popup_type])?;

        // Keep above
        let state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, state, AtomEnum::ATOM, &[above])?;

        conn.flush()?;

        Ok(Self {
            window: win,
            visible: false,
            icon_size: initial_icon_size,
            depth,
            dragging: false,
        })
    }

    pub fn show(&mut self, conn: &RustConnection, x: i16, y: i16) -> Result<(), Box<dyn std::error::Error>> {
        // Position the popup above the click point
        let px = (x - POPUP_W as i16 / 2).max(0);
        let py = y - POPUP_H as i16 - 12;
        conn.configure_window(self.window, &ConfigureWindowAux::new()
            .x(px as i32)
            .y(py as i32)
            .stack_mode(StackMode::ABOVE)
        )?;
        conn.map_window(self.window)?;
        conn.flush()?;
        self.visible = true;
        Ok(())
    }

    pub fn hide(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        conn.unmap_window(self.window)?;
        conn.flush()?;
        self.visible = false;
        self.dragging = false;
        Ok(())
    }

    fn value_to_x(&self, value: f64) -> f64 {
        let track_w = POPUP_W as f64 - SLIDER_MARGIN * 2.0;
        let t = (value - MIN_ICON_SIZE) / (MAX_ICON_SIZE - MIN_ICON_SIZE);
        SLIDER_MARGIN + t * track_w
    }

    fn x_to_value(&self, x: f64) -> f64 {
        let track_w = POPUP_W as f64 - SLIDER_MARGIN * 2.0;
        let t = ((x - SLIDER_MARGIN) / track_w).clamp(0.0, 1.0);
        MIN_ICON_SIZE + t * (MAX_ICON_SIZE - MIN_ICON_SIZE)
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::ButtonPress(ev) if ev.event == self.window => {
                self.dragging = true;
                self.icon_size = self.x_to_value(ev.event_x as f64);
                true
            }
            Event::ButtonRelease(ev) if ev.event == self.window => {
                self.dragging = false;
                true
            }
            Event::MotionNotify(ev) if ev.event == self.window && self.dragging => {
                self.icon_size = self.x_to_value(ev.event_x as f64);
                true
            }
            Event::LeaveNotify(ev) if ev.event == self.window => {
                if !self.dragging {
                    // Signal to hide
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    pub fn should_hide(&self, event: &Event) -> bool {
        matches!(event, Event::LeaveNotify(ev) if ev.event == self.window && !self.dragging)
    }

    pub fn render(&self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        let w = POPUP_W as i32;
        let h = POPUP_H as i32;
        let mut surface = ImageSurface::create(Format::ARgb32, w, h)
            .map_err(|e| format!("Cairo surface: {}", e))?;
        let ctx = Context::new(&surface).map_err(|e| format!("Cairo ctx: {}", e))?;

        // Clear
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        // Background: rounded rect
        let r = 16.0;
        let wf = w as f64;
        let hf = h as f64;

        ctx.new_path();
        ctx.move_to(r, 0.5);
        ctx.line_to(wf - r, 0.5);
        ctx.arc(wf - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        ctx.line_to(wf - 0.5, hf - r);
        ctx.arc(wf - r, hf - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        ctx.line_to(r, hf - 0.5);
        ctx.arc(r, hf - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        ctx.line_to(0.5, r);
        ctx.arc(r, r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        ctx.close_path();

        // Fill dark frosted background
        ctx.set_source_rgba(0.12, 0.12, 0.14, 0.92);
        ctx.fill_preserve().ok();

        // Border
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.2);
        ctx.set_line_width(1.0);
        ctx.stroke().ok();

        // Labels
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.5);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(11.0);
        ctx.move_to(SLIDER_MARGIN - 6.0, 16.0);
        ctx.show_text("Small").ok();
        let ext = ctx.text_extents("Large").unwrap();
        ctx.move_to(wf - SLIDER_MARGIN - ext.width() + 6.0, 16.0);
        ctx.show_text("Large").ok();

        // Slider track
        let track_y = hf / 2.0 + 4.0;
        let track_x1 = SLIDER_MARGIN;
        let track_x2 = wf - SLIDER_MARGIN;

        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
        ctx.set_line_width(4.0);
        ctx.set_line_cap(cairo::LineCap::Round);
        ctx.move_to(track_x1, track_y);
        ctx.line_to(track_x2, track_y);
        ctx.stroke().ok();

        // Active track (filled portion)
        let knob_x = self.value_to_x(self.icon_size);
        ctx.set_source_rgba(0.3, 0.6, 1.0, 0.9);
        ctx.set_line_width(4.0);
        ctx.move_to(track_x1, track_y);
        ctx.line_to(knob_x, track_y);
        ctx.stroke().ok();

        // Knob
        ctx.new_path();
        ctx.arc(knob_x, track_y, KNOB_RADIUS, 0.0, 2.0 * std::f64::consts::PI);
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.fill_preserve().ok();
        ctx.set_source_rgba(0.3, 0.6, 1.0, 0.6);
        ctx.set_line_width(2.0);
        ctx.stroke().ok();

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
                if bpp == 4 {
                    packed.push(data[i + 3]);
                }
            }
        }

        conn.put_image(
            ImageFormat::Z_PIXMAP,
            self.window,
            gc,
            POPUP_W, POPUP_H,
            0, 0,
            0, self.depth,
            &packed,
        )?;
        conn.free_gc(gc)?;
        conn.flush()?;

        Ok(())
    }
}
