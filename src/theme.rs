pub struct MacTheme {
    pub dock_height: i32,
    pub icon_size: i32,
    pub icon_spacing: i32,
    pub corner_radius: f64,
    pub margin: f64,
    pub bg_top: (f64, f64, f64, f64),
    pub bg_bottom: (f64, f64, f64, f64),
    pub stroke_inner: (f64, f64, f64, f64),
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
            dock_height: 76,
            icon_size: 48,
            icon_spacing: 58,
            corner_radius: 22.0,
            margin: 10.0,
            bg_top: (0.1, 0.1, 0.1, 0.4),
            bg_bottom: (0.05, 0.05, 0.05, 0.5),
            stroke_inner: (1.0, 1.0, 1.0, 0.15),
            stroke_outer: (0.0, 0.0, 0.0, 0.6),
            active_dot_color: (1.0, 1.0, 1.0, 0.85),
            max_zoom: 1.8,
            sigma: 48.0,
            separator_color: (1.0, 1.0, 1.0, 0.15),
            separator_width: 1.0,
        }
    }

    pub fn padding_top(&self) -> f64 { 8.0 }
}
