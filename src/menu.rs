use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use cairo::{Context, Format, ImageSurface};

const MENU_W: i32 = 210;
const ITEM_H: i32 = 32;
const SEP_H: i32 = 8;
const FONT_SIZE: f64 = 13.0;
const CORNER_R: f64 = 10.0;

#[derive(Clone, Copy, PartialEq)]
pub enum MenuAction {
    NewWindow,
    Settings,
    KeepInDock,
    Quit,
}

struct MenuItem {
    label: String,
    action: MenuAction,
    separator: bool,
}

pub struct ContextMenu {
    pub window: Window,
    pub visible: bool,
    pub icon_index: usize,
    depth: u8,
    items: Vec<MenuItem>,
    item_rects: Vec<(i32, i32, i32, i32)>,
    total_h: i32,
    hovered: Option<usize>,
    keep_in_dock: bool,
    dock_hidden: bool,
}

impl ContextMenu {
    pub fn new(
        conn: &RustConnection,
        root: Window,
        visual: Visualid,
        depth: u8,
        colormap: Colormap,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let win = conn.generate_id()?;
        conn.create_window(
            depth,
            win,
            root,
            0, 0,
            MENU_W as u16, 100,
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
                    | EventMask::LEAVE_WINDOW,
                ),
        )?;

        let wm_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
        let popup_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_POPUP_MENU")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[popup_type])?;

        let state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
        conn.change_property32(PropMode::REPLACE, win, state, AtomEnum::ATOM, &[above])?;
        conn.flush()?;

        Ok(Self {
            window: win,
            visible: false,
            icon_index: 0,
            depth,
            items: Vec::new(),
            item_rects: Vec::new(),
            total_h: 0,
            hovered: None,
            keep_in_dock: true,
            dock_hidden: false,
        })
    }

    pub fn show(
        &mut self,
        conn: &RustConnection,
        x: i16,
        y: i16,
        icon_index: usize,
        keep_in_dock: bool,
        dock_hidden: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.icon_index = icon_index;
        self.keep_in_dock = keep_in_dock;
        self.dock_hidden = dock_hidden;
        self.build_items();
        self.compute_layout();

        let px = (x as i32).max(0).min(conn.setup().roots[0].width_in_pixels as i32 - MENU_W);
        let py = (y as i32 - self.total_h).max(0);

        conn.configure_window(self.window, &ConfigureWindowAux::new()
            .x(px)
            .y(py)
            .width(MENU_W as u32)
            .height(self.total_h as u32)
            .stack_mode(StackMode::ABOVE)
        )?;
        conn.map_window(self.window)?;
        conn.flush()?;
        self.visible = true;
        self.hovered = None;
        self.render(conn)?;
        Ok(())
    }

    pub fn hide(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        conn.unmap_window(self.window)?;
        conn.flush()?;
        self.visible = false;
        Ok(())
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<(MenuAction, usize)> {
        match event {
            Event::MotionNotify(ev) if ev.event == self.window => {
                let my = ev.event_y as i32;
                let mx = ev.event_x as i32;
                let new_hovered = self.item_rects.iter().enumerate()
                    .find(|(_, rect)| {
                        mx >= 0 && mx <= rect.2 && my >= rect.1 && my < rect.1 + rect.3
                    })
                    .map(|(i, _)| i)
                    .filter(|&i| !self.items[i].separator);
                self.hovered = new_hovered;
                None
            }
            Event::ButtonPress(ev) if ev.event == self.window => {
                let my = ev.event_y as i32;
                let mx = ev.event_x as i32;
                for (i, &(_rx, ry, rw, rh)) in self.item_rects.iter().enumerate() {
                    if mx >= 0 && mx <= rw && my >= ry && my < ry + rh && !self.items[i].separator {
                        let action = self.items[i].action;
                        return Some((action, self.icon_index));
                    }
                }
                None
            }
            Event::LeaveNotify(ev) if ev.event == self.window => {
                self.hovered = None;
                None
            }
            _ => None,
        }
    }

    pub fn should_hide(&self, event: &Event) -> bool {
        if !self.visible { return false; }
        match event {
            Event::ButtonPress(ev) if ev.event != self.window => true,
            Event::MotionNotify(ev) if ev.event != self.window => false,
            _ => false,
        }
    }

    fn build_items(&mut self) {
        self.items.clear();
        self.items.push(MenuItem { label: "New Window".into(), action: MenuAction::NewWindow, separator: false });
        self.items.push(MenuItem { label: String::new(), action: MenuAction::NewWindow, separator: true });
        self.items.push(MenuItem { label: "Settings...".into(), action: MenuAction::Settings, separator: false });
        self.items.push(MenuItem {
            label: if self.keep_in_dock { "✓ Keep in Dock" } else { "  Keep in Dock" }.into(),
            action: MenuAction::KeepInDock,
            separator: false,
        });
        self.items.push(MenuItem { label: String::new(), action: MenuAction::NewWindow, separator: true });
        self.items.push(MenuItem { label: "Quit".into(), action: MenuAction::Quit, separator: false });
    }

    fn compute_layout(&mut self) {
        self.item_rects.clear();
        let mut y = 0i32;
        for item in &self.items {
            let h = if item.separator { SEP_H } else { ITEM_H };
            self.item_rects.push((0, y, MENU_W, h));
            y += h;
        }
        self.total_h = y;
    }

    pub fn render(&self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        let w = MENU_W;
        let h = self.total_h;
        let mut surface = ImageSurface::create(Format::ARgb32, w, h)
            .map_err(|e| format!("Cairo surface: {}", e))?;
        let ctx = Context::new(&surface).map_err(|e| format!("Cairo ctx: {}", e))?;

        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        // Background
        let wf = w as f64;
        let hf = h as f64;
        round_rect(&ctx, 0.0, 0.0, wf, hf, CORNER_R);
        ctx.set_source_rgba(0.12, 0.12, 0.14, 0.94);
        ctx.fill_preserve().ok();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.18);
        ctx.set_line_width(1.0);
        ctx.stroke().ok();

        // Draw items
        for (i, item) in self.items.iter().enumerate() {
            let (_rx, ry, _rw, rh) = self.item_rects[i];
            let ry_f = ry as f64;
            let rh_f = rh as f64;

            if item.separator {
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.12);
                ctx.set_line_width(1.0);
                ctx.move_to(12.0, ry_f + rh_f / 2.0);
                ctx.line_to(wf - 12.0, ry_f + rh_f / 2.0);
                ctx.stroke().ok();
                continue;
            }

            // Hover highlight
            if self.hovered == Some(i) {
                round_rect(&ctx, 4.0, ry_f + 2.0, wf - 8.0, rh_f - 4.0, 6.0);
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.12);
                ctx.fill().ok();
            }

            // Label
            let text_color = if item.action == MenuAction::Quit {
                (1.0, 0.4, 0.4, 0.95)
            } else {
                (1.0, 1.0, 1.0, 0.90)
            };
            ctx.set_source_rgba(text_color.0, text_color.1, text_color.2, text_color.3);
            ctx.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            ctx.set_font_size(FONT_SIZE);
            ctx.move_to(16.0, ry_f + rh_f / 2.0 + FONT_SIZE * 0.35);
            ctx.show_text(&item.label).ok();
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
                if bpp == 4 { packed.push(data[i + 3]); }
            }
        }

        conn.put_image(
            ImageFormat::Z_PIXMAP, self.window, gc,
            w as u16, h as u16, 0, 0, 0, self.depth, &packed,
        )?;
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
