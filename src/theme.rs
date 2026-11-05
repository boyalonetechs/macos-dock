pub type Color = (f64, f64, f64, f64);

const CLEAR: Color = (0.0, 0.0, 0.0, 0.0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

pub struct MacTheme {
    pub mode: ThemeMode,
    pub icon_size: i32,
    pub icon_spacing: i32,
    pub padding_x: f64,
    pub padding_y: f64,
    pub bottom_margin: f64,
    pub corner_radius: f64,

    // Liquid glass fill and the specular strip along the top rim
    pub bg_top: Color,
    pub bg_bottom: Color,
    pub gloss_top: Color,
    pub gloss_bottom: Color,

    // Thin outer rim plus the softer inset line along the bottom
    pub stroke_outer: Color,
    pub stroke_inner: Color,

    // Used until (or instead of) the blurred wallpaper being ready
    pub fallback_top: Color,
    pub fallback_bottom: Color,
    pub veil: Color,

    pub active_dot_color: Color,
    pub separator_color: Color,
    pub separator_width: f64,

    // Menus, settings and other floating panels
    pub panel_bg: Color,
    pub panel_border: Color,
    pub hover_bg: Color,
    pub control_bg: Color,
    pub control_selected: Color,
    pub bar_bg: Color,
    pub bar_border: Color,

    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,

    pub accent: Color,
    pub toggle_off: Color,
    pub knob: Color,
    pub danger: Color,

    pub max_zoom: f64,
    pub sigma: f64,       // icon magnification falloff width
    pub blur_sigma: f64,  // wallpaper blur radius
}

impl MacTheme {
    pub fn new(mode: ThemeMode) -> Self {
        let mut theme = Self {
            mode,
            icon_size: 60,
            icon_spacing: 68, // 60px icon + 8px gap
            padding_x: 14.0,
            padding_y: 9.0,
            bottom_margin: 6.0,
            corner_radius: 20.0,

            bg_top: CLEAR,
            bg_bottom: CLEAR,
            gloss_top: CLEAR,
            gloss_bottom: CLEAR,
            stroke_outer: CLEAR,
            stroke_inner: CLEAR,
            fallback_top: CLEAR,
            fallback_bottom: CLEAR,
            veil: CLEAR,
            active_dot_color: CLEAR,
            separator_color: CLEAR,
            separator_width: 1.0,
            panel_bg: CLEAR,
            panel_border: CLEAR,
            hover_bg: CLEAR,
            control_bg: CLEAR,
            control_selected: CLEAR,
            bar_bg: CLEAR,
            bar_border: CLEAR,
            text_primary: CLEAR,
            text_secondary: CLEAR,
            text_muted: CLEAR,
            accent: CLEAR,
            toggle_off: CLEAR,
            knob: CLEAR,
            danger: CLEAR,

            max_zoom: 1.32,
            sigma: 20.0,
            blur_sigma: 22.0,
        };
        theme.apply_mode(mode);
        theme
    }

    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.mode = mode;
        self.apply_mode(mode);
    }

    fn apply_mode(&mut self, mode: ThemeMode) {
        let p = match mode {
            ThemeMode::Light => Palette {
                bg_top: (1.0, 1.0, 1.0, 0.36),
                bg_bottom: (1.0, 1.0, 1.0, 0.20),
                gloss_top: (1.0, 1.0, 1.0, 0.24),
                gloss_bottom: (1.0, 1.0, 1.0, 0.0),
                stroke_outer: (1.0, 1.0, 1.0, 0.80),
                stroke_inner: (0.0, 0.0, 0.0, 0.10),
                fallback_top: (0.94, 0.94, 0.95, 0.92),
                fallback_bottom: (0.86, 0.86, 0.88, 0.96),
                veil: (1.0, 1.0, 1.0, 0.12),
                active_dot: (0.0, 0.0, 0.0, 0.65),
                separator: (0.0, 0.0, 0.0, 0.16),
                panel_bg: (0.98, 0.98, 0.99, 0.97),
                panel_border: (0.0, 0.0, 0.0, 0.12),
                hover_bg: (0.0, 0.0, 0.0, 0.06),
                control_bg: (0.0, 0.0, 0.0, 0.06),
                control_selected: (1.0, 1.0, 1.0, 0.96),
                bar_bg: (1.0, 1.0, 1.0, 0.10),
                bar_border: (0.0, 0.0, 0.0, 0.08),
                text_primary: (0.0, 0.0, 0.0, 0.90),
                text_secondary: (0.0, 0.0, 0.0, 0.72),
                text_muted: (0.0, 0.0, 0.0, 0.45),
                accent: (0.0, 0.48, 1.0, 0.95),
                toggle_off: (0.78, 0.78, 0.82, 0.95),
                knob: (1.0, 1.0, 1.0, 1.0),
                danger: (0.85, 0.16, 0.12, 0.95),
            },
            ThemeMode::Dark => Palette {
                bg_top: (1.0, 1.0, 1.0, 0.06),
                bg_bottom: (1.0, 1.0, 1.0, 0.10),
                gloss_top: (1.0, 1.0, 1.0, 0.05),
                gloss_bottom: (1.0, 1.0, 1.0, 0.0),
                stroke_outer: (1.0, 1.0, 1.0, 0.45),
                stroke_inner: (1.0, 1.0, 1.0, 0.18),
                fallback_top: (0.13, 0.13, 0.15, 0.80),
                fallback_bottom: (0.06, 0.06, 0.08, 0.90),
                veil: (0.02, 0.02, 0.04, 0.18),
                active_dot: (1.0, 1.0, 1.0, 0.90),
                separator: (1.0, 1.0, 1.0, 0.30),
                panel_bg: (0.14, 0.14, 0.16, 0.96),
                panel_border: (1.0, 1.0, 1.0, 0.16),
                hover_bg: (1.0, 1.0, 1.0, 0.10),
                control_bg: (1.0, 1.0, 1.0, 0.08),
                control_selected: (0.35, 0.35, 0.38, 0.96),
                bar_bg: (0.08, 0.08, 0.10, 0.08),
                bar_border: (1.0, 1.0, 1.0, 0.10),
                text_primary: (1.0, 1.0, 1.0, 0.95),
                text_secondary: (1.0, 1.0, 1.0, 0.80),
                text_muted: (1.0, 1.0, 1.0, 0.55),
                accent: (0.25, 0.55, 1.0, 0.95),
                toggle_off: (0.35, 0.35, 0.40, 0.85),
                knob: (1.0, 1.0, 1.0, 0.97),
                danger: (1.0, 0.42, 0.42, 0.95),
            },
        };

        self.bg_top = p.bg_top;
        self.bg_bottom = p.bg_bottom;
        self.gloss_top = p.gloss_top;
        self.gloss_bottom = p.gloss_bottom;
        self.stroke_outer = p.stroke_outer;
        self.stroke_inner = p.stroke_inner;
        self.fallback_top = p.fallback_top;
        self.fallback_bottom = p.fallback_bottom;
        self.veil = p.veil;
        self.active_dot_color = p.active_dot;
        self.separator_color = p.separator;
        self.panel_bg = p.panel_bg;
        self.panel_border = p.panel_border;
        self.hover_bg = p.hover_bg;
        self.control_bg = p.control_bg;
        self.control_selected = p.control_selected;
        self.bar_bg = p.bar_bg;
        self.bar_border = p.bar_border;
        self.text_primary = p.text_primary;
        self.text_secondary = p.text_secondary;
        self.text_muted = p.text_muted;
        self.accent = p.accent;
        self.toggle_off = p.toggle_off;
        self.knob = p.knob;
        self.danger = p.danger;
    }

    /// Total rendered dock height in pixels
    pub fn dock_height(&self) -> i32 {
        (self.icon_size as f64 + self.padding_y * 2.0).ceil() as i32
    }

    pub fn padding_top(&self) -> f64 {
        self.padding_y
    }
}

struct Palette {
    bg_top: Color,
    bg_bottom: Color,
    gloss_top: Color,
    gloss_bottom: Color,
    stroke_outer: Color,
    stroke_inner: Color,
    fallback_top: Color,
    fallback_bottom: Color,
    veil: Color,
    active_dot: Color,
    separator: Color,
    panel_bg: Color,
    panel_border: Color,
    hover_bg: Color,
    control_bg: Color,
    control_selected: Color,
    bar_bg: Color,
    bar_border: Color,
    text_primary: Color,
    text_secondary: Color,
    text_muted: Color,
    accent: Color,
    toggle_off: Color,
    knob: Color,
    danger: Color,
}
