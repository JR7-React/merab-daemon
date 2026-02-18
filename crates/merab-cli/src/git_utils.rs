use std::process::Command;


#[derive(Debug, Clone)]
pub struct GitFileStat {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

pub fn get_repo_status() -> Vec<GitFileStat> {
    let mut stats = Vec::new();

    // Run git diff --numstat to get changes
    if let Ok(output) = Command::new("git")
        .args(&["diff", "--numstat"])
        .output() 
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    // Handle binary files which might have "-" instead of numbers
                    let added = parts[0].parse().unwrap_or(0);
                    let removed = parts[1].parse().unwrap_or(0);
                    let path = parts[2..].join(" "); // Handle spaces in paths if any, though numstat usually separates by tab/space

                    stats.push(GitFileStat {
                        path,
                        added,
                        removed,
                    });
                }
            }
        }
    }
    
    // Also get untracked/added but not committed files if needed?
    // For now --numstat covers modified tracked files. 
    // To be comprehensive like the image (which usually shows cached+uncached), 
    // we might want `git diff HEAD --numstat` but that includes committed things if we aren't careful.
    // Let's stick to working directory changes for now as it's most useful for "what did I just do".
    
    stats
}
