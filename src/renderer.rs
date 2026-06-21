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
        let m = theme.margin;
        let w = self.width as f64;
        let h = self.height as f64;

        ctx.new_path();
        ctx.move_to(m + r, m);
        ctx.line_to(w - m - r, m);
        ctx.arc(w - m - r, m + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        ctx.line_to(w - m, h - m - r);
        ctx.arc(w - m - r, h - m - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        ctx.line_to(m + r, h - m);
        ctx.arc(m + r, h - m - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        ctx.line_to(m, m + r);
        ctx.arc(m + r, m + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
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
        ctx.move_to(m + r, m);
        ctx.line_to(w - m - r, m);
        ctx.arc(w - m - r, m + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        ctx.line_to(w - m, h - m - r);
        ctx.arc(w - m - r, h - m - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        ctx.line_to(m + r, h - m);
        ctx.arc(m + r, h - m - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        ctx.line_to(m, m + r);
        ctx.arc(m + r, m + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        ctx.stroke().ok();

        let (sr, sg, sb, sa) = theme.stroke_inner;
        ctx.set_source_rgba(sr, sg, sb, sa);
        ctx.set_line_width(0.5);
        ctx.new_path();
        ctx.move_to(m + r, m + 1.0);
        ctx.line_to(w - m - r, m + 1.0);
        ctx.stroke().ok();
    }

    fn draw_icons(&self, ctx: &Context, theme: &MacTheme, manager: &mut AppManager) {
        let icon_count = manager.icons.len();
        if icon_count == 0 { return; }

        let icon_size = theme.icon_size as f64;
        let spacing = theme.icon_spacing as f64;

        let mut actual_width = 0.0;
        for icon in &manager.icons {
            if icon.item_type == DockItemType::Separator {
                actual_width += spacing * 0.5;
            } else {
                actual_width += spacing;
            }
        }
        
        let start_x = (self.width as f64 - actual_width) / 2.0;
        let padding_top = theme.padding_top();

        manager.set_icon_positions(start_x, spacing, icon_size);

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
            let display_size = (icon_size * zoom).round() as i32;
            if display_size < 4 { continue; }

            let cx = icon.x;
            let cy = padding_top + icon_size / 2.0;
            let draw_x = (cx - display_size as f64 / 2.0).round();
            let draw_y = (cy - display_size as f64 / 2.0).round();

            if let Some(ref surf) = surfaces[i] {
                let (sw, sh) = (surf.width(), surf.height());
                ctx.save().ok();

                if (display_size as f64 / icon_size - 1.0).abs() > 0.01 {
                    ctx.translate(cx, cy);
                    ctx.scale(zoom, zoom);
                    ctx.translate(-cx, -cy);
                }

                ctx.set_source_surface(surf, draw_x, draw_y).ok();
                ctx.rectangle(draw_x.max(0.0), draw_y.max(0.0), sw as f64, sh as f64);
                ctx.fill().ok();

                ctx.restore().ok();
            }

            if icon.is_running {
                let dot_y = padding_top + icon_size + 4.0;
                let (dr, dg, db, da) = theme.active_dot_color;
                ctx.set_source_rgba(dr, dg, db, da);
                ctx.new_path();
                ctx.arc(cx, dot_y, 3.0, 0.0, 2.0 * std::f64::consts::PI);
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
