use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

// Colores del splash (ajustados según el diseño)
const SPARK_COLOR: Color = Color::Rgb(255, 80, 80); // Rojo brillante para chispas
const FACE_COLOR: Color = Color::White;
const EYES_COLOR: Color = Color::Red;
const CORE_COLOR: Color = Color::Red; // Core del pecho
const METAL_GRADIENT_START: Color = Color::Rgb(255, 60, 0); // Rojo → Naranja
const METAL_GRADIENT_END: Color = Color::Rgb(200, 30, 0); // Más oscuro
const BASE_COLOR: Color = Color::Rgb(139, 0, 0); // Rojo oscuro para base
const FORGE_LETTERS_COLOR: Color = Color::Rgb(204, 0, 0); // Rojo (#CC0000)
const SUBTITLE_COLOR: Color = Color::DarkGray; // Gris (#888888)

pub fn build_splash_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled(
            "              ·✦    ✧    ✦·",
            Style::default().fg(SPARK_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "           ✧·   · ✦ ·   ·✧",
            Style::default().fg(SPARK_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "              ┌────────┐",
            Style::default().fg(FACE_COLOR),
        )]),
        Line::from(vec![
            Span::styled("              │ ", Style::default().fg(FACE_COLOR)),
            Span::styled("●", Style::default().fg(EYES_COLOR)),
            Span::styled("    ", Style::default().fg(FACE_COLOR)),
            Span::styled("●", Style::default().fg(EYES_COLOR)),
            Span::styled(" │", Style::default().fg(FACE_COLOR)),
        ]),
        Line::from(vec![Span::styled(
            "              │  ╰──╯  │",
            Style::default().fg(FACE_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "              └──┬──┬──┘",
            Style::default().fg(FACE_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "            ┌────┴──┴────┐",
            Style::default().fg(FACE_COLOR),
        )]),
        Line::from(vec![
            Span::styled("       ✦·══╡  ", Style::default().fg(SPARK_COLOR)),
            Span::styled("░▒▓██▓▒░", Style::default().fg(CORE_COLOR)),
            Span::styled("  ╞══·✦", Style::default().fg(SPARK_COLOR)),
        ]),
        Line::from(vec![Span::styled(
            "            └────┬──┬────┘",
            Style::default().fg(FACE_COLOR),
        )]),
        Line::from(vec![
            Span::styled("              ", Style::default().fg(FACE_COLOR)),
            Span::styled("▒▓█┘", Style::default().fg(METAL_GRADIENT_START)),
            Span::styled("  └█▓▒", Style::default().fg(METAL_GRADIENT_END)),
        ]),
        Line::from(vec![
            Span::styled("            ", Style::default().fg(FACE_COLOR)),
            Span::styled("▒▓██████████▓▒", Style::default().fg(METAL_GRADIENT_START)),
        ]),
        Line::from(vec![
            Span::styled("          ", Style::default().fg(FACE_COLOR)),
            Span::styled(
                "░▒▓██████████████▓▒░",
                Style::default().fg(METAL_GRADIENT_START),
            ),
        ]),
        Line::from(vec![
            Span::styled("        ", Style::default().fg(FACE_COLOR)),
            Span::styled("▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄", Style::default().fg(BASE_COLOR)),
        ]),
        Line::from(vec![Span::raw("")]), // Línea en blanco para separar el robot de las letras
        Line::from(vec![Span::styled(
            " ███████╗ ██████╗  ██████╗   ██████╗  ███████╗",
            Style::default().fg(FORGE_LETTERS_COLOR),
        )]),
        Line::from(vec![Span::styled(
            " ██╔════╝██╔═══██╗██╔══██╗ ██╔════╝  ██╔════╝",
            Style::default().fg(FORGE_LETTERS_COLOR),
        )]),
        Line::from(vec![Span::styled(
            " █████╗  ██║   ██║██████╔╝ ██║  ███╗ █████╗",
            Style::default().fg(FORGE_LETTERS_COLOR),
        )]),
        Line::from(vec![Span::styled(
            " ██╔══╝  ██║   ██║██╔══██╗ ██║   ██║ ██╔══╝",
            Style::default().fg(FORGE_LETTERS_COLOR),
        )]),
        Line::from(vec![Span::styled(
            " ██║     ╚██████╔╝██║  ██║ ╚██████╔╝ ███████╗",
            Style::default().fg(FORGE_LETTERS_COLOR),
        )]),
        Line::from(vec![Span::styled(
            " ╚═╝      ╚═════╝ ╚═╝  ╚═╝  ╚═════╝  ╚══════╝",
            Style::default().fg(FORGE_LETTERS_COLOR),
        )]),
        Line::from(vec![Span::raw("")]), // Línea en blanco
        Line::from(vec![Span::styled(
            "           ⚒  Agent  Runtime  Engine  ⚒",
            Style::default().fg(SUBTITLE_COLOR),
        )]),
    ]
}
