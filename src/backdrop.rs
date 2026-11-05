//! Compositor-independent backdrop blur.
//!
//! X11 has no portable "blur behind my window" API. `_KDE_NET_WM_BLUR_BEHIND_REGION`
//! is only understood by KWin and (optionally) picom; Mutter/GNOME, Xfwm, Compiz,
//! etc. simply ignore it. When that happens a translucent dock has nothing to
//! reveal and appears as an empty outline.
//!
//! To make the glass effect work everywhere we build our own backdrop from the
//! current desktop wallpaper: compose it to the screen size, blur it once, and
//! expose the result as a cairo surface that the renderer samples behind the
//! pill. The wallpaper is static, so this is a one-time cost at startup.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use cairo::{Format, ImageSurface};
use image::{DynamicImage, RgbImage, imageops::FilterType};

pub struct Backdrop {
    /// Full-screen, already-blurred wallpaper. `None` until the background
    /// loader finishes (or permanently, if no wallpaper can be read).
    surface: Option<ImageSurface>,
    receiver: Option<Receiver<Vec<u8>>>,
    width: i32,
    height: i32,
}

impl Backdrop {
    pub fn new(screen_width: u16, screen_height: u16, sigma: f64) -> Self {
        let width = screen_width as i32;
        let height = screen_height as i32;

        // Decoding a 4K wallpaper and resizing it can take a moment in a debug
        // build. Do it off the UI thread so the dock appears immediately with
        // its opaque fallback and switches to glass as soon as the blur is
        // ready.
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            if let Some(pixels) = build_pixels(width, height, sigma) {
                let _ = sender.send(pixels);
            }
        });

        Self {
            surface: None,
            receiver: Some(receiver),
            width,
            height,
        }
    }

    /// Poll the background loader. Returns `true` exactly once, when the
    /// blurred wallpaper has just become available (callers use this to force
    /// a redraw).
    pub fn poll_ready(&mut self) -> bool {
        let Some(receiver) = &self.receiver else {
            return false;
        };

        match receiver.try_recv() {
            Ok(pixels) => {
                self.receiver = None;
                self.surface = surface_from_pixels(pixels, self.width, self.height).ok();
                if self.surface.is_none() {
                    eprintln!(
                        "[macos-dock] failed to create backdrop surface; using opaque glass fallback"
                    );
                }
                self.surface.is_some()
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                eprintln!(
                    "[macos-dock] no readable wallpaper found; using opaque glass fallback"
                );
                false
            }
        }
    }

    pub fn surface(&self) -> Option<&ImageSurface> {
        self.surface.as_ref()
    }
}

fn build_pixels(width: i32, height: i32, sigma: f64) -> Option<Vec<u8>> {
    if width <= 0 || height <= 0 {
        return None;
    }

    let path = find_wallpaper()?;
    let image = load_image(&path)?;
    let mode = wallpaper_mode();

    let composed = compose(image, width as u32, height as u32, &mode);
    let blurred = blur(&composed, sigma.max(1.0) as f32);
    Some(rgb_to_pixels(&blurred))
}

// ---------------------------------------------------------------------------
// Wallpaper discovery
// ---------------------------------------------------------------------------

fn find_wallpaper() -> Option<PathBuf> {
    // Explicit override always wins.
    if let Ok(path) = env::var("MACOS_DOCK_WALLPAPER") {
        let p = PathBuf::from(percent_decode(&path));
        if p.is_file() {
            return Some(p);
        }
    }

    // GNOME / Ubuntu / Pop!_OS / elementary (gsettings)
    for key in ["picture-uri-dark", "picture-uri"] {
        if let Some(value) = gsettings_get("org.gnome.desktop.background", key)
            && let Some(path) = uri_to_path(&value)
            && path.is_file()
        {
            return Some(path);
        }
    }

    // KDE Plasma
    if let Some(path) = kde_wallpaper() {
        return Some(path);
    }

    // XFCE
    if let Some(path) = xfce_wallpaper() {
        return Some(path);
    }

    // Common per-user locations (some sessions write an extensionless file).
    let home = env::var("HOME").ok()?;
    for candidate in [
        format!("{home}/.config/background"),
        format!("{home}/.config/wallpaper"),
        format!("{home}/.local/share/backgrounds/current"),
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }

    // Last resort: the first picture in the standard backgrounds directories.
    for dir in [
        format!("{home}/.local/share/backgrounds"),
        "/usr/share/backgrounds".to_string(),
        "/usr/share/wallpapers".to_string(),
    ] {
        if let Some(path) = first_image_in(Path::new(&dir)) {
            return Some(path);
        }
    }

    None
}

fn gsettings_get(schema: &str, key: &str) -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    Some(
        raw.trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string(),
    )
}

fn wallpaper_mode() -> String {
    gsettings_get("org.gnome.desktop.background", "picture-options")
        .unwrap_or_else(|| "zoom".to_string())
}

fn kde_wallpaper() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let config = fs::read_to_string(format!(
        "{home}/.config/plasma-org.kde.plasma.desktop-appletsrc"
    ))
    .ok()?;

    for line in config.lines() {
        if let Some(value) = line.trim().strip_prefix("Image=")
            && let Some(path) = uri_to_path(value)
            && path.is_file()
        {
            return Some(path);
        }
    }
    None
}

fn xfce_wallpaper() -> Option<PathBuf> {
    let output = Command::new("xfconf-query")
        .args([
            "-c",
            "xfce4-desktop",
            "-p",
            "/backdrop/screen0/monitor0/workspace0/last-image",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    path.is_file().then_some(path)
}

fn first_image_in(dir: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file() && is_image(path))
        .collect();
    entries.sort();
    entries.into_iter().next()
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "tif" | "tiff")
    )
}

fn uri_to_path(value: &str) -> Option<PathBuf> {
    let value = value.trim().trim_matches('\'').trim_matches('"');
    if let Some(rest) = value.strip_prefix("file://") {
        // file:///home/me/wall.png -> /home/me/wall.png
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        return Some(PathBuf::from(percent_decode(rest)));
    }
    if value.starts_with('/') {
        return Some(PathBuf::from(value));
    }
    None
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// Image composition
// ---------------------------------------------------------------------------

fn load_image(path: &Path) -> Option<DynamicImage> {
    // `image::open` guesses the format from the file extension; many desktop
    // wallpapers (e.g. ~/.config/background) have no extension, so sniff the
    // content instead.
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
}

fn compose(image: DynamicImage, width: u32, height: u32, mode: &str) -> RgbImage {
    let source = image.to_rgb8();
    let (src_w, src_h) = source.dimensions();
    if src_w == 0 || src_h == 0 {
        return RgbImage::new(width, height);
    }

    match mode {
        "stretched" => image::imageops::resize(&source, width, height, FilterType::Triangle),
        "scaled" => {
            let (w, h) = fit_within(src_w, src_h, width, height);
            let resized = image::imageops::resize(&source, w, h, FilterType::Triangle);
            let mut canvas = RgbImage::from_pixel(width, height, image::Rgb([0, 0, 0]));
            image::imageops::overlay(
                &mut canvas,
                &resized,
                ((width - w) / 2) as i64,
                ((height - h) / 2) as i64,
            );
            canvas
        }
        "centered" => {
            let mut canvas = RgbImage::from_pixel(width, height, image::Rgb([0, 0, 0]));
            let x = (width as i64 - src_w as i64) / 2;
            let y = (height as i64 - src_h as i64) / 2;
            image::imageops::overlay(&mut canvas, &source, x, y);
            canvas
        }
        "wallpaper" => {
            let mut canvas = RgbImage::new(width, height);
            for y in (0..height).step_by(src_h.max(1) as usize) {
                for x in (0..width).step_by(src_w.max(1) as usize) {
                    image::imageops::overlay(&mut canvas, &source, x as i64, y as i64);
                }
            }
            canvas
        }
        // "zoom", "spanned", or anything unknown: cover the screen.
        _ => {
            let scale = (width as f64 / src_w as f64).max(height as f64 / src_h as f64);
            let scaled_w = ((src_w as f64 * scale).round() as u32).max(1);
            let scaled_h = ((src_h as f64 * scale).round() as u32).max(1);
            let resized = image::imageops::resize(&source, scaled_w, scaled_h, FilterType::Triangle);
            let x = (scaled_w - width) / 2;
            let y = (scaled_h - height) / 2;
            image::imageops::crop_imm(&resized, x, y, width, height).to_image()
        }
    }
}

fn fit_within(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let scale = (max_w as f64 / src_w as f64).min(max_h as f64 / src_h as f64);
    (
        ((src_w as f64 * scale).round() as u32).clamp(1, max_w),
        ((src_h as f64 * scale).round() as u32).clamp(1, max_h),
    )
}

fn blur(image: &RgbImage, sigma: f32) -> RgbImage {
    let (width, height) = image.dimensions();
    let scale = 4u32;
    let small_w = (width / scale).max(1);
    let small_h = (height / scale).max(1);

    // Blurring a quarter-size copy is ~16x cheaper and indistinguishable once
    // the result is scaled back up behind a translucent pill.
    let small = image::imageops::resize(image, small_w, small_h, FilterType::Triangle);
    let blurred = image::imageops::blur(&small, sigma / scale as f32);
    image::imageops::resize(&blurred, width, height, FilterType::Triangle)
}

fn rgb_to_pixels(image: &RgbImage) -> Vec<u8> {
    let (width, height) = image.dimensions();
    let mut data = vec![0u8; width as usize * height as usize * 4];

    // Cairo `ARgb32` is B, G, R, A in memory on little-endian machines.
    for (index, pixel) in image.pixels().enumerate() {
        let offset = index * 4;
        data[offset] = pixel[2];
        data[offset + 1] = pixel[1];
        data[offset + 2] = pixel[0];
        data[offset + 3] = 0xff;
    }
    data
}

fn surface_from_pixels(
    data: Vec<u8>,
    width: i32,
    height: i32,
) -> Result<ImageSurface, Box<dyn std::error::Error>> {
    Ok(ImageSurface::create_for_data(
        data,
        Format::ARgb32,
        width,
        height,
        width * 4,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_file_uris_and_percent_escapes() {
        assert_eq!(
            uri_to_path("file:///home/me/My%20Wallpaper.png"),
            Some(PathBuf::from("/home/me/My Wallpaper.png"))
        );
        assert_eq!(
            uri_to_path("'/home/me/wall.jpg'"),
            Some(PathBuf::from("/home/me/wall.jpg"))
        );
    }

    #[test]
    fn cover_mode_fills_the_screen() {
        let image = DynamicImage::new_rgb8(400, 100);
        let composed = compose(image, 800, 400, "zoom");
        assert_eq!(composed.dimensions(), (800, 400));
    }

    #[test]
    fn blur_preserves_size() {
        let image = RgbImage::from_pixel(64, 32, image::Rgb([10, 20, 30]));
        assert_eq!(blur(&image, 8.0).dimensions(), (64, 32));
    }

    #[test]
    fn builds_a_backdrop_when_a_wallpaper_is_available() {
        if find_wallpaper().is_none() {
            return;
        }
        let pixels = build_pixels(640, 360, 22.0).expect("wallpaper found but decode failed");
        assert_eq!(pixels.len(), 640 * 360 * 4);

        // A wallpaper that survived composition and blur must still contain
        // some variation (a flat colour would mean we blurred the wrong thing).
        let (mut min, mut max) = (255u8, 0u8);
        for pixel in pixels.chunks_exact(4) {
            min = min.min(pixel[2]);
            max = max.max(pixel[2]);
        }
        assert!(max.saturating_sub(min) > 2, "backdrop is a flat colour");

        let surface =
            surface_from_pixels(pixels, 640, 360).expect("failed to create cairo surface");
        assert_eq!((surface.width(), surface.height()), (640, 360));
    }
}
