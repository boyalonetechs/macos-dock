use std::collections::HashMap;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::errors::{ReplyError, ConnectionError};

pub struct DockWindow {
    pub conn: RustConnection,
    pub root: Window,
    pub window: Window,
    pub width: u16,
    pub height: u16,
    pub cursor_x: f64,
    pub screen_h: u16,
    pub screen_w: u16,
    depth: u8,
    atoms: HashMap<String, Atom>,
}

fn find_argb_visual(conn: &RustConnection, screen: usize) -> Option<(Visualid, u8)> {
    let info = &conn.setup().roots[screen];
    for depth in &info.allowed_depths {
        if depth.depth == 32 {
            for visual in &depth.visuals {
                if visual.red_mask == 0xff0000 && visual.green_mask == 0xff00 && visual.blue_mask == 0xff {
                    return Some((visual.visual_id, 32));
                }
            }
        }
    }
    None
}

impl DockWindow {
    pub fn new(dock_height: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let (conn, screen_idx) = x11rb::connect(None)?;
        let screen_info = &conn.setup().roots[screen_idx];
        let root = screen_info.root;
        let display_w = screen_info.width_in_pixels;
        let display_h = screen_info.height_in_pixels;

        let win = conn.generate_id()?;

        let (visual, depth) = find_argb_visual(&conn, screen_idx).unwrap_or((screen_info.root_visual, screen_info.root_depth));

        let colormap = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual)?;

        conn.create_window(
            depth,
            win,
            root,
            0, (display_h - dock_height) as i16,
            display_w, dock_height,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new()
                .background_pixel(0x00000001)
                .border_pixel(0)
                .colormap(colormap)
                .event_mask(
                    EventMask::EXPOSURE
                    | EventMask::STRUCTURE_NOTIFY
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::ENTER_WINDOW
                    | EventMask::LEAVE_WINDOW,
                ),
        )?;

        let mut atoms = HashMap::new();
        for name in &[
            "_NET_WM_WINDOW_TYPE",
            "_NET_WM_WINDOW_TYPE_DOCK",
            "_NET_WM_STRUT_PARTIAL",
            "_KDE_NET_WM_BLUR_BEHIND_REGION",
            "_NET_WM_DESKTOP",
            "_NET_WM_STATE",
            "_NET_WM_STATE_STICKY",
            "_NET_WM_STATE_ABOVE",
            "_NET_CLIENT_LIST",
            "_NET_ACTIVE_WINDOW",
            "WM_CLASS",
        ] {
            let a = conn.intern_atom(false, name.as_bytes())?.reply()?.atom;
            atoms.insert(name.to_string(), a);
        }

        let wm_type = *atoms.get("_NET_WM_WINDOW_TYPE").unwrap();
        let dock_type = *atoms.get("_NET_WM_WINDOW_TYPE_DOCK").unwrap();
        conn.change_property32(
            PropMode::REPLACE, win, wm_type, AtomEnum::ATOM, &[dock_type],
        )?;

        let desktop = *atoms.get("_NET_WM_DESKTOP").unwrap();
        conn.change_property32(
            PropMode::REPLACE, win, desktop, AtomEnum::CARDINAL, &[0xFFFFFFFF],
        )?;

        let state = *atoms.get("_NET_WM_STATE").unwrap();
        let atom_t = AtomEnum::ATOM;
        let sticky = *atoms.get("_NET_WM_STATE_STICKY").unwrap();
        let above = *atoms.get("_NET_WM_STATE_ABOVE").unwrap();
        conn.change_property32(
            PropMode::REPLACE, win, state, atom_t, &[sticky, above],
        )?;

        let strut_atom = *atoms.get("_NET_WM_STRUT_PARTIAL").unwrap();
        let w = display_w as u32;
        let h = dock_height as u32;
        let strut_data: [u32; 12] = [0, 0, 0, h, 0, 0, 0, 0, 0, 0, 0, w];
        conn.change_property32(
            PropMode::REPLACE, win, strut_atom, AtomEnum::CARDINAL, &strut_data,
        )?;

        conn.map_window(win)?;
        conn.flush()?;

        let blur_atom = *atoms.get("_KDE_NET_WM_BLUR_BEHIND_REGION").unwrap();
        conn.change_property32(
            PropMode::REPLACE, win, blur_atom, AtomEnum::CARDINAL, &[0],
        )?;
        conn.flush()?;

        Ok(Self {
            conn,
            root,
            window: win,
            width: display_w,
            height: dock_height,
            cursor_x: display_w as f64 / 2.0,
            screen_h: display_h,
            screen_w: display_w,
            depth,
            atoms,
        })
    }

    pub fn set_title(&self, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.change_property8(
            PropMode::REPLACE,
            self.window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;
        let utf8 = self.conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
        let net_name = self.conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        self.conn.change_property8(
            PropMode::REPLACE,
            self.window,
            net_name,
            utf8,
            title.as_bytes(),
        )?;
        self.conn.flush()?;
        Ok(())
    }

    pub fn configure(&self, x: i16, y: i16, w: u16, h: u16) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.configure_window(self.window, &ConfigureWindowAux::new()
            .x(x as i32)
            .y(y as i32)
            .width(w as u32)
            .height(h as u32)
        )?;
        self.conn.flush()?;
        Ok(())
    }

    pub fn push_pixels(&self, data: &[u8], width: u16, height: u16, stride: i32) -> Result<(), Box<dyn std::error::Error>> {
        let gc = self.conn.generate_id()?;
        self.conn.create_gc(gc, self.window, &CreateGCAux::new())?;

        let w = width as usize;
        let h = height as usize;
        let stride = stride as usize;
        let bpp = if self.depth == 32 { 4 } else { 3 };

        let mut packed = Vec::with_capacity(w * h * bpp);
        for y in 0..h {
            let row_start = y * stride;
            for x in 0..w {
                let i = row_start + x * 4;
                packed.push(data[i]);     // B
                packed.push(data[i + 1]); // G
                packed.push(data[i + 2]); // R
                if bpp == 4 {
                    packed.push(data[i + 3]); // A
                }
            }
        }

        self.conn.put_image(
            ImageFormat::Z_PIXMAP,
            self.window,
            gc,
            width, height,
            0, 0,
            0, self.depth,
            &packed,
        )?;
        self.conn.free_gc(gc)?;
        self.conn.flush()?;
        Ok(())
    }

    pub fn get_running_windows(&self) -> Result<Vec<u32>, ReplyError> {
        let net_cl = self.atoms.get("_NET_CLIENT_LIST");
        if let Some(atom) = net_cl {
            let r = self.conn.get_property(false, self.root, *atom, AtomEnum::WINDOW, 0, 2048)?.reply()?;
            if r.format == 32 {
                let len = r.length as usize;
                let mut windows = Vec::with_capacity(len);
                for i in 0..len {
                    let offset = i * 4;
                    if offset + 4 <= r.value.len() {
                        let slice: [u8; 4] = r.value[offset..offset+4].try_into().unwrap_or([0; 4]);
                        windows.push(u32::from_ne_bytes(slice));
                    }
                }
                return Ok(windows);
            }
        }
        Ok(Vec::new())
    }

    pub fn get_window_class(&self, wid: u32) -> Option<String> {
        let atom = self.atoms.get("WM_CLASS")?;
        let r = self.conn.get_property(false, wid, *atom, AtomEnum::STRING, 0, 256).ok()?.reply().ok()?;
        if r.format == 8 {
            let s = String::from_utf8_lossy(&r.value);
            let parts: Vec<&str> = s.split('\0').collect();
            parts.get(1).or_else(|| parts.get(0)).map(|s| s.to_string())
        } else {
            None
        }
    }

    pub fn next_event(&self) -> Result<Option<Event>, ConnectionError> {
        self.conn.poll_for_event()
    }
}
