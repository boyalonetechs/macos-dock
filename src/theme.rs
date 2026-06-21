pub struct MacTheme {
    pub icon_size: i32,
    pub icon_spacing: i32,
    pub padding_x: f64,
    pub padding_y: f64,
    pub bottom_margin: f64,
    pub corner_radius: f64,
    pub bg_top: (f64, f64, f64, f64),
    pub bg_bottom: (f64, f64, f64, f64),
    pub stroke_outer: (f64, f64, f64, f64),
    pub active_dot_color: (f64, f64, f64, f64),
    pub max_zoom: f64,
    pub sigma: f64,
    pub separator_color: (f64, f64, f64, f64),
    pub separator_width: f64,
}

impl MacTheme {
    pub fn new_dark() -> Self {
        Self {
            icon_size: 48,
            icon_spacing: 58,
            padding_x: 20.0,
            padding_y: 12.0,
            bottom_margin: 10.0,
            corner_radius: 24.0,
            bg_top: (0.1, 0.1, 0.1, 0.5),
            bg_bottom: (0.15, 0.15, 0.15, 0.6),
            stroke_outer: (1.0, 1.0, 1.0, 0.3),
            active_dot_color: (1.0, 1.0, 1.0, 0.85),
            max_zoom: 1.8,
            sigma: 48.0,
            separator_color: (1.0, 1.0, 1.0, 0.2),
            separator_width: 1.0,
        }
    }

    pub fn dock_height(&self) -> i32 {
        (self.icon_size as f64 + self.padding_y * 2.0).ceil() as i32
    }
    pub fn padding_top(&self) -> f64 {
        self.padding_y
    }
}
