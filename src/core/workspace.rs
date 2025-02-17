use super::window::Window;

pub struct Workspace {
    pub windows: Vec<Window>,
    pub focused: Option<usize>,
    pub index: usize,
    pub name: String,
}

impl Workspace {
    pub fn new(index: usize) -> Self {
        Self {
            windows: Vec::new(),
            focused: None,
            index,
            name: format!("Workspace {}", index + 1),
        }
    }

    pub fn add_window(&mut self, window: Window) {
        self.windows.push(window);
        self.focused = Some(self.windows.len() - 1);
    }

    pub fn remove_window(&mut self, window_id: u64) {
        if let Some(idx) = self.windows.iter().position(|w| w.id == window_id) {
            self.windows.remove(idx);
            if self.focused == Some(idx) {
                self.focused = if !self.windows.is_empty() {
                    Some(idx.saturating_sub(1))
                } else {
                    None
                };
            }
        }
    }

    pub fn get_focused_window(&self) -> Option<&Window> {
        self.focused.and_then(|idx| self.windows.get(idx))
    }
}
