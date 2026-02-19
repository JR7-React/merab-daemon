use std::io;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use merab_ai::{ChatMessage, ChatRole};
use merab_core::EventKind;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::ScrollbarState,
    Terminal,
};

use crate::event_channel;
use crate::chat_render::{render_feed, render_input, render_sidebar, render_status_bar};
use crate::slash_commands::{self, SlashCommandResult};
use crate::client::MerabClient;
use crate::git_utils::GitFileStat;

/// Returns `true` if the caller should exit the event loop.
#[allow(clippy::too_many_arguments)]
pub async fn handle_enter(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    client: &MerabClient,
    conv_id: &str,
    context: &mut Vec<ChatMessage>,
    messages: &mut Vec<ChatMessage>,
    input: &mut String,
    model: &str,
    tasks: &mut Vec<(bool, String)>,
    tokens_used: &mut u64,
    scroll_offset: &mut usize,
    sidebar_scroll: &mut usize,
    is_processing: &mut bool,
    repo_status: &[GitFileStat],
    sidebar_scrollbar_state: &mut ScrollbarState,
) -> anyhow::Result<bool> {
    if input.trim().is_empty() {
        return Ok(false);
    }

    // Handle slash commands
    if input.starts_with('/') {
        if let Some((cmd, args)) = slash_commands::parse_slash_input(input) {
            match slash_commands::resolve_command(cmd) {
                Some(name) => {
                    let result = slash_commands::execute_slash_command(name, args, client).await;
                    match result {
                        SlashCommandResult::Exit => return Ok(true),
                        SlashCommandResult::Output(lines) => {
                            messages.push(ChatMessage::system(&lines.join("\n")));
                        }
                        SlashCommandResult::Error(msg) => {
                            messages.push(ChatMessage::system(&format!("Error: {msg}")));
                        }
                        SlashCommandResult::Silent => {
                            // Handle clear and reset specially
                            if name == "clear" {
                                messages.clear();
                                *scroll_offset = 0;
                            } else if name == "reset" {
                                context.clear();
                                messages.clear();
                                tasks.retain(|(done, _)| *done);
                                *scroll_offset = 0;
                            }
                        }
                    }
                    input.clear();
                    return Ok(false);
                }
                None => {
                    messages.push(ChatMessage::system(&format!(
                        "Unknown command: /{}. Type /help for available commands.",
                        cmd
                    )));
                    input.clear();
                    return Ok(false);
                }
            }
        }
    }

    let user_msg = input.clone();
    messages.push(ChatMessage::user(&user_msg));
    tasks.push((false, format!("Process: {}", &user_msg[..user_msg.len().min(25)])));
    input.clear();
    *is_processing = true;
    *scroll_offset = 0;

    // Helper closure to redraw the terminal
    let redraw = |terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
                  messages: &[ChatMessage],
                  input: &str,
                  model: &str,
                  tasks: &[(bool, String)],
                  tokens_used: u64,
                  repo_status: &[GitFileStat],
                  scroll_offset: usize,
                  sidebar_scroll: usize,
                  sidebar_scrollbar_state: &mut ScrollbarState| {
        let _ = terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3), Constraint::Length(1)])
                .split(f.area());
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
                .split(chunks[0]);
            let mut tasks_vec: Vec<(bool, String)> = tasks.to_vec();
            render_feed(f, main_chunks[0], messages, scroll_offset);
            render_sidebar(f, main_chunks[1], model, &mut tasks_vec, tokens_used, true, repo_status, sidebar_scroll, sidebar_scrollbar_state);
            render_input(f, chunks[1], input, true);
            render_status_bar(f, chunks[2], model);
        });
    };

    // Force initial redraw
    redraw(terminal, messages, input, model, tasks, *tokens_used, repo_status, *scroll_offset, *sidebar_scroll, sidebar_scrollbar_state);

    // --- Unified Agentic Orchestration ---

    // 1. Build prompt with context history
    let full_prompt = if context.is_empty() {
        user_msg.clone()
    } else {
        let history = context.iter()
            .map(|m| format!("[{}]: {}", match m.role {
                merab_ai::ChatRole::User => "User",
                merab_ai::ChatRole::Assistant => "Assistant",
                _ => "System",
            }, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Contexto previo de la conversación:\n{}\n\nTarea actual:\n{}", history, user_msg)
    };

    // 2. Create temp file for events
    let event_file = std::env::temp_dir()
        .join(format!("merab-chat-events-{}.jsonl", std::process::id()));
    let event_file_str = event_file.to_string_lossy().to_string();

    // 3. Spawn orchestration in background task
    let client_url = client.url().to_string();
    let task_str = full_prompt.clone();
    let ef = event_file_str.clone();
    
    // We use a separate thread for the client call because tokio::spawn might be tricky 
    // if we are not in a full tokio runtime context for proper joining, 
    // but run_app is likely called from tokio::main.
    let orchestrate_handle = tokio::spawn(async move {
        // Create a new client instance for this task to avoid lifetime issues
        if let Ok(c) = MerabClient::new(&client_url) {
            c.ai_orchestrate_stream(&task_str, &ef).await.ok()
        } else {
            None
        }
    });

    // 4. Start event tailing to channel
    let (tx, rx) = mpsc::channel::<merab_core::ProgressEvent>();
    let running = Arc::new(AtomicBool::new(true));
    let tail_handle = event_channel::tail_events_to_channel(
        event_file.clone(),
        tx,
        running.clone(),
    );

    // 5. UI Loop: poll events + terminal input
    
    loop {

        // Process pipeline events (non-blocking)
        while let Ok(event) = rx.try_recv() {
            match event.kind {
                EventKind::Planning => {
                   // Update the last task (Process: ...) to Planning
                   if let Some(last) = tasks.last_mut() {
                       if last.1.starts_with("Process:") {
                           last.1 = "Planning...".to_string();
                       }
                   }
                }
                EventKind::PlanReady => {
                    if let Some(last) = tasks.last_mut() {
                         if last.1 == "Planning..." {
                             last.0 = true;
                         }
                    }
                }
                EventKind::SubtaskStart => {
                    let persona = event.persona.as_deref().unwrap_or("Agent");
                    let msg_short: String = event.message.chars().take(40).collect();
                    tasks.push((false, format!("[{}] {}", persona, msg_short)));
                    // Also scroll sidebar to bottom
                    if tasks.len() > 20 {
                         *sidebar_scroll = tasks.len() - 20;
                    }
                }
                EventKind::Step => {
                    let persona = event.persona.as_deref().unwrap_or("Agent");
                    messages.push(ChatMessage::system(&format!("[{}] {}", persona, event.message)));
                    // Scroll feed to bottom? managed by render_feed usually
                }
                EventKind::SubtaskDone => {
                    if let Some(last) = tasks.last_mut() {
                        last.0 = true;
                    }
                }
                EventKind::Error => {
                    messages.push(ChatMessage::system(&format!("Error: {}", event.message)));
                    if let Some(last) = tasks.last_mut() {
                        last.1 = format!("✗ {}", last.1);
                    }
                }
                EventKind::Retrying => {
                    messages.push(ChatMessage::system(&format!("⟳ {}", event.message)));
                }
                EventKind::Done => { /* handled by orchestrate future result */ }
            }
        }

        redraw(terminal, messages, input, model, tasks, *tokens_used, repo_status, *scroll_offset, *sidebar_scroll, sidebar_scrollbar_state);

        // Check for terminal input (Ctrl+C to cancel)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                 if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    running.store(false, Ordering::Relaxed);
                    orchestrate_handle.abort();
                    messages.push(ChatMessage::system("Proceso cancelado por usuario."));
                    *is_processing = false;
                     let _ = tail_handle.join();
                     let _ = std::fs::remove_file(&event_file);
                    return Ok(false);
                }
                // Handle scrolling?
                match key.code {
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                        *sidebar_scroll = sidebar_scroll.saturating_sub(1);
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                         *sidebar_scroll = sidebar_scroll.saturating_add(1);
                    }
                    KeyCode::Up => if *scroll_offset > 0 { *scroll_offset -= 1; },
                    KeyCode::Down => *scroll_offset = scroll_offset.saturating_add(1),
                    _ => {}
                }
            }
        }

        if orchestrate_handle.is_finished() {
            break;
        }
    }

    // 6. Cleanup and results
    running.store(false, Ordering::Relaxed);
    let _ = tail_handle.join();
    let _ = std::fs::remove_file(&event_file);

    match orchestrate_handle.await {
        Ok(Some(resp)) => {
            messages.push(ChatMessage::assistant(&resp.content));
            context.push(ChatMessage::user(&user_msg));
            context.push(ChatMessage::assistant(&resp.content));

            if let Some(usage) = &resp.usage {
                *tokens_used += usage.input + usage.output;
                messages.push(ChatMessage::system(&format!(
                    "Tokens: {} in / {} out | Costo: ${:.4} | Modelo: {}",
                    usage.input, usage.output, 
                    merab_core::estimate_cost(&usage.model, usage.input, usage.output).unwrap_or(0.0),
                    usage.model
                )));
            }
            if let Some(artifacts) = &resp.artifacts {
                 let mut artifact_lines = Vec::new();
                for f in &artifacts.files_created { artifact_lines.push(format!("  + {}", f)); }
                for f in &artifacts.files_modified { artifact_lines.push(format!("  ~ {}", f)); }
                if !artifact_lines.is_empty() {
                    messages.push(ChatMessage::system(&format!("Archivos:\n{}", artifact_lines.join("\n"))));
                }
            }
            
            // Persist
             if !conv_id.is_empty() {
                let _ = client.conv_add_message(conv_id, "user", &user_msg).await;
                 let _ = client.conv_add_message(conv_id, "assistant", &resp.content).await;
            }
        }
        _ => {
             messages.push(ChatMessage::system("Error: La orquestación falló o no retornó respuesta."));
        }
    }
    
    if let Some(last) = tasks.last_mut() { last.0 = true; }
    *is_processing = false;
    Ok(false)
}
