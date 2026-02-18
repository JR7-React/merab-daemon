use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use merab_core::SystemStatus;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::io;
use std::time::{Duration, Instant};

pub mod splash;

pub struct TuiApp {
    pub status: Option<SystemStatus>,
    pub should_quit: bool,
    pub show_splash: bool,
    splash_start: Instant,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            status: None,
            should_quit: false,
            show_splash: true,
            splash_start: Instant::now(),
        }
    }

    pub fn update_status(&mut self, status: SystemStatus) {
        self.status = Some(status);
    }
}

const SPLASH_DURATION: Duration = Duration::from_secs(3);

pub async fn run_tui<F, Fut>(mut app: TuiApp, mut tick_fn: F) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<SystemStatus>>,
{
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(500);

    loop {
        if !app.show_splash
            && let Ok(new_status) = tick_fn().await
        {
            app.update_status(new_status);
        }

        if app.show_splash {
            if app.splash_start.elapsed() >= SPLASH_DURATION {
                app.show_splash = false;
            } else {
                terminal.draw(splash_ui)?;
            }
        }

        if !app.show_splash {
            terminal.draw(|f| ui(f, &app))?;
        }

        if event::poll(tick_rate)?
            && let Event::Key(key) = event::read()?
        {
            if app.show_splash {
                app.show_splash = false;
                continue;
            }
            if let KeyCode::Char('q') = key.code {
                break;
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(())
}

/// Centra un rect de `width x height` dentro de `area`.
fn center_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn splash_ui(f: &mut ratatui::Frame) {
    let area = f.area();

    // Fondo negro
    f.render_widget(Clear, area);
    let bg = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(bg, area);

    let lines = splash::build_splash_lines();
    let content_height = lines.len() as u16;
    let content_width = 56; // ancho max del art

    let centered = center_rect(area, content_width, content_height);

    let splash = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().bg(Color::Black));

    f.render_widget(splash, centered);
}

fn ui(f: &mut ratatui::Frame, app: &TuiApp) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main Body
            Constraint::Length(3), // Footer
        ])
        .split(size);

    // 1. Header con branding rojo
    let header = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White));

    let uptime = app
        .status
        .as_ref()
        .map(|s| s.node_info.uptime_seconds)
        .unwrap_or(0);
    let version = app
        .status
        .as_ref()
        .map(|s| s.node_info.version.as_str())
        .unwrap_or("?");

    let header_line = Line::from(vec![
        Span::styled(" \u{2692} ", Style::default().fg(Color::Rgb(204, 0, 0))),
        Span::styled("MERAB", Style::default().fg(Color::Rgb(204, 0, 0))),
        Span::styled(
            format!(" v{} | Uptime: {}s | [Q] Quit", version, uptime),
            Style::default().fg(Color::White),
        ),
    ]);
    f.render_widget(Paragraph::new(header_line).block(header), chunks[0]);

    // 2. Main Body (Agents & Stats)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    // Agents List
    let agents_block = Block::default()
        .borders(Borders::ALL)
        .title(" Active Agents ");
    if let Some(status) = &app.status {
        let items: Vec<ListItem> = status
            .agents
            .iter()
            .map(|a| {
                let mem_mb = a.memory_usage_bytes / (1024 * 1024);
                let limit_mb = a.memory_limit_bytes / (1024 * 1024);
                let color = if a.status == merab_core::AgentStatus::Running {
                    Color::Green
                } else {
                    Color::Red
                };

                ListItem::new(format!(
                    " {} [{:?}] - Memory: {}MB / {}MB",
                    a.name, a.status, mem_mb, limit_mb
                ))
                .style(Style::default().fg(color))
            })
            .collect();

        let list = List::new(items).block(agents_block);
        f.render_widget(list, body_chunks[0]);
    } else {
        f.render_widget(
            Paragraph::new("Connecting to daemon...").block(agents_block),
            body_chunks[0],
        );
    }

    // Proxy Stats
    let stats_block = Block::default()
        .borders(Borders::ALL)
        .title(" System Stats ");
    if let Some(status) = &app.status {
        let hit_rate = if status.proxy.total_requests > 0 {
            (status.proxy.cache_hits as f64 / status.proxy.total_requests as f64) * 100.0
        } else {
            0.0
        };

        let stats_text = format!(
            " LLM Proxy:\n  Requests: {}\n  Cache Hits: {}\n  Hit Rate: {:.1}%\n\n Network:\n  RPC Port: {}\n  A2A Port: {}\n  Proxy Port: {}",
            status.proxy.total_requests,
            status.proxy.cache_hits,
            hit_rate,
            status.node_info.rpc_port,
            status.node_info.a2a_port,
            status.node_info.proxy_port
        );
        f.render_widget(
            Paragraph::new(stats_text).block(stats_block),
            body_chunks[1],
        );
    } else {
        f.render_widget(Paragraph::new("...").block(stats_block), body_chunks[1]);
    }

    // 3. Footer
    let footer = Block::default().borders(Borders::ALL);
    let footer_line = Line::from(vec![
        Span::styled(
            " \u{2692} Merab Agent Runtime \u{2014} ",
            Style::default().fg(Color::Rgb(204, 0, 0)),
        ),
        Span::styled(
            "Revolutionizing AI Orchestration ",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(Paragraph::new(footer_line).block(footer), chunks[2]);
}
