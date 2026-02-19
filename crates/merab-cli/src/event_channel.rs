use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use merab_core::ProgressEvent;

use crate::event_tail::parse_event_line;

pub fn tail_events_to_channel(
    path: PathBuf,
    tx: std::sync::mpsc::Sender<ProgressEvent>,
    running: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
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
                                    let is_done = event.kind == merab_core::EventKind::Done;
                                    let _ = tx.send(event);
                                    if is_done {
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
            std::thread::sleep(Duration::from_millis(80));
        }
    })
}
