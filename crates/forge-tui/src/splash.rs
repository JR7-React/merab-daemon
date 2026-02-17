use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

// Colores del splash
const SPARK: Color = Color::Rgb(255, 80, 80); // Rojo brillante
const FACE: Color = Color::White;
const EYES: Color = Color::Red;
const CORE: Color = Color::Red; // Core del pecho
const METAL_GRADIENT_START: Color = Color::Rgb(255, 60, 0); // Naranja rojizo
const METAL_GRADIENT_END: Color = Color::Rgb(200, 30, 0); // Rojo oscuro
const BASE_COLOR: Color = Color::Rgb(139, 0, 0); // Rojo oscuro
const FORGE_LETTERS: Color = Color::Rgb(204, 0, 0); // Rojo más oscuro
const SUBTITLE: Color = Color::DarkGray;

pub fn build_splash_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("        .·✦✧.·         ", Style::default().fg(SPARK)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("     ·✦✧·          ", Style::default().fg(SPARK)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("   ✧·                ", Style::default().fg(SPARK)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("  ┌───────────┐      ", Style::default().fg(FACE)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("  │  ", Style::default().fg(FACE)),
            Span::styled("●", Style::default().fg(EYES)),
            Span::styled("   ", Style::default().fg(FACE)),
            Span::styled("●", Style::default().fg(EYES)),
            Span::styled("  │      ", Style::default().fg(FACE)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("  │   ", Style::default().fg(FACE)),
            Span::styled("░▒▓██", Style::default().fg(CORE)),
            Span::styled("   │      ", Style::default().fg(FACE)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("  └─", Style::default().fg(FACE)),
            Span::styled("───────", Style::default().fg(METAL_GRADIENT_START)),
            Span::styled("──┘      ", Style::default().fg(FACE)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("  ╡", Style::default().fg(FACE)),
            Span::styled("░▒▓██████▓▒░", Style::default().fg(METAL_GRADIENT_END)),
            Span::styled("╡      ", Style::default().fg(FACE)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("  ▄▀▀", Style::default().fg(BASE_COLOR)),
            Span::styled("        ", Style::default().fg(FORGE_LETTERS)),
            Span::styled("▀▀▄      ", Style::default().fg(BASE_COLOR)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled(" █", Style::default().fg(BASE_COLOR)),
            Span::styled("███████████", Style::default().fg(FORGE_LETTERS)),
            Span::styled("███", Style::default().fg(BASE_COLOR)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled(" ", Style::default().fg(BASE_COLOR)),
            Span::styled("█", Style::default().fg(FORGE_LETTERS)),
            Span::raw("   "),
            Span::styled("FORGE", Style::default().fg(FORGE_LETTERS)),
            Span::raw("   "),
            Span::styled("█", Style::default().fg(FORGE_LETTERS)),
            Span::raw("                     "),
        ]),
        Line::from(vec![
            Span::styled(" █", Style::default().fg(BASE_COLOR)),
            Span::styled("███████████", Style::default().fg(FORGE_LETTERS)),
            Span::styled("███", Style::default().fg(BASE_COLOR)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::styled("  ▀▄▄", Style::default().fg(BASE_COLOR)),
            Span::raw("        "),
            Span::styled("▄▄▀      ", Style::default().fg(BASE_COLOR)),
            Span::raw("                   "),
        ]),
        Line::from(vec![
            Span::raw("                   "),
            Span::styled("Revolutionizing AI Orchestration", Style::default().fg(SUBTITLE)),
        ]),
    ]
}
