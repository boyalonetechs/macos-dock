use std::collections::HashMap;
use cairo::ImageSurface;
use crate::desktop::{DesktopEntry, IconCache};

#[derive(Clone, PartialEq)]
pub enum DockItemType {
    PinnedApp,
    RunningApp,
    Separator,
    Folder,
    Trash,
}

#[derive(Clone)]
pub struct DockIcon {
    pub name: String,
    pub icon_name: String,
    pub is_running: bool,
    pub is_active: bool,
    pub x: f64,
    pub zoom: f64,
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
        // Pinned apps (Left section)
        self.add_pinned("Finder", "system-file-manager", None);
        self.add_pinned("Launchpad", "applications-system", None);
        self.add_pinned("System Settings", "preferences-system", None);

        // Separator 1
        self.icons.push(DockIcon {
            name: String::new(),
            icon_name: String::new(),
            is_running: false,
            is_active: false,
            x: 0.0,
            zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Separator,
        });

        // Folders and Trash (Right section, added now but they will be kept at the end during sync)
        // Separator 2
        self.icons.push(DockIcon {
            name: String::new(),
            icon_name: String::new(),
            is_running: false,
            is_active: false,
            x: 0.0,
            zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Separator,
        });
        
        self.icons.push(DockIcon {
            name: "Downloads".to_string(),
            icon_name: "folder-download".to_string(),
            is_running: false,
            is_active: false,
            x: 0.0,
            zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Folder,
        });

        self.icons.push(DockIcon {
            name: "Trash".to_string(),
            icon_name: "user-trash".to_string(),
            is_running: false,
            is_active: false,
            x: 0.0,
            zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Trash,
        });
    }

    fn add_pinned(&mut self, name: &str, default_icon: &str, entry_idx: Option<usize>) {
        self.icons.push(DockIcon {
            name: name.to_string(),
            icon_name: default_icon.to_string(),
            is_running: false,
            is_active: false,
            x: 0.0,
            zoom: 1.0,
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
                        is_active: false,
                        x: 0.0,
                        zoom: 1.0,
                        entry_index: Some(idx),
                        item_type: DockItemType::RunningApp,
                    });
                    // We'll update new_window_map index later after rebuilding self.icons
                }
            }
        }

        // Rebuild self.icons to preserve order: Pinned -> Sep1 -> Running -> Sep2 -> Folders/Trash
        let mut new_icons = Vec::new();
        
        // 1. Pinned apps
        for icon in &self.icons {
            if icon.item_type == DockItemType::PinnedApp {
                new_icons.push(icon.clone());
            }
        }
        
        // 2. Separator 1
        new_icons.push(DockIcon {
            name: String::new(),
            icon_name: String::new(),
            is_running: false,
            is_active: false,
            x: 0.0,
            zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Separator,
        });

        // 3. Running apps
        for icon in running_icons {
            new_icons.push(icon);
        }

        // 4. Separator 2
        new_icons.push(DockIcon {
            name: String::new(),
            icon_name: String::new(),
            is_running: false,
            is_active: false,
            x: 0.0,
            zoom: 1.0,
            entry_index: None,
            item_type: DockItemType::Separator,
        });

        // 5. Folders / Trash
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

    pub fn set_icon_positions(&mut self, start_x: f64, spacing: f64, icon_size: f64) {
        let mut current_x = start_x + icon_size / 2.0;
        for icon in self.icons.iter_mut() {
            if icon.item_type == DockItemType::Separator {
                icon.x = current_x - (icon_size - spacing)/2.0; 
                current_x += spacing * 0.5; // Separators take half spacing
            } else {
                icon.x = current_x;
                current_x += spacing;
            }
        }
    }

    pub fn update_zoom(&mut self, cursor_x: f64, sigma: f64, max_zoom: f64) {
        for icon in self.icons.iter_mut() {
            if icon.item_type == DockItemType::Separator {
                icon.zoom = 1.0;
                continue;
            }
            let dist = (icon.x - cursor_x).abs();
            icon.zoom = 1.0 + (max_zoom - 1.0) * (-(dist * dist) / (2.0 * sigma * sigma)).exp();
        }
    }
}
