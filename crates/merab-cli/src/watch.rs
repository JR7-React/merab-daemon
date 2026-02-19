use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use merab_core::pricing::estimate_cost;

use crate::client::MerabClient;

pub struct WatchConfig {
    pub pattern: String,
    pub task: String,
    pub test: bool,
    pub debounce: u64,
    pub quiet: bool,
}

pub async fn run_watch(client: &MerabClient, config: WatchConfig) -> Result<()> {
    use std::sync::atomic::Ordering;

    let project_path = std::env::current_dir()?;

    println!(
        "[merab watch] Observando {} en {}",
        config.pattern,
        project_path.display()
    );
    println!("[merab watch] Tarea: \"{}\"", config.task);
    println!("[merab watch] Presiona Ctrl+C para detener.\n");

    let (tx, rx) = mpsc::channel::<PathBuf>();

    let pattern_clone = config.pattern.clone();
    let project_clone = project_path.clone();
    std::thread::spawn(move || {
        watch_files(&project_clone, &pattern_clone, tx);
    });

    let debounce_dur = Duration::from_secs(config.debounce);
    let mut last_trigger = Instant::now() - debounce_dur;
    let mut pending_files: Vec<PathBuf> = Vec::new();

    loop {
        loop {
            match rx.try_recv() {
                Ok(path) => pending_files.push(path),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        if !pending_files.is_empty() && last_trigger.elapsed() >= debounce_dur {
            let changed: Vec<String> = pending_files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            pending_files.clear();
            last_trigger = Instant::now();

            if !config.quiet {
                println!(
                    "[merab watch] Cambio detectado: {}",
                    changed.join(", ")
                );
            }

            let full_task = format!(
                "{}\n\nArchivos modificados:\n{}",
                config.task,
                changed
                    .iter()
                    .map(|f| format!("- {}", f))
                    .collect::<Vec<_>>()
                    .join("\n")
            );

            let event_file = std::env::temp_dir()
                .join(format!("merab-watch-{}.jsonl", std::process::id()));
            let event_file_str = event_file.to_string_lossy().to_string();

            let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let tail_handle = if !config.quiet {
                Some(crate::event_tail::start_event_tail(
                    &event_file,
                    running.clone(),
                ))
            } else {
                None
            };

            let result = if config.test {
                client
                    .ai_orchestrate_with_tests(&full_task, &event_file_str)
                    .await
            } else {
                client
                    .ai_orchestrate_stream(&full_task, &event_file_str)
                    .await
            };

            running.store(false, Ordering::Relaxed);
            if let Some(handle) = tail_handle {
                let _ = handle.join();
            }
            let _ = std::fs::remove_file(&event_file);

            match result {
                Ok(resp) => {
                    if !config.quiet {
                        if let Some(usage) = &resp.usage {
                            println!(
                                "Tokens: {} | Costo: ${:.4}",
                                usage.input + usage.output,
                                estimate_cost(&usage.model, usage.input, usage.output)
                                    .unwrap_or(0.0)
                            );
                        }
                        println!();
                    }
                }
                Err(e) => eprintln!("[merab watch] Error: {}", e),
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn watch_files(
    project_path: &std::path::Path,
    pattern: &str,
    tx: Sender<PathBuf>,
) {
    use std::collections::HashMap;

    let mut last_modified: HashMap<PathBuf, SystemTime> = HashMap::new();
    let full_pattern = project_path.join(pattern).to_string_lossy().to_string();

    loop {
        if let Ok(paths) = glob::glob(&full_pattern) {
            for path in paths.flatten() {
                if let Ok(meta) = std::fs::metadata(&path)
                    && let Ok(modified) = meta.modified()
                {
                    let prev = last_modified.get(&path).copied();
                    if prev != Some(modified) {
                        if prev.is_some() {
                            let _ = tx.send(path.clone());
                        }
                        last_modified.insert(path, modified);
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
