use std::io::{self, Write};
use termimad::{MadSkin, crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
    terminal::{Clear, ClearType},
}};
use forge_ai::ChatMessage;
use crate::client::ForgeClient;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

const MAX_CHAT_STEPS: u32 = 10;

pub async fn start_chat_session(client: &ForgeClient) -> anyhow::Result<()> {
    print_splash()?;
    println!("\nType 'quit', 'exit', or '/q' to stop. '/reset' to clear context.\n");

    let mut context: Vec<ChatMessage> = Vec::new();
    let skin = make_skin();
    let mut rl = DefaultEditor::new()?;

    loop {
        // Prompt
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Cyan),
            Print("\n╭── You\n"),
            ResetColor
        )?;

        let readline = rl.readline("╰─> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                rl.add_history_entry(input)?;

                if input.eq_ignore_ascii_case("quit")
                    || input.eq_ignore_ascii_case("exit")
                    || input == "/q"
                {
                    break;
                }

                if input.is_empty() {
                    continue;
                }

                if input == "/reset" {
                    context.clear();
                    print_status("Context cleared.")?;
                    continue;
                }

                // Run the agent loop (LLM → tool → LLM → ... → final answer)
                let final_text = run_agent_loop(client, input, &mut context, &skin).await?;

                // Update context with user input and final AI response
                context.push(ChatMessage::user(input));
                context.push(ChatMessage::assistant(&final_text));
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

/// The agent loop: send to LLM, if tool_call → execute → show → repeat.
async fn run_agent_loop(
    client: &ForgeClient,
    user_input: &str,
    context: &mut Vec<ChatMessage>,
    skin: &MadSkin,
) -> anyhow::Result<String> {
    // Build messages for this turn (context + new user message)
    let mut messages = context.clone();
    messages.push(ChatMessage::user(user_input));

    print_thinking();

    for step in 0..MAX_CHAT_STEPS {
        // Send to LLM via ai_chat (single-step, returns tool_call if any)
        // ai_chat takes `message` + `context` separately.
        // context = all messages before current, message = the new one.
        let (current_msg, prev_context) = if step == 0 {
            // First step: user_input is the new message, context is the history
            (user_input.to_string(), context.clone())
        } else {
            // Subsequent steps: the "new message" is the tool result we just appended
            // We need to send the full conversation as context + last as message
            let last = messages.last().cloned().unwrap_or(ChatMessage::user(""));
            let prev = messages[..messages.len() - 1].to_vec();
            (last.content.clone(), prev)
        };

        let resp = client.ai_chat(&current_msg, prev_context).await;

        // Clear "Thinking..." / "Running tool..."
        execute!(io::stdout(), Clear(ClearType::CurrentLine), Print("\r"))?;

        match resp {
            Ok(ai_resp) => {
                if let Some(tool_call) = &ai_resp.tool_call {
                    // Show the tool being called
                    print_tool_call(&tool_call.name, &tool_call.arguments)?;

                    // Execute the tool
                    let tool_result = client
                        .ai_execute_tool(&tool_call.name, tool_call.arguments.clone())
                        .await;

                    match tool_result {
                        Ok(result) => {
                            print_tool_result(&tool_call.name, &result)?;

                            let result_str = serde_json::to_string(&result)?;

                            // Update messages for next iteration
                            messages.push(ChatMessage::assistant(&ai_resp.content));
                            messages.push(ChatMessage::user(format!(
                                "Tool '{}' returned:\n{}",
                                tool_call.name, result_str
                            )));

                            print_thinking_step(step + 1);
                            continue;
                        }
                        Err(e) => {
                            print_tool_error(&tool_call.name, &e)?;

                            // Feed error back to LLM
                            messages.push(ChatMessage::assistant(&ai_resp.content));
                            messages.push(ChatMessage::user(format!(
                                "Tool '{}' failed with error: {}",
                                tool_call.name, e
                            )));

                            print_thinking_step(step + 1);
                            continue;
                        }
                    }
                } else {
                    // No tool call — LLM is done, show the response
                    print_ai_response(&ai_resp.content, skin)?;
                    return Ok(ai_resp.content);
                }
            }
            Err(e) => {
                print_error(&format!("{}", e))?;
                return Ok(String::new());
            }
        }
    }

    // Max steps reached
    print_status("Max steps reached.")?;
    Ok("(max steps reached)".to_string())
}

// ── UI Helpers ───────────────────────────────────────────────

fn print_thinking() {
    let _ = execute!(
        io::stdout(),
        SetForegroundColor(Color::DarkGrey),
        Print("  Thinking..."),
        ResetColor
    );
    let _ = io::stdout().flush();
}

fn print_thinking_step(step: u32) {
    let _ = execute!(
        io::stdout(),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("  Thinking... (step {})", step + 1)),
        ResetColor
    );
    let _ = io::stdout().flush();
}

fn print_tool_call(name: &str, args: &serde_json::Value) -> anyhow::Result<()> {
    let args_preview = match args {
        serde_json::Value::Object(map) => {
            let pairs: Vec<String> = map
                .iter()
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => {
                            if s.len() > 60 {
                                format!("\"{}...\"", &s[..57])
                            } else {
                                format!("\"{}\"", s)
                            }
                        }
                        other => {
                            let s = other.to_string();
                            if s.len() > 60 {
                                format!("{}...", &s[..57])
                            } else {
                                s
                            }
                        }
                    };
                    format!("{}={}", k, val_str)
                })
                .collect();
            pairs.join(", ")
        }
        _ => String::new(),
    };

    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print(format!("  ► {} ({})\n", name, args_preview)),
        ResetColor
    )?;
    let _ = io::stdout().flush();
    Ok(())
}

fn print_tool_result(name: &str, result: &serde_json::Value) -> anyhow::Result<()> {
    // Extract text content from MCP response
    let text = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Show truncated result
    let preview = if text.len() > 200 {
        format!("{}... ({} chars)", &text[..197], text.len())
    } else {
        text.to_string()
    };

    if !preview.is_empty() {
        execute!(
            io::stdout(),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  ✓ {} → {}\n", name, preview.replace('\n', " "))),
            ResetColor
        )?;
    }
    Ok(())
}

fn print_tool_error(name: &str, error: &anyhow::Error) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Red),
        Print(format!("  ✗ {} failed: {}\n", name, error)),
        ResetColor
    )?;
    Ok(())
}

fn print_ai_response(content: &str, skin: &MadSkin) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Magenta),
        Print("╭── Forge AI\n│\n"),
        ResetColor
    )?;

    skin.print_text(content);

    execute!(
        io::stdout(),
        SetForegroundColor(Color::Magenta),
        Print("╰───────────────────────────────\n"),
        ResetColor
    )?;
    Ok(())
}

fn print_error(msg: &str) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Red),
        Print(format!("\n  Error: {}\n", msg)),
        ResetColor
    )?;
    Ok(())
}

fn print_status(msg: &str) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print(format!("  {}\n", msg)),
        ResetColor
    )?;
    Ok(())
}

fn print_splash() -> anyhow::Result<()> {
    let lines = forge_tui::splash::build_splash_lines();

    for line in lines {
        for span in line.spans {
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
                ratatui::style::Color::LightRed => Color::Red,
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
    skin
}
