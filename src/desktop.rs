use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use cairo::{Context, Format, ImageSurface};

pub struct DesktopEntry {
    pub name: String,
    pub icon_name: String,
    pub filename: String,
    pub startup_wm_class: Option<String>,
}

pub struct IconCache {
    icons: HashMap<String, ImageSurface>,
}

impl IconCache {
    pub fn new() -> Self {
        Self { icons: HashMap::new() }
    }

    pub fn get_or_load(&mut self, name: &str, size: i32) -> Option<ImageSurface> {
        let key = format!("{}:{}", name, size);
        if let Some(surf) = self.icons.get(&key) {
            return Some(surf.clone());
        }
        if let Some(surf) = load_app_icon(name, size) {
            self.icons.insert(key.clone(), surf.clone());
            return Some(surf);
        }
        None
    }
}

pub fn find_desktop_files() -> Vec<DesktopEntry> {
    let mut entries = Vec::new();
    let dirs = [
        "/usr/share/applications/",
        &format!("{}/.local/share/applications/", std::env::var("HOME").unwrap_or_default()),
    ];
    for dir in &dirs {
        if let Ok(read) = fs::read_dir(dir) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    if let Some(de) = parse_desktop_file(&path) {
                        entries.push(de);
                    }
                }
            }
        }
    }
    entries
}

pub fn parse_desktop_file(path: &Path) -> Option<DesktopEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut icon_name = None;
    let mut startup_wm_class = None;
    let mut in_desktop = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop = true;
            continue;
        }
        if line.starts_with('[') && line != "[Desktop Entry]" {
            if in_desktop { break; }
            continue;
        }
        if !in_desktop { continue; }

        if let Some(val) = line.strip_prefix("Name=") {
            name = Some(val.to_string());
        } else if let Some(val) = line.strip_prefix("Icon=") {
            icon_name = Some(val.to_string());
        } else if let Some(val) = line.strip_prefix("StartupWMClass=") {
            startup_wm_class = Some(val.to_string());
        } else if line.starts_with("Name[") {
            if name.is_none() {
                if let Some(val) = line.split('=').nth(1) {
                    name = Some(val.to_string());
                }
            }
        }
    }

    let filename = path.file_stem()?.to_str()?.to_lowercase();
    Some(DesktopEntry {
        name: name.unwrap_or_else(|| filename.clone()),
        icon_name: icon_name.unwrap_or_else(|| filename.clone()),
        filename,
        startup_wm_class,
    })
}

fn search_icon_dirs(icon_name: &str, size: i32) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let themes = ["MacTahoe", "MacTahoe-dark", "MacTahoe-light", "WhiteSur", "WhiteSur-dark", "WhiteSur-light"];
    let categories = ["apps", "places", "mimetypes", "devices"];
    let sub_dirs = ["scalable", &format!("{}", size), "48", "64", "128", "256"];

    let mut dirs = Vec::new();

    // User icon themes (prioritize MacTahoe, then WhiteSur)
    for theme in &themes {
        for cat in &categories {
            for sub in &sub_dirs {
                dirs.push(format!("{}/.local/share/icons/{}/{}/{}", home, theme, cat, sub));
            }
        }
    }

    // System icon themes
    for sub in &sub_dirs {
        for cat in &categories {
            dirs.push(format!("{}/.local/share/icons/hicolor/{}/{}", home, sub, cat));
            dirs.push(format!("/usr/share/icons/hicolor/{}/{}", sub, cat));
            dirs.push(format!("/usr/share/icons/Adwaita/{}/{}", sub, cat));
        }
    }

    dirs.push(format!("/usr/share/pixmaps"));

    for dir in &dirs {
        let p = Path::new(dir);
        for ext in &["svg", "png", "xpm"] {
            let candidate = p.join(format!("{}.{}", icon_name, ext));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn load_app_icon(icon_name: &str, size: i32) -> Option<ImageSurface> {
    let icon_name = icon_name.trim_start_matches('/');
    if icon_name.starts_with('/') || icon_name.starts_with('~') {
        let path = if icon_name.starts_with("~/") {
            let home = std::env::var("HOME").ok()?;
            PathBuf::from(&home).join(&icon_name[2..])
        } else {
            PathBuf::from(icon_name)
        };
        return load_image_file(&path, size);
    }

    let path = search_icon_dirs(icon_name, size)?;
    load_image_file(&path, size)
}

fn load_image_file(path: &Path, size: i32) -> Option<ImageSurface> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "svg" => render_svg_to_surface(path, size),
        "png" => render_png_to_surface(path, size),
        _ => None,
    }
}

fn render_svg_to_surface(path: &Path, size: i32) -> Option<ImageSurface> {
    let svg_data = fs::read(path).ok()?;
    let tree = usvg::Tree::from_data(&svg_data, &usvg::Options::default()).ok()?;
    let pixmap_size = tree.size();

    let size_f = size as f32;
    let scale = size_f / pixmap_size.height().max(pixmap_size.width());
    let w = (pixmap_size.width() * scale).ceil() as u32;
    let h = (pixmap_size.height() * scale).ceil() as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let data = pixmap.data().to_vec();
    let mut surface = ImageSurface::create(Format::ARgb32, w as i32, h as i32).ok()?;
    let stride = surface.stride();
    {
        let mut surf_data = surface.data().ok()?;
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let si = ((y * w as i32 + x) * 4) as usize;
                let di = (y * stride + x * 4) as usize;
                if si + 3 < data.len() && di + 3 < surf_data.len() {
                    // Source: premultiplied RGBA [R, G, B, A]
                    // Cairo ARGB32 LE:  [B, G, R, A] (premultiplied)
                    surf_data[di]     = data[si + 2]; // B
                    surf_data[di + 1] = data[si + 1]; // G
                    surf_data[di + 2] = data[si];     // R
                    surf_data[di + 3] = data[si + 3]; // A
                }
            }
        }
    }

    Some(surface)
}

fn render_png_to_surface(path: &Path, size: i32) -> Option<ImageSurface> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgba8();
    let (w, h) = rgb.dimensions();

    let scale = (size as f64) / (h as f64).max(w as f64);
    let new_w = (w as f64 * scale) as i32;
    let new_h = (h as f64 * scale) as i32;

    let surface = ImageSurface::create(Format::ARgb32, new_w, new_h).ok()?;
    let ctx = Context::new(&surface).ok()?;

    let scaled = image::imageops::resize(&rgb, new_w as u32, new_h as u32, image::imageops::Lanczos3);
    let data = scaled.into_raw();

    let stride = new_w * 4;
    let len = (stride * new_h) as usize;
    let mut argb = vec![0u8; len];
    for y in 0..new_h {
        for x in 0..new_w {
            let si = ((y * new_w + x) * 4) as usize;
            let di = (y * stride + x * 4) as usize;
            if si + 3 < data.len() && di + 3 < len {
                let r = data[si];
                let g = data[si + 1];
                let b = data[si + 2];
                let a = data[si + 3];
                // Source: non-premultiplied RGBA
                // Cairo ARGB32 LE: [B, G, R, A] with premultiplied alpha
                if a > 0 {
                    argb[di]     = (b as u16 * a as u16 / 255) as u8;
                    argb[di + 1] = (g as u16 * a as u16 / 255) as u8;
                    argb[di + 2] = (r as u16 * a as u16 / 255) as u8;
                    argb[di + 3] = a;
                } else {
                    argb[di]     = 0;
                    argb[di + 1] = 0;
                    argb[di + 2] = 0;
                    argb[di + 3] = 0;
                }
            }
        }
    }

    let img_surf = ImageSurface::create_for_data(
        argb,
        Format::ARgb32,
        new_w, new_h,
        stride,
    ).ok()?;

    ctx.set_source_surface(&img_surf, 0.0, 0.0).ok()?;
    ctx.paint().ok()?;

    Some(surface)
}

pub fn match_desktop_to_class<'a>(class: &'a str, entries: &'a [DesktopEntry]) -> Option<&'a DesktopEntry> {
    let class_lower = class.to_lowercase();

    // 1. Exact match on StartupWMClass (highest priority)
    for entry in entries {
        if let Some(ref wm_class) = entry.startup_wm_class {
            if wm_class.to_lowercase() == class_lower {
                return Some(entry);
            }
        }
    }

    // 2. Exact match on filename
    for entry in entries {
        if entry.filename == class_lower {
            return Some(entry);
        }
    }

    // 3. Partial match on StartupWMClass
    for entry in entries {
        if let Some(ref wm_class) = entry.startup_wm_class {
            let wm_lower = wm_class.to_lowercase();
            if wm_lower.contains(&class_lower) || class_lower.contains(&wm_lower) {
                return Some(entry);
            }
        }
    }

    // 4. Partial match on filename
    for entry in entries {
        if entry.filename.contains(&class_lower) || class_lower.contains(&entry.filename) {
            return Some(entry);
        }
    }

    // 5. Partial match on name
    for entry in entries {
        if entry.name.to_lowercase().contains(&class_lower) || class_lower.contains(&entry.name.to_lowercase()) {
            return Some(entry);
        }
    }
    None
}
