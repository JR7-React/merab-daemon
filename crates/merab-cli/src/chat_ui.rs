use std::io::{self, Write};
use termimad::{MadSkin, crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
    terminal::{Clear, ClearType},
}};
use merab_ai::ChatMessage;
use crate::client::ForgeClient;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

const MAX_CHAT_STEPS: u32 = 10;

pub async fn start_chat_session(client: &ForgeClient) -> anyhow::Result<()> {
    let status = client.get_system_status().await?;
    let model = status.ai.model.clone();
    
    print_header(&model)?;
    print_help()?;
    
    let mut context: Vec<ChatMessage> = Vec::new();
    let skin = make_skin();
    let mut rl = DefaultEditor::new()?;
    let mut step_count: u32 = 0;
    let mut tool_count: u32 = 0;

    loop {
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Green),
            Print("\n❯ "),
            ResetColor
        )?;

        let readline = rl.readline("");
        match readline {
            Ok(line) => {
                let input = line.trim();
                rl.add_history_entry(input)?;

                if handle_slash_command(input, client, &mut context).await? {
                    continue;
                }

                if input.is_empty() {
                    continue;
                }

                let result = run_agent_loop(client, input, &mut context, &skin, &model).await;
                
                match result {
                    Ok((response, steps, tools)) => {
                        context.push(ChatMessage::user(input));
                        context.push(ChatMessage::assistant(&response));
                        step_count += steps;
                        tool_count += tools;
                    }
                    Err(e) => {
                        print_error(&format!("{}", e))?;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                print_goodbye(&model, step_count, tool_count)?;
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_slash_command(
    input: &str,
    client: &ForgeClient,
    context: &mut Vec<ChatMessage>,
) -> anyhow::Result<bool> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    
    if parts.is_empty() || !parts[0].starts_with('/') {
        return Ok(false);
    }

    match parts[0] {
        "/help" | "/h" | "/?" => {
            print_help()?;
        }
        "/clear" | "/c" => {
            execute!(io::stdout(), Clear(ClearType::All), Print("\x1b[H"))?;
            let status = client.get_system_status().await?;
            print_header(&status.ai.model)?;
        }
        "/reset" | "/r" => {
            context.clear();
            print_status("Context cleared.")?;
        }
        "/model" | "/m" => {
            let status = client.get_system_status().await?;
            print_model_info(&status.ai.model, status.ai.max_tokens, status.ai.temperature)?;
        }
        "/agents" | "/a" => {
            let agents = client.list_agents().await?;
            print_agents(&agents)?;
        }
        "/status" | "/s" => {
            let status = client.get_system_status().await?;
            print_status_full(&status)?;
        }
        "/quit" | "/q" | "/exit" => {
            print_status("Goodbye!")?;
            std::process::exit(0);
        }
        _ => {
            print_error(&format!("Unknown command: {}. Type /help for available commands.", parts[0]))?;
        }
    }

    Ok(true)
}

async fn run_agent_loop(
    client: &ForgeClient,
    user_input: &str,
    context: &mut Vec<ChatMessage>,
    skin: &MadSkin,
    model: &str,
) -> anyhow::Result<(String, u32, u32)> {
    let mut messages = context.clone();
    messages.push(ChatMessage::user(user_input));

    print_thinking(model);

    let mut total_steps: u32 = 0;
    let mut total_tools: u32 = 0;

    for step in 0..MAX_CHAT_STEPS {
        let (current_msg, prev_context) = if step == 0 {
            (user_input.to_string(), context.clone())
        } else {
            let last = messages.last().cloned().unwrap_or(ChatMessage::user(""));
            let prev = messages[..messages.len() - 1].to_vec();
            (last.content.clone(), prev)
        };

        let resp = client.ai_chat(&current_msg, prev_context).await;

        execute!(io::stdout(), Clear(ClearType::CurrentLine), Print("\r"))?;

        match resp {
            Ok(ai_resp) => {
                if let Some(tool_call) = &ai_resp.tool_call {
                    total_steps += 1;
                    total_tools += 1;
                    
                    print_tool_call(&tool_call.name, &tool_call.arguments)?;

                    let tool_result = client
                        .ai_execute_tool(&tool_call.name, tool_call.arguments.clone())
                        .await;

                    match tool_result {
                        Ok(result) => {
                            print_tool_result(&tool_call.name, &result)?;

                            let result_str = serde_json::to_string(&result)?;
                            messages.push(ChatMessage::assistant(&ai_resp.content));
                            messages.push(ChatMessage::user(format!(
                                "Tool '{}' returned:\n{}",
                                tool_call.name, result_str
                            )));

                            print_thinking(model);
                            continue;
                        }
                        Err(e) => {
                            print_tool_error(&tool_call.name, &e)?;

                            messages.push(ChatMessage::assistant(&ai_resp.content));
                            messages.push(ChatMessage::user(format!(
                                "Tool '{}' failed with error: {}",
                                tool_call.name, e
                            )));

                            print_thinking(model);
                            continue;
                        }
                    }
                } else {
                    print_ai_response(&ai_resp.content, skin)?;
                    return Ok((ai_resp.content, total_steps, total_tools));
                }
            }
            Err(e) => {
                print_error(&format!("{}", e))?;
                return Ok((String::new(), total_steps, total_tools));
            }
        }
    }

    print_status("Max steps reached.")?;
    Ok(("(max steps reached)".to_string(), total_steps, total_tools))
}

// ── UI Components ───────────────────────────────────────────────

fn print_header(model: &str) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        Clear(ClearType::All),
        Print("\x1b[H"),
        SetForegroundColor(Color::Cyan),
        Print("\n"),
        Print("  ███╗   ███╗███████╗██████╗  █████╗ ██████╗ \n"),
        Print("  ████╗ ████║██╔════╝██╔══██╗██╔══██╗██╔══██╗\n"),
        Print("  ██╔████╔██║█████╗  ██████╔╝███████║██████╔╝\n"),
        Print("  ██║╚██╔╝██║██╔══╝  ██╔══██╗██╔══██║██╔══██╗\n"),
        Print("  ██║ ╚═╝ ██║███████╗██║  ██║██║  ██║██████╔╝\n"),
        Print("  ╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝ \n"),
        Print("\n"),
        ResetColor,
        SetForegroundColor(Color::DarkGrey),
        Print("  AI Agent Runtime Engine"),
        ResetColor,
    )?;
    
    println!();
    println!();
    
    let model_short = model.split('/').last().unwrap_or(model);
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print(&format!("  Model: {}", model_short)),
        ResetColor,
    )?;
    
    println!();
    println!();
    
    Ok(())
}

fn print_help() -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::DarkGrey),
        Print("  Commands: "),
        ResetColor,
    )?;
    
    println!();
    println!("    /help, /h     Show this help");
    println!("    /clear, /c    Clear screen");
    println!("    /reset, /r    Reset conversation context");
    println!("    /model, /m    Show current model info");
    println!("    /agents, /a   List running agents");
    println!("    /status, /s   Show system status");
    println!("    /quit, /q     Exit Forge");
    println!();
    
    Ok(())
}

fn print_thinking(model: &str) {
    let model_short = model.split('/').last().unwrap_or(model);
    let _ = execute!(
        io::stdout(),
        SetForegroundColor(Color::DarkGrey),
        Print(&format!("  ◷ Thinking ({})...", model_short)),
        ResetColor
    );
    let _ = io::stdout().flush();
}

fn print_tool_call(name: &str, args: &serde_json::Value) -> anyhow::Result<()> {
    let args_preview = match args {
        serde_json::Value::Object(map) => {
            let pairs: Vec<String> = map
                .iter()
                .take(3)
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => {
                            if s.len() > 40 {
                                format!("\"{}...\"", &s[..37])
                            } else {
                                format!("\"{}\"", s)
                            }
                        }
                        other => {
                            let s = other.to_string();
                            if s.len() > 40 {
                                format!("{}...", &s[..37])
                            } else {
                                s
                            }
                        }
                    };
                    format!("{}={}", k, val_str)
                })
                .collect();
            let more = if map.len() > 3 { "..." } else { "" };
            format!("{}{}", pairs.join(", "), more)
        }
        _ => String::new(),
    };

    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print(&format!("  ○ {}({})\n", name, args_preview)),
        ResetColor
    )?;
    Ok(())
}

fn print_tool_result(name: &str, result: &serde_json::Value) -> anyhow::Result<()> {
    let text = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let preview = if text.len() > 100 {
        format!("{}... ({} chars)", &text[..97], text.len())
    } else {
        text.to_string()
    };

    if !preview.is_empty() {
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Green),
            Print(&format!("  ✓ {} → {}\n", name, preview.replace('\n', " "))),
            ResetColor
        )?;
    }
    Ok(())
}

fn print_tool_error(name: &str, error: &anyhow::Error) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Red),
        Print(&format!("  ✗ {} failed: {}\n", name, error)),
        ResetColor
    )?;
    Ok(())
}

fn print_ai_response(content: &str, skin: &MadSkin) -> anyhow::Result<()> {
    println!();
    skin.print_text(content);
    println!();
    Ok(())
}

fn print_error(msg: &str) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Red),
        Print(&format!("\n  ✗ {}\n", msg)),
        ResetColor
    )?;
    Ok(())
}

fn print_status(msg: &str) -> anyhow::Result<()> {
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print(&format!("  {}\n", msg)),
        ResetColor
    )?;
    Ok(())
}

fn print_model_info(model: &str, max_tokens: u32, temperature: f32) -> anyhow::Result<()> {
    let model_short = model.split('/').last().unwrap_or(model);
    println!();
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print("  Model Configuration\n"),
        ResetColor,
    )?;
    println!("  ─────────────────────");
    println!("  Model:       {}", model_short);
    println!("  Max tokens:  {}", max_tokens);
    println!("  Temperature: {}", temperature);
    println!("  Full ID:     {}", model);
    println!();
    Ok(())
}

fn print_agents(agents: &[merab_core::AgentSummary]) -> anyhow::Result<()> {
    println!();
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print("  Agents\n"),
        ResetColor,
    )?;
    println!("  ────────────────────────────────────────────");
    
    if agents.is_empty() {
        println!("  No agents registered.");
    } else {
        for agent in agents {
            let status_icon = match agent.status {
                merab_core::AgentStatus::Running => "●",
                merab_core::AgentStatus::Stopped => "○",
                merab_core::AgentStatus::Failed => "✗",
                _ => "?",
            };
            let status_color = match agent.status {
                merab_core::AgentStatus::Running => Color::Green,
                merab_core::AgentStatus::Stopped => Color::Yellow,
                merab_core::AgentStatus::Failed => Color::Red,
                _ => Color::Reset,
            };
            execute!(
                io::stdout(),
                SetForegroundColor(status_color),
                Print(&format!("  {} ", status_icon)),
                ResetColor,
                Print(&format!("{:<15} {:?}\n", agent.name, agent.status)),
            )?;
        }
    }
    println!();
    Ok(())
}

fn print_status_full(status: &merab_core::SystemStatus) -> anyhow::Result<()> {
    println!();
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Yellow),
        Print("  System Status\n"),
        ResetColor,
    )?;
    println!("  ────────────────────────────────────────────");
    println!("  Version:    {}", status.node_info.version);
    println!("  Uptime:     {}s", status.node_info.uptime_seconds);
    println!("  RPC port:   {}", status.node_info.rpc_port);
    println!("  Proxy port: {}", status.node_info.proxy_port);
    println!();
    println!("  Model:      {}", status.ai.model.split('/').last().unwrap_or(&status.ai.model));
    println!("  Max tokens: {}", status.ai.max_tokens);
    println!();
    println!("  Agents:     {} running", status.agents.iter().filter(|a| a.status == merab_core::AgentStatus::Running).count());
    println!("  Requests:   {} ({} cached)", status.proxy.total_requests, status.proxy.cache_hits);
    println!();
    Ok(())
}

fn print_goodbye(_model: &str, steps: u32, tools: u32) -> anyhow::Result<()> {
    println!();
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Cyan),
        Print("  Thanks for using Forge!\n"),
        ResetColor,
    )?;
    println!("  Session: {} steps, {} tool calls", steps, tools);
    println!();
    Ok(())
}

fn make_skin() -> MadSkin {
    let mut skin = MadSkin::default();
    skin.set_headers_fg(Color::Cyan);
    skin.bold.set_fg(Color::Yellow);
    skin.italic.set_fg(Color::DarkGrey);
    skin.code_block.set_fg(Color::Green);
    skin
}
