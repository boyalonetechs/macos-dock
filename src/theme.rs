pub struct MacTheme {
    pub icon_size: i32,
    pub icon_spacing: i32,
    pub padding_x: f64,
    pub padding_y: f64,
    pub bottom_margin: f64,
    pub corner_radius: f64,

    // Liquid glass: near-zero fill, all blur comes from compositor
    pub bg_top: (f64, f64, f64, f64),
    pub bg_bottom: (f64, f64, f64, f64),

    // Gloss: bright specular strip on the top rim
    pub gloss_top: (f64, f64, f64, f64),
    pub gloss_bottom: (f64, f64, f64, f64),

    // Outer border
    pub stroke_outer: (f64, f64, f64, f64),

    // Inner border (bottom half — gives glass thickness illusion)
    pub stroke_inner: (f64, f64, f64, f64),

    pub active_dot_color: (f64, f64, f64, f64),
    pub max_zoom: f64,
    pub sigma: f64,          // hint to compositor blur radius
    pub separator_color: (f64, f64, f64, f64),
    pub separator_width: f64,
}

impl MacTheme {
    pub fn new_liquid_glass() -> Self {
        Self {
            icon_size: 60,
            icon_spacing: 68,   // 60px icon + 8px gap
            padding_x: 14.0,
            padding_y: 9.0,
            bottom_margin: 6.0,
            corner_radius: 39.0, // dock_height/2 = (60+18)/2 = 39 → true pill

            // Pure glass body: white with almost zero opacity
            // No blue, no grey — just barely-there white so the pill is
            // distinguishable against any wallpaper when blur is active
            bg_top:    (1.0, 1.0, 1.0, 0.06),
            bg_bottom: (1.0, 1.0, 1.0, 0.10),

            // Specular gloss: bright white strip at the very top rim
            gloss_top:    (1.0, 1.0, 1.0, 0.05),
            gloss_bottom: (1.0, 1.0, 1.0, 0.00),

            // Outer border: crisp white, softly transparent
            stroke_outer: (1.0, 1.0, 1.0, 0.45),

            // Inner border (inside bottom half): slightly warmer, lower opacity
            stroke_inner: (1.0, 1.0, 1.0, 0.18),

            // White running-app dot
            active_dot_color: (1.0, 1.0, 1.0, 0.90),

            max_zoom: 1.32,
            sigma: 22.0,   // pass to compositor as blur radius hint

            separator_color: (1.0, 1.0, 1.0, 0.30),
            separator_width: 1.0,
        }
    }

    /// Total rendered dock height in pixels
    pub fn dock_height(&self) -> i32 {
        (self.icon_size as f64 + self.padding_y * 2.0).ceil() as i32
    }

    pub fn padding_top(&self) -> f64 {
        self.padding_y
    }
}