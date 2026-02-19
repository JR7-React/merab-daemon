use std::path::{Path, PathBuf};

use merab_core::ProjectInstructions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub name: String,
    pub language: String,
    pub root_path: String,
    pub description: String,
    pub key_files: Vec<String>,
    pub dependencies: Vec<String>,
    pub instructions: Option<String>,
    pub instructions_path: Option<PathBuf>,
}

impl ProjectContext {
    pub fn detect(root: &Path) -> Self {
        let mut ctx = if root.join("Cargo.toml").exists() {
            detect_rust(root)
        } else if root.join("package.json").exists() {
            detect_node(root)
        } else if root.join("go.mod").exists() {
            detect_go(root)
        } else if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            detect_python(root)
        } else {
            detect_generic(root)
        };

        if let Some(instr) = ProjectInstructions::load(root) {
            ctx.instructions = Some(instr.content);
            ctx.instructions_path = Some(instr.path);
        }

        ctx
    }

    pub fn to_prompt_string(&self) -> String {
        let mut s = format!(
            "Project: {} | Language: {} | Root: {}\n",
            self.name, self.language, self.root_path
        );
        if !self.description.is_empty() {
            s.push_str(&format!("Description: {}\n", self.description));
        }
        if !self.dependencies.is_empty() {
            s.push_str(&format!(
                "Key dependencies: {}\n",
                self.dependencies.join(", ")
            ));
        }
        if !self.key_files.is_empty() {
            s.push_str(&format!("Structure: {}\n", self.key_files.join(", ")));
        }
        s
    }

    pub fn display(&self) {
        println!("Project:      {}", self.name);
        println!("Language:     {}", self.language);
        println!("Root:         {}", self.root_path);
        if !self.description.is_empty() {
            println!("Description:  {}", self.description);
        }
        if !self.dependencies.is_empty() {
            println!("Dependencies: {}", self.dependencies.join(", "));
        }
        if !self.key_files.is_empty() {
            println!("Structure:    {}", self.key_files.join(", "));
        }
        if let Some(ref path) = self.instructions_path {
            println!(
                "Instructions: {} ({} chars)",
                path.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
                self.instructions.as_ref().map(|s| s.len()).unwrap_or(0)
            );
        }
    }
}

fn detect_rust(root: &Path) -> ProjectContext {
    let cargo_toml = read_file(root.join("Cargo.toml"));
    let name = toml_value(&cargo_toml, "name").unwrap_or_else(|| "unknown".to_string());
    let version = toml_value(&cargo_toml, "version").unwrap_or_default();
    let edition = toml_value(&cargo_toml, "edition").unwrap_or_else(|| "2021".to_string());

    let display_name = if version.is_empty() {
        name.clone()
    } else {
        format!("{} v{}", name, version)
    };

    let language = format!("Rust (edition {})", edition);
    let description = readme_excerpt(root);

    // Key files: workspace crates or src/
    let mut key_files = Vec::new();
    let crates_dir = root.join("crates");
    if crates_dir.exists() {
        let mut crates: Vec<String> = std::fs::read_dir(&crates_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        crates.sort();
        key_files.extend(crates);
    } else {
        if root.join("src/main.rs").exists() {
            key_files.push("src/main.rs".to_string());
        }
        if root.join("src/lib.rs").exists() {
            key_files.push("src/lib.rs".to_string());
        }
    }

    // Top-level dependencies from [dependencies] or [workspace.dependencies]
    let deps = rust_deps(&cargo_toml);

    ProjectContext {
        name: display_name,
        language,
        root_path: root.to_string_lossy().to_string(),
        description,
        key_files,
        dependencies: deps,
        instructions: None,
        instructions_path: None,
    }
}

fn detect_node(root: &Path) -> ProjectContext {
    let pkg = read_file(root.join("package.json"));
    let name = json_value(&pkg, "name").unwrap_or_else(|| "unknown".to_string());
    let version = json_value(&pkg, "version").unwrap_or_default();
    let desc = json_value(&pkg, "description").unwrap_or_default();

    let has_ts = root.join("tsconfig.json").exists();
    let language = if has_ts {
        "TypeScript"
    } else {
        "JavaScript (Node.js)"
    }
    .to_string();

    let display_name = if version.is_empty() {
        name
    } else {
        format!("{} v{}", name, version)
    };

    let deps = node_deps(&pkg);
    let key_files = src_structure(root);

    ProjectContext {
        name: display_name,
        language,
        root_path: root.to_string_lossy().to_string(),
        description: desc,
        key_files,
        dependencies: deps,
        instructions: None,
        instructions_path: None,
    }
}

fn detect_go(root: &Path) -> ProjectContext {
    let go_mod = read_file(root.join("go.mod"));
    let name = go_mod
        .lines()
        .find(|l| l.starts_with("module "))
        .map(|l| l.trim_start_matches("module ").trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let go_version = go_mod
        .lines()
        .find(|l| l.starts_with("go "))
        .map(|l| l.trim_start_matches("go ").trim().to_string())
        .unwrap_or_default();

    let language = format!("Go {}", go_version);
    let description = readme_excerpt(root);

    ProjectContext {
        name,
        language,
        root_path: root.to_string_lossy().to_string(),
        description,
        key_files: src_structure(root),
        dependencies: Vec::new(),
        instructions: None,
        instructions_path: None,
    }
}

fn detect_python(root: &Path) -> ProjectContext {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let deps = if root.join("requirements.txt").exists() {
        read_file(root.join("requirements.txt"))
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .take(10)
            .map(|l| l.split("==").next().unwrap_or(l).trim().to_string())
            .collect()
    } else {
        Vec::new()
    };

    ProjectContext {
        name,
        language: "Python".to_string(),
        root_path: root.to_string_lossy().to_string(),
        description: readme_excerpt(root),
        key_files: src_structure(root),
        dependencies: deps,
        instructions: None,
        instructions_path: None,
    }
}

fn detect_generic(root: &Path) -> ProjectContext {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    ProjectContext {
        name,
        language: "Unknown".to_string(),
        root_path: root.to_string_lossy().to_string(),
        description: readme_excerpt(root),
        key_files: src_structure(root),
        dependencies: Vec::new(),
        instructions: None,
        instructions_path: None,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn read_file(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn readme_excerpt(root: &Path) -> String {
    let content = ["README.md", "README.txt", "README"]
        .iter()
        .find_map(|name| std::fs::read_to_string(root.join(name)).ok())
        .unwrap_or_default();

    content
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect()
}

/// Extract a simple `key = "value"` from TOML text (works for [package] section).
fn toml_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find(|l| {
            l.trim().starts_with(&format!("{} =", key))
                || l.trim().starts_with(&format!("{}=", key))
        })
        .and_then(|l| l.splitn(2, '=').nth(1))
        .map(|v| v.trim().trim_matches('"').to_string())
}

fn rust_deps(cargo_toml: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut deps = Vec::new();

    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" || trimmed == "[workspace.dependencies]" {
            in_deps = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_deps = false;
        }
        if in_deps {
            if let Some(name) = trimmed.splitn(2, '=').next() {
                let name = name.trim();
                if !name.is_empty() && !name.starts_with('#') {
                    deps.push(name.to_string());
                }
            }
        }
    }
    deps.into_iter().take(12).collect()
}

fn node_deps(pkg_json: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(pkg_json).unwrap_or_default();
    let mut deps = Vec::new();
    for section in &["dependencies", "devDependencies"] {
        if let Some(obj) = v.get(section).and_then(|d| d.as_object()) {
            for key in obj.keys().take(8) {
                if !deps.contains(key) {
                    deps.push(key.clone());
                }
            }
        }
    }
    deps.into_iter().take(12).collect()
}

fn json_value(json: &str, key: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get(key)?.as_str().map(|s| s.to_string())
}

fn src_structure(root: &Path) -> Vec<String> {
    let src = root.join("src");
    if !src.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&src)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() {
                format!("src/{}/", name)
            } else {
                format!("src/{}", name)
            }
        })
        .take(10)
        .collect()
}
