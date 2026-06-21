use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub desktop_file: Option<String>,
    pub icon_path: Option<String>,
}

pub struct AppTracker {
    pub windows: HashSet<u32>,
    pub apps: Vec<AppEntry>,
}

impl AppTracker {
    pub fn new() -> Self {
        Self {
            windows: HashSet::new(),
            apps: Vec::new(),
        }
    }

    pub fn update_from_window_list(&mut self, window_ids: &[u32]) {
        let new_set: HashSet<u32> = window_ids.iter().copied().collect();
        if new_set != self.windows {
            self.windows = new_set;
        }
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}
