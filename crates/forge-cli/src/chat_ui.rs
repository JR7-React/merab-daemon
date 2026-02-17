use std::io::{self, Write};
use termimad::{MadSkin, crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
    terminal::{Clear, ClearType},

}};
use forge_ai::ChatMessage; // removed ChatRole
use crate::client::ForgeClient;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;


// We need to import the client type. In main.rs it's "mod client;", so here we might need "use crate::client::ForgeClient;"
// But check visibility. client module in main.rs might not be public.
// Users usually put client in `lib.rs` or `mod.rs`. Here it is in `client.rs` but `main.rs` declares `mod client`.
// We should probably move `Chat` command logic to a new file but we need access to `ForgeClient`.

pub async fn start_chat_session(client: &ForgeClient) -> anyhow::Result<()> {
    // 1. Splash Screen
    print_splash()?;
    
    // 2. Chat Loop
    println!("\nType 'quit', 'exit', or '/q' to stop.\n");
    
    let mut context: Vec<ChatMessage> = Vec::new();
    let skin = make_skin();
    let mut rl = DefaultEditor::new()?;

    loop {
        // 3. Prompt
        // Render custom prompt before readline? 
        // Rustyline handles the prompt string.
        // But to have multi-line colored prompt like "You\n>", we pass it to readline.
        // However, rustyline might not handle ANSI codes correctly in prompt length calculation depending on version.
        // Let's try simple prompt first or styled one.
        
        // let prompt = "\x1b[36mYou\n╰─>\x1b[0m "; // Cyan "You\n╰─>"

        // Actually, let's print "You" separately to avoid prompt issues, and just prompt with "╰─> ".
        
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Cyan),
            Print("\n╭── "),
            Print("You"),
            Print("\n"),
            ResetColor
        )?;
        
        let readline = rl.readline("╰─> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                rl.add_history_entry(input)?;

                if input.eq_ignore_ascii_case("quit") || input.eq_ignore_ascii_case("exit") || input == "/q" {
                    break;
                }

                if input.is_empty() {
                    continue;
                }
                
                if input == "/reset" {
                    context.clear();
                    execute!(io::stdout(), SetForegroundColor(Color::Yellow), Print("Context cleared.\n"), ResetColor)?;
                    continue;
                }
                
                // 4. Processing Indicator
                print_thinking();

                // 5. AI Request
                match client.ai_chat(input, context.clone()).await {
                    Ok(resp) => {
                         // Clear "Thinking..." line (rustyline might have messed with cursor, but printing \r helps)
                        execute!(io::stdout(), Clear(ClearType::CurrentLine), Print("\r"))?;

                        // Header
                        execute!(
                            io::stdout(),
                            SetForegroundColor(Color::Magenta),
                            Print("╭── "),
                            Print("Forge AI"),
                            Print("\n│"),
                            Print("\n"),
                            ResetColor
                        )?;

                        // Render Markdown content
                        skin.print_text(&resp.content);

                        // Footer
                        execute!(
                            io::stdout(),
                            SetForegroundColor(Color::Magenta),
                            Print("╰───────────────────────────────\n"),
                            ResetColor
                        )?;

                        // Update Context
                        context.push(ChatMessage::user(input));
                        context.push(ChatMessage::assistant(resp.content));

                        // 6. Tool Calls
                        if let Some(tool) = resp.tool_call {
                            execute!(
                                io::stdout(),
                                SetForegroundColor(Color::Yellow),
                                Print(format!("\n[Tool Call: {}]\n", tool.name)),
                                ResetColor
                            )?;
                        }
                    }
                    Err(e) => {
                         execute!(
                            io::stdout(),
                            SetForegroundColor(Color::Red),
                            Print(format!("\nError: {}\n", e)),
                            ResetColor
                        )?;
                    }
                }
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }


    Ok(())
}

fn print_splash() -> anyhow::Result<()> {
    // Using forge_tui::splash if available
    // Since forge_tui returns ratatui Lines, we need to map colors manually or just output raw ansi if possible.
    // Ideally we'd use the same logic but for crossterm direct printing. 
    // For now, let's just print a simplified version or try to interpret the ratatui styles.
    // Accessing `forge_tui::splash::build_splash_lines()`
    
    let lines = forge_tui::splash::build_splash_lines();
    
    for line in lines {
        for span in line.spans {
            // Map ratatui style to crossterm
            let fg = span.style.fg.unwrap_or(ratatui::style::Color::Reset);
            let color = match fg {
                ratatui::style::Color::Reset => Color::Reset,
                ratatui::style::Color::Black => Color::Black,
                ratatui::style::Color::Red => Color::Red,
                ratatui::style::Color::Green => Color::Green,
                ratatui::style::Color::Yellow => Color::Yellow,
                ratatui::style::Color::Blue => Color::Blue,
                ratatui::style::Color::Magenta => Color::Magenta,
                ratatui::style::Color::Cyan => Color::Cyan,
                ratatui::style::Color::Gray => Color::Grey,
                ratatui::style::Color::DarkGray => Color::DarkGrey,
                ratatui::style::Color::LightRed => Color::Red, // approximation
                ratatui::style::Color::LightGreen => Color::Green,
                ratatui::style::Color::LightYellow => Color::Yellow,
                ratatui::style::Color::LightBlue => Color::Blue,
                ratatui::style::Color::LightMagenta => Color::Magenta,
                ratatui::style::Color::LightCyan => Color::Cyan,
                ratatui::style::Color::White => Color::White,
                ratatui::style::Color::Rgb(r, g, b) => Color::Rgb { r, g, b },
                ratatui::style::Color::Indexed(i) => Color::AnsiValue(i),
            };
            
            execute!(
                io::stdout(),
                SetForegroundColor(color),
                Print(span.content),
            )?;
        }
        println!();
    }
    execute!(io::stdout(), ResetColor)?;
    
    Ok(())
}

fn make_skin() -> MadSkin {
    let mut skin = MadSkin::default();
    skin.set_headers_fg(Color::Magenta);
    skin.bold.set_fg(Color::Yellow);
    skin.italic.set_fg(Color::Cyan);
    // skin.code_block.bg = Color::AnsiValue(236); 
    // skin.inline_code.bg = Color::AnsiValue(236);
    // skin.inline_code.set_fg(Color::Reset); 
    skin
}


fn print_thinking() {
    let _ = execute!(
        io::stdout(),
        SetForegroundColor(Color::DarkGrey),
        Print("Thinking..."),
        ResetColor
    );
    let _ = io::stdout().flush();
}
