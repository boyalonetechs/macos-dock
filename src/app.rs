use std::collections::HashMap;
use cairo::ImageSurface;
use crate::desktop::{DesktopEntry, IconCache};

#[derive(Clone, PartialEq)]
pub enum DockItemType {
    Launcher,
    PinnedApp,
    RunningApp,
    Separator,
    Folder,
    Trash,
}

const MAX_DOCK_ICONS: usize = 25;

#[derive(Clone)]
pub struct DockIcon {
    pub name: String,
    pub icon_name: String,
    pub is_running: bool,
    pub x: f64,
    pub zoom: f64,
    pub target_zoom: f64,
    pub entry_index: Option<usize>,
    pub item_type: DockItemType,
}

pub struct AppManager {
    pub icons: Vec<DockIcon>,
    pub entries: Vec<DesktopEntry>,
    icon_cache: IconCache,
    window_map: HashMap<u32, usize>,
    class_map: HashMap<String, usize>,
}

impl AppManager {
    pub fn new() -> Self {
        let entries = crate::desktop::find_desktop_files();
        let mut manager = Self {
            icons: Vec::new(),
            entries,
            icon_cache: IconCache::new(),
            window_map: HashMap::new(),
            class_map: HashMap::new(),
        };
        manager.init_pinned_items();
        manager
    }

    fn init_pinned_items(&mut self) {
        // Fixed tail icons: separator + Downloads + Trash = 3
        let tail_count = 3usize;
        let max_apps = MAX_DOCK_ICONS.saturating_sub(1 + tail_count); // 1 for launcher

        // All installed applications sorted alphabetically
        let mut app_indices: Vec<usize> = (0..self.entries.len()).collect();
        app_indices.sort_by(|&a, &b| self.entries[a].name.to_lowercase().cmp(&self.entries[b].name.to_lowercase()));

        let app_count = app_indices.len().min(max_apps);
        for &idx in &app_indices[..app_count] {
            let name = self.entries[idx].name.clone();
            let icon_name = self.entries[idx].icon_name.clone();
            self.add_pinned(&name, &icon_name, Some(idx));
        }

        // Separator 1 (between apps and system items)
        self.icons.push(DockIcon {
            name: String::new(),
            icon_name: String::new(),
            is_running: false,
            x: 0.0,
            zoom: 1.0,
            target_zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Separator,
        });

        // System items
        self.icons.push(DockIcon {
            name: "Downloads".to_string(),
            icon_name: "folder-download".to_string(),
            is_running: false,
            x: 0.0,
            zoom: 1.0,
            target_zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Folder,
        });

        self.icons.push(DockIcon {
            name: "Trash".to_string(),
            icon_name: "user-trash".to_string(),
            is_running: false,
            x: 0.0,
            zoom: 1.0,
            target_zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Trash,
        });

        // Launcher icon at the very front (leftmost)
        let launcher = DockIcon {
            name: "Launcher".to_string(),
            icon_name: "view-app-grid".to_string(),
            is_running: false,
            x: 0.0,
            zoom: 1.0,
            target_zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Launcher,
        };
        self.icons.insert(0, launcher);
    }

    fn add_pinned(&mut self, name: &str, default_icon: &str, entry_idx: Option<usize>) {
        self.icons.push(DockIcon {
            name: name.to_string(),
            icon_name: default_icon.to_string(),
            is_running: false,
            x: 0.0,
            zoom: 1.0,
            target_zoom: 1.0,
            entry_index: entry_idx,
            item_type: DockItemType::PinnedApp,
        });
    }

    pub fn sync_windows(&mut self, window_ids: &[u32], get_class: &impl Fn(u32) -> Option<String>) {
        let mut running_icons: Vec<DockIcon> = Vec::new();
        let mut seen_entries = std::collections::HashSet::new();
        let mut new_window_map = HashMap::new();

        for icon in &mut self.icons {
            if icon.item_type == DockItemType::PinnedApp {
                icon.is_running = false;
            }
        }

        for &wid in window_ids {
            let entry_idx = if let Some(&old_idx) = self.window_map.get(&wid) {
                if old_idx < self.icons.len() {
                    let old_icon = &self.icons[old_idx];
                    if let Some(e_idx) = old_icon.entry_index {
                        Some(e_idx)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let entry_idx = entry_idx.or_else(|| {
                let class = get_class(wid)?;
                let cls_lower = class.to_lowercase();
                if let Some(&idx) = self.class_map.get(&cls_lower) {
                    Some(idx)
                } else {
                    let entry = crate::desktop::match_desktop_to_class(&class, &self.entries)?;
                    let idx = self.entries.iter().position(|e| e.filename == entry.filename)?;
                    self.class_map.insert(cls_lower, idx);
                    Some(idx)
                }
            });

            if let Some(idx) = entry_idx {
                let mut is_pinned = false;
                for (i, icon) in self.icons.iter_mut().enumerate() {
                    if icon.item_type == DockItemType::PinnedApp && icon.entry_index == Some(idx) {
                        icon.is_running = true;
                        is_pinned = true;
                        new_window_map.insert(wid, i);
                        break;
                    }
                }

                if !is_pinned && seen_entries.insert(idx) {
                    let name = self.entries[idx].name.clone();
                    let icon_name = self.entries[idx].icon_name.clone();
                    running_icons.push(DockIcon {
                        name,
                        icon_name,
                        is_running: true,
                        x: 0.0,
                        zoom: 1.0,
                        target_zoom: 1.0,
                        entry_index: Some(idx),
                        item_type: DockItemType::RunningApp,
                    });
                    // We'll update new_window_map index later after rebuilding self.icons
                }
            }
        }

        // Rebuild self.icons to preserve order: Pinned -> Sep -> Running -> Folder/Trash
        let mut new_icons = Vec::new();
        
        // 1. Pinned apps
        for icon in &self.icons {
            if icon.item_type == DockItemType::PinnedApp {
                new_icons.push(icon.clone());
            }
        }
        
        // 2. Running apps (if any non-pinned)
        if !running_icons.is_empty() {
            new_icons.push(DockIcon {
                name: String::new(),
                icon_name: String::new(),
                is_running: false,
                x: 0.0,
                zoom: 1.0,
                target_zoom: 1.0,
                entry_index: None,
                item_type: DockItemType::Separator,
            });
            for icon in running_icons {
                new_icons.push(icon);
            }
        }

        // 3. Separator and Folder / Trash
        new_icons.push(DockIcon {
            name: String::new(),
            icon_name: String::new(),
            is_running: false,
            x: 0.0,
            zoom: 1.0,
            target_zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Separator,
        });
        for icon in &self.icons {
            if icon.item_type == DockItemType::Folder || icon.item_type == DockItemType::Trash {
                new_icons.push(icon.clone());
            }
        }

        self.icons = new_icons;
        
        // Rebuild window map correctly since indices changed
        self.window_map.clear();
        for &wid in window_ids {
            if let Some(class) = get_class(wid) {
                if let Some(entry) = crate::desktop::match_desktop_to_class(&class, &self.entries) {
                    if let Some(idx) = self.entries.iter().position(|e| e.filename == entry.filename) {
                        for (i, icon) in self.icons.iter().enumerate() {
                            if icon.entry_index == Some(idx) {
                                self.window_map.insert(wid, i);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn load_icon_surface(&mut self, index: usize, size: i32) -> Option<ImageSurface> {
        if index < self.icons.len() {
            let icon_name = &self.icons[index].icon_name;
            if icon_name.is_empty() { return None; }
            self.icon_cache.get_or_load(icon_name, size)
        } else {
            None
        }
    }

    pub fn set_icon_positions(&mut self, start_x: f64, icon_size: f64, edge_gap: f64, sep_width: f64) {
        let mut current_x = start_x;
        for icon in self.icons.iter_mut() {
            if icon.item_type == DockItemType::Separator {
                icon.x = current_x + sep_width / 2.0;
                current_x += sep_width + edge_gap;
            } else {
                let display_w = icon_size * icon.zoom;
                current_x += display_w / 2.0;
                icon.x = current_x;
                current_x += display_w / 2.0 + edge_gap;
            }
        }
    }

    pub fn update_zoom(&mut self, cursor_x: f64, sigma: f64, max_zoom: f64) {
        for icon in self.icons.iter_mut() {
            if icon.item_type == DockItemType::Separator {
                icon.target_zoom = 1.0;
                continue;
            }
            let dist = (icon.x - cursor_x).abs();
            icon.target_zoom = 1.0 + (max_zoom - 1.0) * (-(dist * dist) / (2.0 * sigma * sigma)).exp();
        }
    }

    pub fn all_entries(&self) -> &[DesktopEntry] {
        &self.entries
    }
}
