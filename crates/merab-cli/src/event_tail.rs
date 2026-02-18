use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use merab_core::{EventKind, ProgressEvent};

pub fn start_event_tail(event_file: &Path, running: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    let path = event_file.to_path_buf();
    thread::spawn(move || {
        let mut last_pos: u64 = 0;

        while running.load(Ordering::Relaxed) {
            if let Ok(file) = File::open(&path) {
                if let Ok(metadata) = file.metadata() {
                    if metadata.len() > last_pos {
                        if let Ok(mut file) = File::open(&path) {
                            let _ = file.seek(SeekFrom::Start(last_pos));
                            let reader = BufReader::new(file);

                            for line in reader.lines().flatten() {
                                if let Ok(event) = serde_json::from_str::<ProgressEvent>(&line) {
                                    display_event(&event);
                                    if event.kind == EventKind::Done {
                                        running.store(false, Ordering::Relaxed);
                                        return;
                                    }
                                }
                            }
                        }
                        if let Ok(file) = File::open(&path) {
                            if let Ok(metadata) = file.metadata() {
                                last_pos = metadata.len();
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    })
}

fn display_event(event: &ProgressEvent) {
    let persona_tag = event
        .persona
        .as_ref()
        .map(|p| p.to_uppercase())
        .unwrap_or_default();

    match event.kind {
        EventKind::Planning => {
            println!("[Planner] {}", event.message);
        }
        EventKind::PlanReady => {
            println!("[Plan] {}", event.message);
            println!();
        }
        EventKind::SubtaskStart => {
            println!("[{}] {}", persona_tag, event.message);
        }
        EventKind::Step => {
            println!("  → {}", event.message);
        }
        EventKind::SubtaskDone => {
            println!("[{}] ✓ Done", persona_tag);
            println!();
        }
        EventKind::Done => {
            println!("✓ Completado");
        }
        EventKind::Error => {
            eprintln!("! Error: {}", event.message);
        }
    }
}
