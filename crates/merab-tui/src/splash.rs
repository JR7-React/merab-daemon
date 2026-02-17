use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

const TITLE_COLOR: Color = Color::Cyan;
const SUBTITLE_COLOR: Color = Color::DarkGray;

pub fn build_splash_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(
            "  ███╗   ███╗███████╗██████╗  █████╗ ██████╗ ",
            Style::default().fg(TITLE_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "  ████╗ ████║██╔════╝██╔══██╗██╔══██╗██╔══██╗",
            Style::default().fg(TITLE_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "  ██╔████╔██║█████╗  ██████╔╝███████║██████╔╝",
            Style::default().fg(TITLE_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "  ██║╚██╔╝██║██╔══╝  ██╔══██╗██╔══██║██╔══██╗",
            Style::default().fg(TITLE_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "  ██║ ╚═╝ ██║███████╗██║  ██║██║  ██║██████╔╝",
            Style::default().fg(TITLE_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "  ╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝ ",
            Style::default().fg(TITLE_COLOR),
        )]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(
            "       AI Agent Runtime Engine",
            Style::default().fg(SUBTITLE_COLOR),
        )]),
    ]
}
