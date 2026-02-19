use std::env;

use crossterm::execute;
use crossterm::style::{Color as CColor, Print, ResetColor, SetForegroundColor};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::git_utils::GitFileStat;

// ── Windows: restaurar selección de texto con mouse ──────────────────────────

/// En Windows/ConHost, `enable_raw_mode()` desactiva `ENABLE_QUICK_EDIT_MODE`,
/// que es la flag que permite seleccionar texto con el mouse. La restauramos aquí
/// y quitamos `ENABLE_MOUSE_INPUT` para que el host maneje el mouse (selección),
/// no la aplicación.
#[cfg(windows)]
pub fn restore_console_mouse_selection() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, STD_INPUT_HANDLE,
    };
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
        mode |= ENABLE_QUICK_EDIT_MODE | ENABLE_EXTENDED_FLAGS;
        mode &= !ENABLE_MOUSE_INPUT;
        SetConsoleMode(handle, mode);
    }
}

#[cfg(not(windows))]
pub fn restore_console_mouse_selection() {}

// ── Splash ────────────────────────────────────────────────────────────────────

pub fn print_splash() -> anyhow::Result<()> {
    execute!(
        std::io::stdout(),
        SetForegroundColor(CColor::Cyan),
        Print(
            r#"
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

"#
        ),
        ResetColor,
    )?;

    execute!(
        std::io::stdout(),
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

// ── Widget renderers ──────────────────────────────────────────────────────────

pub fn render_feed(
    f: &mut ratatui::Frame,
    area: Rect,
    messages: &[merab_ai::ChatMessage],
    scroll: usize,
) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " Feed ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({} msgs)", messages.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let mut lines: Vec<Line> = vec![];
    let max_width = area.width.saturating_sub(6) as usize;

    for msg in messages.iter() {
        match &msg.role {
            merab_ai::ChatRole::User => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("╭── ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        "You",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                for line in msg.content.lines().take(100) {
                    let chars: Vec<char> = line.chars().collect();
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
                    Span::styled(
                        "Merab",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                for line in msg.content.lines().take(100) {
                    let chars: Vec<char> = line.chars().collect();
                    for chunk in chars.chunks(max_width.max(20)) {
                        lines.push(Line::from(vec![
                            Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                            Span::raw(chunk.iter().collect::<String>()),
                        ]));
                    }
                }
                lines.push(Line::from("╰──"));
            }
            merab_ai::ChatRole::System => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("╭── ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        "System",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                for line in msg.content.lines().take(100) {
                    let chars: Vec<char> = line.chars().collect();
                    for chunk in chars.chunks(max_width.max(20)) {
                        lines.push(Line::from(vec![
                            Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                            Span::raw(chunk.iter().collect::<String>()),
                        ]));
                    }
                }
                lines.push(Line::from("╰──"));
            }
        }
    }

    let total_lines = lines.len() as u16;
    let view_height = area.height.saturating_sub(2);
    let max_scroll = total_lines.saturating_sub(view_height);
    let actual_scroll = max_scroll.saturating_sub(scroll as u16);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((actual_scroll, 0));
    f.render_widget(paragraph, area);
}

pub fn render_sidebar(
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
    let status_text = if is_processing {
        "● Processing..."
    } else {
        "● Ready"
    };
    let status_color = if is_processing {
        Color::Yellow
    } else {
        Color::Green
    };
    let max_task_len = area.width.saturating_sub(8) as usize;

    let mut all_lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            " MERAB",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            " AI Agent Runtime",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Status",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
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
        Line::from(vec![Span::styled(
            status_text,
            Style::default().fg(status_color),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Tasks",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
    ];

    for (done, task) in tasks.iter() {
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
        Span::styled(
            "Modified Files",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    for stat in repo_status {
        let path_display = if stat.path.len() > max_task_len.saturating_sub(8) {
            format!("...{}", &stat.path[stat.path.len().min(8)..])
        } else {
            stat.path.clone()
        };
        all_lines.push(Line::from(vec![Span::styled(
            format!("  {} ", path_display),
            Style::default().fg(Color::White),
        )]));
        all_lines.push(Line::from(vec![Span::styled(
            format!("    +{} -{}", stat.added, stat.removed),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    let visible_lines_count = area.height.saturating_sub(2) as usize;
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
    let paragraph = Paragraph::new(visible_lines).block(block);
    f.render_widget(paragraph, area);

    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        area.inner(ratatui::layout::Margin {
            vertical: 0,
            horizontal: 0,
        }),
        scrollbar_state,
    );
}

pub fn render_input(f: &mut ratatui::Frame, area: Rect, input: &str, is_processing: bool) {
    let title = if is_processing {
        Line::from(vec![
            Span::styled(" Processing ", Style::default().fg(Color::Yellow)),
            Span::styled("...", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Input ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "(ESC quit, ↑↓/PgUp/PgDn scroll)",
                Style::default().fg(Color::DarkGray),
            ),
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
        Span::styled(
            cursor,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ])];

    f.render_widget(Paragraph::new(lines).block(block), area);
}

pub fn render_status_bar(f: &mut ratatui::Frame, area: Rect, model: &str) {
    let cwd = env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_default();

    let line = Line::from(vec![
        Span::styled(" ESC quit ", Style::default().fg(Color::DarkGray)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(model, Style::default().fg(Color::Yellow)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("📂 {}", cwd), Style::default().fg(Color::Blue)),
    ]);

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Black)),
        area,
    );
}

pub fn render_slash_popup(
    f: &mut ratatui::Frame,
    area: Rect,
    completions: &[crate::slash_commands::SlashCommandDef],
    selected: usize,
) {
    if completions.is_empty() {
        return;
    }

    let max_visible = 8.min(completions.len());
    let popup_height = (max_visible + 2) as u16;
    let popup_width = 50;

    let popup_area = Rect {
        x: area.x,
        y: area.y.saturating_sub(popup_height),
        width: popup_width.min(area.width),
        height: popup_height,
    };

    let block = Block::default()
        .title(" Commands ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines = vec![];
    for (i, cmd) in completions.iter().take(max_visible).enumerate() {
        let style = if i == selected {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("/{}", cmd.name), style),
            Span::styled(format!(" - {}", cmd.description), style),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, popup_area);
}
