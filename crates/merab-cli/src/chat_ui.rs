use std::io;
use std::env;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    style::{Color as CColor, Print, ResetColor, SetForegroundColor},
};
use merab_ai::ChatMessage;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use crate::client::ForgeClient;
use crate::git_utils::{self, GitFileStat};
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

/// En Windows/ConHost, `enable_raw_mode()` desactiva `ENABLE_QUICK_EDIT_MODE`,
/// que es la flag que permite seleccionar texto con el mouse. La restauramos aquí
/// y quitamos `ENABLE_MOUSE_INPUT` para que el host maneje el mouse (selección),
/// no la aplicación.
#[cfg(windows)]
fn restore_console_mouse_selection() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, STD_INPUT_HANDLE,
    };
    // ENABLE_QUICK_EDIT_MODE=0x0040, ENABLE_EXTENDED_FLAGS=0x0080, ENABLE_MOUSE_INPUT=0x0010
    const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
    const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;
    const ENABLE_MOUSE_INPUT: u32 = 0x0010;

    unsafe {
        let handle = GetStdHandle(STD_INPUT_HANDLE);
        if handle == INVALID_HANDLE_VALUE {
            return;
        }
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return;
        }
        // Restaurar ENABLE_QUICK_EDIT_MODE + ENABLE_EXTENDED_FLAGS
        // Quitar ENABLE_MOUSE_INPUT para no interceptar clics del usuario
        mode |= ENABLE_QUICK_EDIT_MODE | ENABLE_EXTENDED_FLAGS;
        mode &= !ENABLE_MOUSE_INPUT;
        SetConsoleMode(handle, mode);
    }
}

#[cfg(not(windows))]
fn restore_console_mouse_selection() {}

pub async fn start_chat_session(client: &ForgeClient) -> anyhow::Result<()> {
    // Show splash briefly before entering alternate screen
    print_splash()?;

    let status = client.get_system_status().await?;
    let model = status.ai.model.clone();
    let model_short = model.split('/').last().unwrap_or(&model).to_string();

    // SETUP TERMINAL: standard TUI hygiene
    enable_raw_mode()?;
    // Restaurar ENABLE_QUICK_EDIT_MODE para que el usuario pueda seleccionar
    // texto con el mouse en PowerShell/ConHost (crossterm la desactiva en raw mode).
    restore_console_mouse_selection();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut context: Vec<ChatMessage> = Vec::new();
    let mut messages: Vec<ChatMessage> = Vec::new();
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
    ).await;

    // RESTORE TERMINAL
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Print goodbye on the main screen after exiting TUI
    if res.is_ok() {
         println!("\n  Thanks for using Merab!\n");
    }
    
    res
}

fn print_splash() -> anyhow::Result<()> {
    // Only clear if we are not in alternate screen yet? 
    // Actually, let's keep it simple: print splash to standard stdout first, then enter TUI.
    // The splash will be visible for 800ms before TUI takes over.
    execute!(
        io::stdout(),
        // Clear(ClearType::All), // Don't clear all, just print splash
        SetForegroundColor(CColor::Cyan),
        Print(r#"
              ·✦    ✧    ✦·
           ✧·   · ✦ ·   ·✧
               ┌────────────┐
               │  ●     ●  │
               │    ╭──╯   │
               └────┬──┬───┘
             ┌──────┴──┴──────┐
        ✦·══╡   ░▒▓██▓▒░    ╞══·✦
             └──────┬──┬──────┘
                 ▒▓█┘  └█▓▒
               ▒▓██████████▓▒
            ░▒▓██████████████▓▒░
            ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄

"#),
        ResetColor,
    )?;
    
    execute!(
        io::stdout(),
        SetForegroundColor(CColor::Cyan),
        Print("  ███╗   ███╗███████╗██████╗  █████╗ ██████╗ \n"),
        Print("  ████╗ ████║██╔════╝██╔══██╗██╔══██╗██╔══██╗\n"),
        Print("  ██╔████╔██║█████╗  ██████╔╝███████║██████╔╝\n"),
        Print("  ██║╚██╔╝██║██╔══╝  ██╔══██╗██╔══██║██╔══██╗\n"),
        Print("  ██║ ╚═╝ ██║███████╗██║  ██║██║  ██║██████╔╝\n"),
        Print("  ╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝ \n"),
        ResetColor,
        SetForegroundColor(CColor::DarkGrey),
        Print("       AI Agent Runtime Engine\n\n"),
        ResetColor,
    )?;

    std::thread::sleep(std::time::Duration::from_millis(800));
    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    client: &ForgeClient,
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
    let mut sidebar_scrollbar_state = ScrollbarState::default().content_length(tasks.len() + repo_status.len() + 10);
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3), Constraint::Length(1)])
                .split(f.area());

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
                .split(chunks[0]);

            render_feed(f, main_chunks[0], messages, *scroll_offset);
            render_feed(f, main_chunks[0], messages, *scroll_offset);
            render_sidebar(f, main_chunks[1], model, tasks, *tokens_used, *is_processing, repo_status, *sidebar_scroll, &mut sidebar_scrollbar_state);
            render_input(f, chunks[1], input, *is_processing);
            render_status_bar(f, chunks[2], model);
        })?;

        let timeout = if *is_processing {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::from_millis(100)
        };

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    
                    if *is_processing {
                        continue;
                    }

                    match key.code {
                        // Handle Ctrl+C, Ctrl+D
                        KeyCode::Char('c') | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(());
                        }
                        KeyCode::Char(c) => {
                            input.push(c);
                        }
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Enter => {
                            if input.trim().is_empty() {
                                continue;
                            }
                            if input.starts_with("/quit") || input.starts_with("/q") || input.starts_with("/exit") {
                                return Ok(());
                            }
                            if input.starts_with("/reset") {
                                context.clear();
                                messages.clear();
                                tasks.retain(|(done, _)| *done);
                                *scroll_offset = 0;
                                input.clear();
                                continue;
                            }
                            if input.starts_with("/clear") {
                                messages.clear();
                                *scroll_offset = 0;
                                input.clear();
                                continue;
                            }

                            let user_msg = input.clone();
                            messages.push(ChatMessage::user(&user_msg));
                            tasks.push((false, format!("Process: {}", &user_msg[..user_msg.len().min(25)])));
                            input.clear();
                            *is_processing = true;
                            *scroll_offset = 0; // Reset scroll on new message

                            // Force redraw to show user message immediately
                            terminal.draw(|f| {
                                let chunks = Layout::default()
                                    .direction(Direction::Vertical)
                                    .constraints([Constraint::Min(1), Constraint::Length(3), Constraint::Length(1)])
                                    .split(f.area());
                                let main_chunks = Layout::default()
                                    .direction(Direction::Horizontal)
                                    .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
                                    .split(chunks[0]);
                                render_feed(f, main_chunks[0], messages, *scroll_offset);
                                render_sidebar(f, main_chunks[1], model, tasks, *tokens_used, true, repo_status, *sidebar_scroll, &mut sidebar_scrollbar_state);
                                render_input(f, chunks[1], input, true);
                                render_status_bar(f, chunks[2], model);
                            })?;

                            match process_message(client, context, &user_msg).await {
                                Ok((response, tokens)) => {
                                    messages.push(ChatMessage::assistant(&response));
                                    // Also update context for next turn
                                    context.push(ChatMessage::user(&user_msg));
                                    context.push(ChatMessage::assistant(&response));
                                    *tokens_used += tokens as u64;
                                    if let Some(last_task) = tasks.last_mut() {
                                        last_task.0 = true;
                                    }
                                }
                                Err(e) => {
                                    messages.push(ChatMessage::assistant(&format!("Error: {}", e)));
                                }
                            }
                            *is_processing = false;
                        }
                        // Sidebar scrolling (Alt+Up/Down)
                        KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                             *sidebar_scroll = sidebar_scroll.saturating_sub(1);
                        }
                        KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                             *sidebar_scroll = sidebar_scroll.saturating_add(1);
                        }
                        KeyCode::Up => {
                            if *scroll_offset > 0 {
                                *scroll_offset -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if *scroll_offset < messages.len() * 100 { // Allow scrolling down extensively
                                *scroll_offset += 1;
                            }
                        }
                        KeyCode::Esc => {
                            return Ok(());
                        }
                        KeyCode::PageUp => {
                             *scroll_offset = scroll_offset.saturating_sub(10);
                        }
                        KeyCode::PageDown => {
                             *scroll_offset = scroll_offset.saturating_add(10);
                        }
                         _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

fn render_feed(f: &mut ratatui::Frame, area: Rect, messages: &[ChatMessage], scroll: usize) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Feed ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("({} msgs)", messages.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let mut lines: Vec<Line> = vec![];

    // Calculate max width for content (area width minus padding and border)
    let max_width = area.width.saturating_sub(6) as usize;

    for msg in messages.iter().skip(scroll) {
        match &msg.role {
            merab_ai::ChatRole::User => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("╭── ", Style::default().fg(Color::Cyan)),
                    Span::styled("You", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]));
                for line in msg.content.lines().take(100) {
                    // Wrap long lines
                    let line_text = line.to_string();
                    let chars: Vec<char> = line_text.chars().collect();
                    for chunk in chars.chunks(max_width.max(20)) {
                        lines.push(Line::from(vec![
                            Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                            Span::raw(chunk.iter().collect::<String>()),
                        ]));
                    }
                }
                lines.push(Line::from("╰──"));
            }
            merab_ai::ChatRole::Assistant => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("╭── ", Style::default().fg(Color::Green)),
                    Span::styled("Merab", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]));
                for line in msg.content.lines().take(100) {
                    // Wrap long lines
                    let line_text = line.to_string();
                    let chars: Vec<char> = line_text.chars().collect();
                    for chunk in chars.chunks(max_width.max(20)) {
                        lines.push(Line::from(vec![
                            Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                            Span::raw(chunk.iter().collect::<String>()),
                        ]));
                    }
                }
                lines.push(Line::from("╰──"));
            }
            _ => {}
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_sidebar(
    f: &mut ratatui::Frame,
    area: Rect,
    model: &str,
    tasks: &[(bool, String)],
    tokens: u64,
    is_processing: bool,
    repo_status: &[GitFileStat],
    scroll: usize,
    scrollbar_state: &mut ScrollbarState,
) {
    let status_text = if is_processing { "● Processing..." } else { "● Ready" };
    let status_color = if is_processing { Color::Yellow } else { Color::Green };

    // Calculate max task name length based on sidebar width
    let max_task_len = area.width.saturating_sub(8) as usize;

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" MERAB", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" AI Agent Runtime", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Status", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(model, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("  Tokens: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", tokens)),
        ]),
        Line::from(vec![
            Span::styled("  Agents: ", Style::default().fg(Color::DarkGray)),
            Span::styled("3 running", Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tasks", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let mut all_lines = lines;
    for (done, task) in tasks.iter().take(6) {
        let check = if *done { "✓" } else { "·" };
        let style = if *done {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let truncated = if task.len() > max_task_len {
            format!("{}…", &task[..max_task_len.saturating_sub(1)])
        } else {
            task.clone()
        };
        all_lines.push(Line::from(vec![
            Span::styled(format!("  {} ", check), style),
            Span::styled(truncated, style),
        ]));
    }

    all_lines.push(Line::from(""));
    all_lines.push(Line::from(vec![
        Span::styled("▼ ", Style::default().fg(Color::Blue)),
        Span::styled("Modified Files", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
    ]));

    for stat in repo_status {
         let path_display = if stat.path.len() > max_task_len.saturating_sub(8) {
            format!("...{}", &stat.path[stat.path.len().min(8)..]) // Simple truncate for now
         } else {
            stat.path.clone()
         };
         
         let stats_part = format!("+{} -{}", stat.added, stat.removed);

         all_lines.push(Line::from(vec![
             Span::styled(format!("  {} ", path_display), Style::default().fg(Color::White)),
         ]));
         all_lines.push(Line::from(vec![
             Span::styled(format!("    {}", stats_part), Style::default().fg(Color::DarkGray)),
         ]));
    }

    // Pad with empty lines if needed to fill space or just let it be short
    
    // Apply scrolling
    let visible_lines_count = area.height.saturating_sub(2) as usize; // borders
    let total_lines = all_lines.len();
    *scrollbar_state = scrollbar_state.content_length(total_lines);
    
    let visible_lines: Vec<Line> = all_lines
        .into_iter()
        .skip(scroll)
        .take(visible_lines_count)
        .collect();

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let _inner_area = block.inner(area);
    let paragraph = Paragraph::new(visible_lines).block(block);
    f.render_widget(paragraph, area);

    // Render Scrollbar
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        area.inner(ratatui::layout::Margin { vertical: 0, horizontal: 0 }), // Overlay on right border
        scrollbar_state,
    );
}

fn render_input(f: &mut ratatui::Frame, area: Rect, input: &str, is_processing: bool) {
    let title = if is_processing {
        Line::from(vec![
            Span::styled(" Processing ", Style::default().fg(Color::Yellow)),
            Span::styled("...", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Input ", Style::default().fg(Color::Cyan)),
            Span::styled("(ESC quit, ↑↓/PgUp/PgDn scroll)", Style::default().fg(Color::DarkGray)),
        ])
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let cursor = if is_processing { "⋯" } else { "▎" };
    let lines = vec![Line::from(vec![
        Span::styled("❯ ", Style::default().fg(Color::Cyan)),
        Span::raw(input),
        Span::styled(cursor, Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
    ])];

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn render_status_bar(f: &mut ratatui::Frame, area: Rect, model: &str) {
    let cwd = env::current_dir().ok().and_then(|p| p.to_str().map(|s| s.to_string())).unwrap_or_default();
    
    let line = Line::from(vec![
        Span::styled(" ESC quit ", Style::default().fg(Color::DarkGray)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(model, Style::default().fg(Color::Yellow)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("📂 {}", cwd), Style::default().fg(Color::Blue)),
    ]);

    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Black));
    f.render_widget(paragraph, area);
}

async fn process_message(
    client: &ForgeClient,
    context: &mut Vec<ChatMessage>,
    input: &str,
) -> anyhow::Result<(String, usize)> {
    let mut messages = context.clone();
    messages.push(ChatMessage::user(input));

    let max_steps = 10;

    for _step in 0..max_steps {
        let (current_msg, prev_context) = if messages.len() <= 1 {
            (input.to_string(), context.clone())
        } else {
            let last = messages.last().cloned().unwrap_or(ChatMessage::user(""));
            let prev = messages[..messages.len().saturating_sub(1)].to_vec();
            (last.content.clone(), prev)
        };

        let resp = client.ai_chat(&current_msg, prev_context).await?;

        if let Some(tool_call) = &resp.tool_call {
            messages.push(ChatMessage::assistant(&resp.content));

            let result = client.ai_execute_tool(&tool_call.name, tool_call.arguments.clone()).await?;
            let result_str = serde_json::to_string(&result)?;
            messages.push(ChatMessage::user(format!("Tool '{}': {}", tool_call.name, result_str)));
            continue;
        }

        let tokens = resp.content.len();
        return Ok((resp.content, tokens));
    }

    Ok(("Max steps reached".to_string(), 0))
}
