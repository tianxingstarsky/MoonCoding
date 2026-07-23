use std::path::Path;

pub struct Project {
    root: String,
}

impl Project {
    pub fn open(root: &Path) -> Self {
        Self {
            root: root.display().to_string(),
        }
    }
}

pub fn project_name(project: &Project) -> &str {
    &project.root
}
