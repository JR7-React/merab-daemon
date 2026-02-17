use std::io;
use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, List, ListItem},
    Terminal,
    style::{Color, Style},
};
use forge_core::SystemStatus;

pub struct TuiApp {
    pub status: Option<SystemStatus>,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            status: None,
            should_quit: false,
        }
    }

    pub fn update_status(&mut self, status: SystemStatus) {
        self.status = Some(status);
    }
}

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
        if let Ok(new_status) = tick_fn().await {
            app.update_status(new_status);
        }

        terminal.draw(|f| ui(f, &app))?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    break;
                }
            }
        }
        
        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    Ok(())
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

    // 1. Header
    let header = Block::default()
        .borders(Borders::ALL)
        .title(" Forge Node Monitor ")
        .style(Style::default().fg(Color::Cyan));
    
    let uptime = app.status.as_ref().map(|s| s.node_info.uptime_seconds).unwrap_or(0);
    let version = app.status.as_ref().map(|s| s.node_info.version.as_str()).unwrap_or("?");
    
    let header_text = format!(" Version: {} | Uptime: {}s | [Q] Quit", version, uptime);
    f.render_widget(Paragraph::new(header_text).block(header), chunks[0]);

    // 2. Main Body (Agents & Stats)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Agents
            Constraint::Percentage(40), // Stats
        ])
        .split(chunks[1]);

    // Agents List
    let agents_block = Block::default().borders(Borders::ALL).title(" Active Agents ");
    if let Some(status) = &app.status {
        let items: Vec<ListItem> = status.agents.iter().map(|a| {
            let mem_mb = a.memory_usage_bytes / (1024 * 1024);
            let limit_mb = a.memory_limit_bytes / (1024 * 1024);
            let color = if a.status == forge_core::AgentStatus::Running { Color::Green } else { Color::Red };
            
            ListItem::new(format!(
                " {} [{:?}] - Memory: {}MB / {}MB",
                a.name, a.status, mem_mb, limit_mb
            )).style(Style::default().fg(color))
        }).collect();
        
        let list = List::new(items).block(agents_block);
        f.render_widget(list, body_chunks[0]);
    } else {
        f.render_widget(Paragraph::new("Connecting to daemon...").block(agents_block), body_chunks[0]);
    }

    // Proxy Stats
    let stats_block = Block::default().borders(Borders::ALL).title(" System Stats ");
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
        f.render_widget(Paragraph::new(stats_text).block(stats_block), body_chunks[1]);
    } else {
        f.render_widget(Paragraph::new("...").block(stats_block), body_chunks[1]);
    }

    // 3. Footer
    let footer = Block::default().borders(Borders::ALL);
    f.render_widget(Paragraph::new(" Forge Agent Runtime — Revolutionizing AI Orchestration ").style(Style::default().fg(Color::DarkGray)).block(footer), chunks[2]);
}
