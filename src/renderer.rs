use cairo::{Context, Format, ImageSurface};
use crate::theme::MacTheme;
use crate::app::{AppManager, DockItemType};

pub struct Renderer {
    width: i32,
    height: i32,
    stride: i32,
    zoom_headroom: i32,
}

impl Renderer {
    pub fn new(width: i32, height: i32) -> Self {
        let surf = ImageSurface::create(Format::ARgb32, width, height)
            .expect("Failed to create cairo surface");
        let stride = surf.stride();
        Self { width, height, stride, zoom_headroom: 0 }
    }

    pub fn resize(&mut self, width: i32, height: i32, zoom_headroom: i32) {
        let surf = ImageSurface::create(Format::ARgb32, width, height)
            .expect("Failed to resize surface");
        self.stride = surf.stride();
        self.width = width;
        self.height = height;
        self.zoom_headroom = zoom_headroom;
    }

    pub fn render(&mut self, theme: &MacTheme, manager: &mut AppManager) -> (Vec<u8>, i32) {
        let mut surf = ImageSurface::create(Format::ARgb32, self.width, self.height)
            .expect("Failed to create render surface");
        let ctx = Context::new(&surf).expect("Failed to create cairo context");

        // Start fully transparent — compositor sees through and applies blur
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        self.draw_background(&ctx, theme);
        self.draw_icons(&ctx, theme, manager);

        drop(ctx);
        surf.flush();
        let data = surf.data().expect("Failed to get surface data");
        (data.to_vec(), self.stride)
    }

    // -----------------------------------------------------------------------
    // Liquid-glass pill background
    // -----------------------------------------------------------------------
    fn draw_background(&self, ctx: &Context, theme: &MacTheme) {
        ctx.save().ok();

        let y0   = self.zoom_headroom as f64;
        let w    = self.width as f64;
        let bg_h = (self.height - self.zoom_headroom) as f64;
        let r    = theme.corner_radius;

        // Reusable closure: traces the pill path
        let pill = |ctx: &Context| {
            ctx.new_path();
            ctx.move_to(r, y0);
            ctx.line_to(w - r, y0);
            ctx.arc(w - r, y0 + r,        r,  -std::f64::consts::FRAC_PI_2, 0.0);
            ctx.line_to(w, y0 + bg_h - r);
            ctx.arc(w - r, y0 + bg_h - r, r,   0.0, std::f64::consts::FRAC_PI_2);
            ctx.line_to(r, y0 + bg_h);
            ctx.arc(r,     y0 + bg_h - r, r,   std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
            ctx.line_to(0.0, y0 + r);
            ctx.arc(r,     y0 + r,        r,   std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
            ctx.close_path();
        };

        // 1. ── Glass body fill ─────────────────────────────────────────────
        //    Pure white, near-zero opacity. The compositor blur does the heavy
        //    lifting; this just gives the pill a whisper of white so it reads
        //    against any wallpaper when blur is enabled.
        let body = cairo::LinearGradient::new(0.0, y0, 0.0, y0 + bg_h);
        let (r1, g1, b1, a1) = theme.bg_top;
        let (r2, g2, b2, a2) = theme.bg_bottom;
        body.add_color_stop_rgba(0.0,  r1, g1, b1, a1);
        body.add_color_stop_rgba(1.0,  r2, g2, b2, a2);
        pill(ctx);
        ctx.set_source(&body).ok();
        ctx.fill().ok();

        // 2. ── Top specular gloss ──────────────────────────────────────────
        //    A bright white highlight on the upper ~30% of the pill height.
        //    This is the key "liquid glass" cue — it mimics refracted light
        //    bending over the curved top surface of thick glass.
        let gloss_h = bg_h * 0.30;
        let gloss = cairo::LinearGradient::new(0.0, y0, 0.0, y0 + gloss_h);
        let (gr1, gg1, gb1, ga1) = theme.gloss_top;
        let (gr2, gg2, gb2, ga2) = theme.gloss_bottom;
        gloss.add_color_stop_rgba(0.0,  gr1, gg1, gb1, ga1);
        gloss.add_color_stop_rgba(0.55, gr1, gg1, gb1, ga1 * 0.15);
        gloss.add_color_stop_rgba(1.0,  gr2, gg2, gb2, ga2);

        // Clip the gloss to just the top portion of the pill
        pill(ctx);
        ctx.clip();
        ctx.rectangle(0.0, y0, w, gloss_h);
        ctx.set_source(&gloss).ok();
        ctx.fill().ok();
        ctx.reset_clip();

        // 3. ── Outer stroke ────────────────────────────────────────────────
        //    Single-pixel white border traces the entire pill outline.
        let (sr, sg, sb, sa) = theme.stroke_outer;
        ctx.set_source_rgba(sr, sg, sb, sa);
        ctx.set_line_width(1.0);
        pill(ctx);
        ctx.stroke().ok();

        // 4. ── Inner bottom-edge stroke ────────────────────────────────────
        //    A very subtle inset line on the lower half gives the illusion of
        //    glass thickness / depth (the bottom face of the glass catching
        //    reflected light differently from the top).
        //    We draw it 1px inside the outer stroke on the bottom arc only.
        {
            let inset = 1.5_f64;
            let (ir, ig, ib, ia) = theme.stroke_inner;
            ctx.set_source_rgba(ir, ig, ib, ia);
            ctx.set_line_width(1.0);
            ctx.new_path();
            // Bottom arc only: from left corner to right corner of the lower half
            ctx.arc(w - r, y0 + bg_h - r, r - inset,
                    0.0, std::f64::consts::FRAC_PI_2);
            ctx.line_to(r, y0 + bg_h - inset);
            ctx.arc(r, y0 + bg_h - r, r - inset,
                    std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
            ctx.stroke().ok();
        }

        ctx.restore().ok();
    }

    // -----------------------------------------------------------------------
    // Icons + running-app dots
    // -----------------------------------------------------------------------
    fn draw_icons(&self, ctx: &Context, theme: &MacTheme, manager: &mut AppManager) {
        let icon_count = manager.icons.len();
        if icon_count == 0 { return; }

        let icon_size = theme.icon_size as f64;
        let max_zoom  = theme.max_zoom;
        let edge_gap  = (theme.icon_spacing - theme.icon_size) as f64;
        let sep_width = 16.0_f64;
        let headroom  = self.zoom_headroom as f64;
        let padding_x = theme.padding_x;

        // Total content width
        let mut content_width = -edge_gap;
        for icon in &manager.icons {
            if icon.item_type == DockItemType::Separator {
                content_width += sep_width + edge_gap;
            } else {
                content_width += icon_size * icon.zoom + edge_gap;
            }
        }

        // Centre within dock, honouring horizontal padding
        let start_x = padding_x
            + (self.width as f64 - padding_x * 2.0 - content_width) / 2.0;
        let padding_top = theme.padding_top();

        manager.set_icon_positions(start_x, icon_size, edge_gap, sep_width);

        // Load at hi-res for sharp rendering at all zoom levels
        let hi_res = (icon_size * max_zoom).ceil() as i32;
        let mut surfaces = Vec::new();
        for i in 0..icon_count {
            surfaces.push(manager.load_icon_surface(i, hi_res));
        }

        for (i, icon) in manager.icons.iter().enumerate() {
            // ── Separator ──
            if icon.item_type == DockItemType::Separator {
                let cx = icon.x;
                let cy = headroom + padding_top + icon_size / 2.0;
                let sep_h = icon_size * 0.65;
                let (sr, sg, sb, sa) = theme.separator_color;
                ctx.set_source_rgba(sr, sg, sb, sa);
                ctx.set_line_width(theme.separator_width);
                ctx.new_path();
                ctx.move_to(cx, cy - sep_h / 2.0);
                ctx.line_to(cx, cy + sep_h / 2.0);
                ctx.stroke().ok();
                continue;
            }

            // ── Icon ──
            let zoom    = icon.zoom;
            let cx      = icon.x;
            // floor_y = bottom of icon slot at zoom 1
            let floor_y = headroom + padding_top + icon_size;

            if let Some(ref surf) = surfaces[i] {
                let (sw, sh) = (surf.width() as f64, surf.height() as f64);
                ctx.save().ok();

                // Icons grow upward from the floor (bottom-anchored zoom)
                let lift = (zoom - 1.0) * icon_size * 0.5;
                ctx.translate(cx, floor_y - lift);
                ctx.scale(zoom / max_zoom, zoom / max_zoom);

                ctx.set_source_surface(surf, -sw / 2.0, -sh).ok();
                ctx.rectangle(-sw / 2.0, -sh, sw, sh);
                ctx.fill().ok();

                ctx.restore().ok();
            }

            // ── Running-app dot ──
            if icon.is_running {
                let (dr, dg, db, da) = theme.active_dot_color;
                ctx.set_source_rgba(dr, dg, db, da);
                ctx.new_path();
                ctx.arc(cx, floor_y + 4.0, 2.5, 0.0, 2.0 * std::f64::consts::PI);
                ctx.fill().ok();
            }
        }
    }
}