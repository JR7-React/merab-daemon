use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use merab_ai::ChatMessage;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::ScrollbarState,
    Terminal,
};

use crate::chat_render::{
    render_feed, render_input, render_sidebar, render_status_bar, restore_console_mouse_selection,
    print_splash, render_slash_popup,
};
use crate::slash_commands;
use crate::client::MerabClient;
use crate::git_utils::{self, GitFileStat};

#[allow(dead_code)]
pub async fn start_chat_session(client: &MerabClient) -> anyhow::Result<()> {
    start_chat_session_with_history(client, "", Vec::new()).await
}

pub async fn start_chat_session_with_history(
    client: &MerabClient,
    conv_id: &str,
    history: Vec<ChatMessage>,
) -> anyhow::Result<()> {
    print_splash()?;

    let status = client.get_system_status().await?;
    let model = status.ai.model.clone();
    let model_short = model.split('/').last().unwrap_or(&model).to_string();

    enable_raw_mode()?;
    restore_console_mouse_selection();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut context: Vec<ChatMessage> = Vec::new();
    let mut messages: Vec<ChatMessage> = history;
    let mut input = String::new();
    let mut tasks: Vec<(bool, String)> = vec![
        (true, "Initialize environment".to_string()),
        (true, "Load MCP agents".to_string()),
        (false, "Process request".to_string()),
    ];
    let mut tokens_used: u64 = 0;
    let mut scroll_offset: usize = 0;
    let mut sidebar_scroll_offset: usize = 0;
    let mut is_processing = false;
    let repo_status = git_utils::get_repo_status();

    let res = run_app(
        &mut terminal,
        client,
        conv_id,
        &mut context,
        &mut messages,
        &mut input,
        &model_short,
        &mut tasks,
        &mut tokens_used,
        &mut scroll_offset,
        &mut sidebar_scroll_offset,
        &mut is_processing,
        &repo_status,
    )
    .await;

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if res.is_ok() {
        println!("\n  Thanks for using Merab!\n");
    }
    res
}

async fn run_app(
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
) -> anyhow::Result<()> {
    let mut sidebar_scrollbar_state =
        ScrollbarState::default().content_length(tasks.len() + repo_status.len() + 10);
    
    // Slash command completion state
    let mut slash_completions: Vec<slash_commands::SlashCommandDef> = Vec::new();
    let mut slash_selected: usize = 0;

    loop {
        // Update completions based on current input
        if input.starts_with('/') && input.len() > 1 {
            slash_completions = slash_commands::get_completions(input);
            if slash_selected >= slash_completions.len() {
                slash_selected = 0;
            }
        } else {
            slash_completions.clear();
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .split(f.area());

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
                .split(chunks[0]);

            render_feed(f, main_chunks[0], messages, *scroll_offset);
            render_sidebar(
                f,
                main_chunks[1],
                model,
                tasks,
                *tokens_used,
                *is_processing,
                repo_status,
                *sidebar_scroll,
                &mut sidebar_scrollbar_state,
            );
            render_input(f, chunks[1], input, *is_processing);
            
            // Render slash command popup if active
            if !slash_completions.is_empty() {
                render_slash_popup(f, chunks[1], &slash_completions, slash_selected);
            }
            
            render_status_bar(f, chunks[2], model);
        })?;

        let timeout = if *is_processing {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::from_millis(100)
        };

        if !event::poll(timeout)? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press || *is_processing {
                continue;
            }

            match key.code {
                KeyCode::Char('c') | KeyCode::Char('d')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    slash_selected = 0;
                }
                KeyCode::Backspace => {
                    input.pop();
                    slash_selected = 0;
                }
                KeyCode::Tab => {
                    // Accept current completion
                    if !slash_completions.is_empty() {
                        let cmd = &slash_completions[slash_selected];
                        input.clear();
                        input.push('/');
                        input.push_str(&cmd.name);
                        if cmd.needs_args {
                            input.push(' ');
                        }
                        slash_completions.clear();
                    }
                }
                KeyCode::Down if !slash_completions.is_empty() => {
                    slash_selected = (slash_selected + 1).min(slash_completions.len() - 1);
                }
                KeyCode::Up if !slash_completions.is_empty() => {
                    slash_selected = slash_selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    if crate::chat_handlers::handle_enter(

                        terminal,
                        client,
                        conv_id,
                        context,
                        messages,
                        input,
                        model,
                        tasks,
                        tokens_used,
                        scroll_offset,
                        sidebar_scroll,
                        is_processing,
                        repo_status,
                        &mut sidebar_scrollbar_state,
                    )
                    .await?
                    {
                        return Ok(());
                    }
                }
                KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                    *sidebar_scroll = sidebar_scroll.saturating_sub(1);
                }
                KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                    *sidebar_scroll = sidebar_scroll.saturating_add(1);
                }
                KeyCode::Up => {
                    *scroll_offset = scroll_offset.saturating_add(1);
                }
                KeyCode::Down => {
                    if *scroll_offset > 0 {
                        *scroll_offset -= 1;
                    }
                }
                KeyCode::Esc => return Ok(()),
                KeyCode::PageUp => {
                    *scroll_offset = scroll_offset.saturating_add(10);
                }
                KeyCode::PageDown => {
                    *scroll_offset = scroll_offset.saturating_sub(10);
                }
                _ => {}
            }
        }
    }
}


