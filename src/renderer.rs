use cairo::{Context, Format, ImageSurface};
use crate::theme::MacTheme;
use crate::app::{AppManager, DockItemType};

pub struct Renderer {
    surface: ImageSurface,
    width: i32,
    height: i32,
}

impl Renderer {
    pub fn new(width: i32, height: i32) -> Self {
        let surface = ImageSurface::create(Format::ARgb32, width, height)
            .expect("Failed to create cairo surface");
        Self { surface, width, height }
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        self.surface = ImageSurface::create(Format::ARgb32, width, height)
            .expect("Failed to resize surface");
        self.width = width;
        self.height = height;
    }

    pub fn render(&mut self, theme: &MacTheme, manager: &mut AppManager) {
        let ctx = Context::new(&self.surface).expect("Failed to create cairo context");

        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        self.draw_background(&ctx, theme);
        self.draw_icons(&ctx, theme, manager);
    }

    fn draw_background(&self, ctx: &Context, theme: &MacTheme) {
        let r = theme.corner_radius;
        let w = self.width as f64;
        let h = self.height as f64;

        ctx.new_path();
        ctx.move_to(r, 0.0);
        ctx.line_to(w - r, 0.0);
        ctx.arc(w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        ctx.line_to(w, h - r);
        ctx.arc(w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        ctx.line_to(r, h);
        ctx.arc(r, h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        ctx.line_to(0.0, r);
        ctx.arc(r, r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        ctx.close_path();

        let pat = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
        let (r1, g1, b1, a1) = theme.bg_top;
        let (r2, g2, b2, a2) = theme.bg_bottom;
        pat.add_color_stop_rgba(0.0, r1, g1, b1, a1);
        pat.add_color_stop_rgba(1.0, r2, g2, b2, a2);
        ctx.set_source(&pat).ok();
        ctx.fill().ok();

        let (sr, sg, sb, sa) = theme.stroke_outer;
        ctx.set_source_rgba(sr, sg, sb, sa);
        ctx.set_line_width(1.0);
        ctx.new_path();
        ctx.move_to(r, 0.0);
        ctx.line_to(w - r, 0.0);
        ctx.arc(w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        ctx.line_to(w, h - r);
        ctx.arc(w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        ctx.line_to(r, h);
        ctx.arc(r, h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        ctx.line_to(0.0, r);
        ctx.arc(r, r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        ctx.close_path();
        ctx.stroke().ok();
    }

    fn draw_icons(&self, ctx: &Context, theme: &MacTheme, manager: &mut AppManager) {
        let icon_count = manager.icons.len();
        if icon_count == 0 { return; }

        let icon_size = theme.icon_size as f64;
        let edge_gap = (theme.icon_spacing - theme.icon_size) as f64;
        let sep_width = 16.0;

        let mut total_width = -edge_gap;
        for icon in &manager.icons {
            if icon.item_type == DockItemType::Separator {
                total_width += sep_width + edge_gap;
            } else {
                total_width += icon_size * icon.zoom + edge_gap;
            }
        }

        let start_x = (self.width as f64 - total_width) / 2.0;
        let padding_top = theme.padding_top();

        manager.set_icon_positions(start_x, icon_size, edge_gap, sep_width);

        let mut surfaces = Vec::new();
        for i in 0..icon_count {
            surfaces.push(manager.load_icon_surface(i, icon_size as i32));
        }

        for (i, icon) in manager.icons.iter().enumerate() {
            if icon.item_type == DockItemType::Separator {
                let cx = icon.x;
                let cy = padding_top + icon_size / 2.0;
                let sep_height = icon_size * 0.7;
                let (sr, sg, sb, sa) = theme.separator_color;
                ctx.set_source_rgba(sr, sg, sb, sa);
                ctx.set_line_width(theme.separator_width);
                ctx.new_path();
                ctx.move_to(cx, cy - sep_height / 2.0);
                ctx.line_to(cx, cy + sep_height / 2.0);
                ctx.stroke().ok();
                continue;
            }

            let zoom = icon.zoom;
            let cx = icon.x;
            let floor_y = padding_top + icon_size;

            if let Some(ref surf) = surfaces[i] {
                let (sw, sh) = (surf.width() as f64, surf.height() as f64);
                ctx.save().ok();

                let lift = (zoom - 1.0) * icon_size * 0.3;
                let base_y = floor_y + lift;
                ctx.translate(cx, base_y);
                ctx.scale(zoom, zoom);
                ctx.set_source_surface(surf, -sw / 2.0, -sh).ok();
                ctx.rectangle(-sw / 2.0, -sh, sw, sh);
                ctx.fill().ok();

                ctx.restore().ok();
            }

            if icon.is_running {
                let (dr, dg, db, da) = theme.active_dot_color;
                ctx.set_source_rgba(dr, dg, db, da);
                ctx.new_path();
                ctx.arc(cx, floor_y + 5.0, 3.5, 0.0, 2.0 * std::f64::consts::PI);
                ctx.fill().ok();
            }
        }
    }

    pub fn copy_data(&mut self) -> Vec<u8> {
        self.surface.data()
            .expect("Failed to get surface data")
            .to_vec()
    }

    pub fn stride(&self) -> i32 {
        self.surface.stride()
    }
}
