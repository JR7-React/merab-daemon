use std::path::{Path, PathBuf};

const CANDIDATES: &[&str] = &["merab.md", "AGENTS.md", ".merab/instructions.md"];

pub struct ProjectInstructions {
    pub path: PathBuf,
    pub content: String,
}

impl ProjectInstructions {
    pub fn load(project_path: &Path) -> Option<Self> {
        for candidate in CANDIDATES {
            let path = project_path.join(candidate);
            if path.exists()
                && let Ok(content) = std::fs::read_to_string(&path)
                && !content.trim().is_empty()
            {
                return Some(Self { path, content });
            }
        }
        None
    }
}
