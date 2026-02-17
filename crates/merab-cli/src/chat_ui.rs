use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
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

pub async fn start_chat_session(client: &ForgeClient) -> anyhow::Result<()> {
    print_splash()?;

    let status = client.get_system_status().await?;
    let model = status.ai.model.clone();
    let model_short = model.split('/').last().unwrap_or(&model).to_string();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All))?;
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
    ).await;

    disable_raw_mode()?;
    terminal.show_cursor()?;

    println!("\n  Thanks for using Merab!\n");
    res
}

fn print_splash() -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        Clear(ClearType::All),
        Print("\x1b[H"),
        SetForegroundColor(CColor::Cyan),
        Print(r#"
              ·✦    ✧    ✦·
           ✧·   · ✦ ·   ·✧
               ┌────────────┐
               │  ●     ●   │
               │    ╭──╯    │
               └────┬──┬────┘
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
) -> anyhow::Result<()> {
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
            render_sidebar(f, main_chunks[1], model, tasks, *tokens_used);
            render_input(f, chunks[1], input);
            render_status_bar(f, chunks[2], model);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                            return Ok(());
                        }
                        input.push(c);
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Enter => {
                        if input.trim().is_empty() {
                            continue;
                        }
                        if input.starts_with("/quit") || input.starts_with("/q") {
                            return Ok(());
                        }
                        if input.starts_with("/reset") {
                            context.clear();
                            messages.clear();
                            tasks.retain(|(done, _)| *done);
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

                        match process_message(client, context, &user_msg).await {
                            Ok((response, tokens)) => {
                                messages.push(ChatMessage::assistant(&response));
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
                    }
                    KeyCode::Up => {
                        if *scroll_offset > 0 {
                            *scroll_offset -= 1;
                        }
                    }
                    KeyCode::Down => {
                        *scroll_offset += 1;
                    }
                    KeyCode::Esc => {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
}

fn render_feed(f: &mut ratatui::Frame, area: Rect, messages: &[ChatMessage], scroll: usize) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Feed ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("({} messages)", messages.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let mut lines: Vec<Line> = vec![];

    for msg in messages.iter().skip(scroll) {
        match &msg.role {
            merab_ai::ChatRole::User => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("╭── ", Style::default().fg(Color::Cyan)),
                    Span::styled("You", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]));
                for line in msg.content.lines().take(50) {
                    lines.push(Line::from(vec![
                        Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                        Span::raw(line),
                    ]));
                }
                lines.push(Line::from("╰──"));
            }
            merab_ai::ChatRole::Assistant => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("╭── ", Style::default().fg(Color::Green)),
                    Span::styled("Merab", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]));
                for line in msg.content.lines().take(50) {
                    lines.push(Line::from(vec![
                        Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                        Span::raw(line),
                    ]));
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
) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));

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
        all_lines.push(Line::from(vec![
            Span::styled(format!("  {} ", check), style),
            Span::styled(
                if task.len() > 18 { &task[..18] } else { task },
                style,
            ),
        ]));
    }

    all_lines.push(Line::from(""));
    all_lines.push(Line::from(vec![
        Span::styled("◆ ", Style::default().fg(Color::Cyan)),
        Span::styled("Commands", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    all_lines.push(Line::from(vec![
        Span::styled("  /help /reset /quit", Style::default().fg(Color::DarkGray)),
    ]));
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(vec![
        Span::styled("● ", Style::default().fg(Color::Green)),
        Span::styled("merab v0.1.0", Style::default().fg(Color::DarkGray)),
    ]));

    let paragraph = Paragraph::new(all_lines).block(block);
    f.render_widget(paragraph, area);
}

fn render_input(f: &mut ratatui::Frame, area: Rect, input: &str) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Input ", Style::default().fg(Color::Cyan)),
            Span::styled("(ESC quit, ↑↓ scroll)", Style::default().fg(Color::DarkGray)),
        ]))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let lines = vec![Line::from(vec![
        Span::styled("❯ ", Style::default().fg(Color::Cyan)),
        Span::raw(input),
        Span::styled("▎", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
    ])];

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn render_status_bar(f: &mut ratatui::Frame, area: Rect, model: &str) {
    let progress = "████░░░░░░";

    let line = Line::from(vec![
        Span::styled(progress, Style::default().fg(Color::Cyan)),
        Span::styled(" Ready ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(model, Style::default().fg(Color::Yellow)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("ESC quit", Style::default().fg(Color::DarkGray)),
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

    for step in 0..max_steps {
        let (current_msg, prev_context) = if step == 0 {
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
