use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use cairo::{Context, Format, ImageSurface};
use crate::desktop::{DesktopEntry, IconCache};

const ICON_SIZE: i32 = 64;
const CELL_W: i32 = 120;
const CELL_H: i32 = 110;
const COLS: i32 = 8;
const MARGIN_TOP: i32 = 80;
const PADDING: i32 = 40;

pub struct AppGrid {
    pub window: Window,
    pub visible: bool,
    depth: u8,
    screen_w: u16,
    screen_h: u16,
    grid_w: i32,
    grid_h: i32,
    entries: Vec<DesktopEntry>,
    icon_cache: IconCache,
    entry_positions: Vec<(i32, i32, i32, i32)>,
}

impl AppGrid {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        visual: Visualid,
        depth: u8,
        colormap: Colormap,
        screen_w: u16,
        screen_h: u16,
        entries: Vec<DesktopEntry>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let cols = COLS;
        let count = entries.len() as i32;
        let rows = (count + cols - 1) / cols;        let grid_w = (cols * CELL_W + PADDING * 2) as u16;
        let grid_h = (rows * CELL_H + MARGIN_TOP + PADDING) as u16;

        let win = conn.generate_id()?;
        conn.create_window(
            depth,
            win,
            root,
            0, 0,
            grid_w,
            grid_h,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
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
                    | EventMask::KEY_PRESS,
                ),
        )?;

        let wm_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
        let dialog_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DIALOG")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[dialog_type])?;

        let state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
        let focused = conn.intern_atom(false, b"_NET_WM_STATE_FOCUSED")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, state, AtomEnum::ATOM, &[above, focused])?;

        let wm_name = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let utf8 = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
        conn.change_property8(PropMode::REPLACE, win, wm_name, utf8, b"Application Launcher")?;
        conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, b"Application Launcher")?;

        conn.flush()?;

        let mut positions = Vec::new();
        for i in 0..entries.len() {
            let col = (i as i32) % cols;
            let row = (i as i32) / cols;
            let x = PADDING + col * CELL_W;
            let y = MARGIN_TOP + row * CELL_H;
            positions.push((x, y, CELL_W, CELL_H));
        }

        Ok(Self {
            window: win,
            visible: false,
            depth,
            screen_w,
            screen_h,
            grid_w: grid_w as i32,
            grid_h: grid_h as i32,
            entries,
            icon_cache: IconCache::new(),
            entry_positions: positions,
        })
    }

    pub fn toggle(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        if self.visible {
            self.hide(conn)
        } else {
            self.show(conn)
        }
    }

    pub fn show(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        let x = (self.screen_w as i32 - self.grid_w) / 2;
        let y = (self.screen_h as i32 - self.grid_h) / 2;
        conn.configure_window(self.window, &ConfigureWindowAux::new()
            .x(x as i32)
            .y(y as i32)
            .stack_mode(StackMode::ABOVE)
        )?;
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

    pub fn handle_event(&mut self, event: &Event) -> Option<String> {
        match event {
            Event::ButtonPress(ev) if ev.event == self.window => {
                let click_x = ev.event_x as i32;
                let click_y = ev.event_y as i32;
                for (i, &(x, y, w, h)) in self.entry_positions.iter().enumerate() {
                    if click_x >= x && click_x < x + w && click_y >= y && click_y < y + h {
                        if let Some(entry) = self.entries.get(i) {
                            return Some(entry.file_path.to_string_lossy().to_string());
                        }
                    }
                }
                None
            }
            Event::KeyPress(ev) if ev.event == self.window => {
                // Escape to close
                None
            }
            _ => None,
        }
    }

    pub fn should_hide(&self, event: &Event) -> bool {
        if !self.visible { return false; }
        match event {
            Event::ButtonPress(ev) if ev.event != self.window => true,
            Event::KeyPress(ev) if ev.event == self.window => {
                // Check for Escape (keycode 9 on most X11 systems)
                ev.detail == 9
            }
            _ => false,
        }
    }

    pub fn render(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        let w = self.grid_w;
        let h = self.grid_h;
        let mut surface = ImageSurface::create(Format::ARgb32, w, h)
            .map_err(|e| format!("Cairo surface: {}", e))?;
        let ctx = Context::new(&surface).map_err(|e| format!("Cairo ctx: {}", e))?;

        // Clear
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        // Frosted dark background
        let wf = w as f64;
        let hf = h as f64;
        let r = 20.0_f64;
        ctx.new_path();
        ctx.move_to(r, 0.0);
        ctx.line_to(wf - r, 0.0);
        ctx.arc(wf - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        ctx.line_to(wf, hf - r);
        ctx.arc(wf - r, hf - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        ctx.line_to(r, hf);
        ctx.arc(r, hf - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        ctx.line_to(0.0, r);
        ctx.arc(r, r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        ctx.close_path();
        ctx.set_source_rgba(0.08, 0.08, 0.10, 0.88);
        ctx.fill_preserve().ok();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
        ctx.set_line_width(1.0);
        ctx.stroke().ok();

        // Title
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7);
        ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(18.0);
        ctx.move_to(PADDING as f64, (MARGIN_TOP - 20) as f64);
        ctx.show_text("Applications").ok();

        // Draw each icon
        for (i, entry) in self.entries.iter().enumerate() {
            if let Some(&(x, y, _w, _h)) = self.entry_positions.get(i) {
                let cx = x as f64 + CELL_W as f64 / 2.0;
                let cy = y as f64 + 8.0;

                // Load and draw icon
                if let Some(surf) = self.icon_cache.get_or_load(&entry.icon_name, ICON_SIZE) {
                    let sw = surf.width() as f64;
                    let sh = surf.height() as f64;
                    ctx.save().ok();
                    ctx.set_source_surface(&surf, cx - sw / 2.0, cy).ok();
                    ctx.rectangle(cx - sw / 2.0, cy, sw, sh);
                    ctx.fill().ok();
                    ctx.restore().ok();
                } else {
                    // Placeholder circle
                    ctx.new_path();
                    ctx.arc(cx, cy + ICON_SIZE as f64 / 2.0, ICON_SIZE as f64 / 2.0, 0.0, 2.0 * std::f64::consts::PI);
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.1);
                    ctx.fill().ok();
                }

                // Label
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
                ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                ctx.set_font_size(11.0);

                let name = &entry.name;
                let max_w = (CELL_W - 12) as f64;
                let ext = ctx.text_extents(name).unwrap();
                let mut display_name = name.clone();
                if ext.width() > max_w {
                    while !display_name.is_empty() {
                        display_name.pop();
                        let test = format!("{}…", display_name);
                        if ctx.text_extents(&test).map(|e| e.width()).unwrap_or(0.0) <= max_w {
                            display_name = test;
                            break;
                        }
                    }
                }
                let text_ext = ctx.text_extents(&display_name).unwrap();
                ctx.move_to(cx - text_ext.width() / 2.0, cy + ICON_SIZE as f64 + 16.0);
                ctx.show_text(&display_name).ok();
            }
        }

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
            w as u16, h as u16,
            0, 0,
            0, self.depth,
            &packed,
        )?;
        conn.free_gc(gc)?;
        conn.flush()?;

        Ok(())
    }
}
