use super::window::Window;

pub struct Workspace {
    pub windows: Vec<Window>,
    pub focused: Option<usize>,
}
