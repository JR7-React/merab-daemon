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
                                if let Some(event) = parse_event_line(&line) {
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
        EventKind::Retrying => {
            let tag = event
                .persona
                .as_ref()
                .map(|p| p.to_uppercase())
                .unwrap_or_default();
            println!("  [{}] ⟳ {}", tag, event.message);
        }
    }
}

pub(crate) fn parse_event_line(line: &str) -> Option<ProgressEvent> {
    if line.is_empty() {
        return None;
    }
    serde_json::from_str::<ProgressEvent>(line).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_thinking_event() {
        let line = r#"{"kind":"step","persona":"Coder","message":"Analizando..."}"#;
        let result = parse_event_line(line);
        assert!(result.is_some());
        let ev = result.unwrap();
        assert_eq!(ev.persona.as_deref(), Some("Coder"));
        assert_eq!(ev.kind, EventKind::Step);
    }

    #[test]
    fn test_parse_done_event() {
        let line = r#"{"kind":"done","message":"Pipeline completed"}"#;
        let result = parse_event_line(line);
        assert!(result.is_some());
        let ev = result.unwrap();
        assert_eq!(ev.kind, EventKind::Done);
        assert_eq!(ev.message, "Pipeline completed");
    }

    #[test]
    fn test_invalid_json_returns_none() {
        let result = parse_event_line("not json at all");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_line_returns_none() {
        let result = parse_event_line("");
        assert!(result.is_none());
    }
}
