use anyhow::Result;
use crate::client::MerabClient;
use crate::{agent_cmd, ask_cmd, bootstrap, chat_cmd, doctor, index_cmd, jobs_cmd, sessions_cmd, watch};
use crate::{Commands, MemoryCommands, IndexCommands};

pub async fn dispatch(command: Commands, client: MerabClient, url: &str) -> Result<()> {
    match command {
        Commands::Init => {
            println!("Merab Init");
            println!("===========");
            match client.ping().await {
                Ok(_) => println!("[OK] Daemon already running"),
                Err(_) => {
                    print!("[..] Starting daemon... ");
                    bootstrap::init_daemon()?;
                    bootstrap::wait_for_daemon(&client).await?;
                    println!("OK");
                }
            }
            let agents = bootstrap::init_agents(&client).await?;
            println!("\nAgents ready:");
            for agent in &agents {
                println!("  - {} ({:?})", agent.name, agent.status);
            }
            println!("\nMerab is ready. Try:");
            println!("  merab chat      - Interactive chat");
            println!("  merab ask \"...\" - Ask a question");
            println!("  merab list      - List agents");
        }

        Commands::Ping => {
            let result = client.ping().await?;
            println!("{result}");
        }

        Commands::Register { manifest } => agent_cmd::register(&client, manifest).await?,
        Commands::List => agent_cmd::list(&client).await?,
        Commands::Status { id } => agent_cmd::status(&client, &id).await?,
        Commands::Start { id } => agent_cmd::start(&client, &id).await?,
        Commands::Stop { id } => agent_cmd::stop(&client, &id).await?,
        Commands::Unregister { id } => agent_cmd::unregister(&client, &id).await?,
        Commands::Send { from, to, content } => agent_cmd::send(&client, &from, &to, &content).await?,
        Commands::Broadcast { from, content } => agent_cmd::broadcast(&client, &from, &content).await?,
        Commands::Messages { agent_id } => agent_cmd::messages(&client, &agent_id).await?,
        Commands::Ack { message_id } => agent_cmd::ack(&client, &message_id).await?,
        Commands::Tools { agent_id } => agent_cmd::tools(&client, &agent_id).await?,
        Commands::Call { agent_id, tool_name, arguments } => {
            agent_cmd::call(&client, &agent_id, &tool_name, &arguments).await?
        }
        Commands::A2aDiscover { url: a2a_url } => agent_cmd::a2a_discover(&client, &a2a_url).await?,
        Commands::A2aSend { url: a2a_url, skill, input } => {
            agent_cmd::a2a_send(&client, &a2a_url, &skill, &input).await?
        }

        Commands::Memory(cmd) => {
            match cmd {
                MemoryCommands::Put { key, value, ttl } => {
                    let json_value: serde_json::Value =
                        serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
                    client.memory_put(&key, json_value, ttl).await?;
                    println!("Stored key: {}", key);
                }
                MemoryCommands::Get { key } => {
                    match client.memory_get(&key).await? {
                        Some(val) => println!("{}", serde_json::to_string_pretty(&val)?),
                        None => println!("Key not found or expired."),
                    }
                }
                MemoryCommands::Delete { key } => {
                    let deleted = client.memory_delete(&key).await?;
                    if deleted {
                        println!("Deleted key: {}", key);
                    } else {
                        println!("Key not found.");
                    }
                }
                MemoryCommands::List { prefix } => {
                    let keys = client.memory_list(prefix.as_deref()).await?;
                    if keys.is_empty() {
                        println!("No keys found.");
                    } else {
                        for key in keys {
                            println!("  - {}", key);
                        }
                    }
                }
            }
        }

        Commands::Monitor => {
            println!("Monitor TUI not yet implemented in this version");
        }

        Commands::Chat { resume, session, new, list } => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            chat_cmd::run_chat(&ready_client, resume, session, new, list).await?;
        }

        Commands::Ask { question, test, bg } => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            ask_cmd::run_ask(&ready_client, &question, test, bg).await?;
        }

        Commands::Jobs(cmd) => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            jobs_cmd::run(&ready_client, cmd).await?;
        }

        Commands::Plan { task } => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            match ready_client.ai_plan(&task).await {
                Ok(plan) => {
                    println!("Generated plan:");
                    println!("{}", serde_json::to_string_pretty(&plan)?);
                }
                Err(e) => eprintln!("Error generating plan: {}", e),
            }
        }

        Commands::ExecutePlan { plan_json } => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            match ready_client.ai_execute_plan(&plan_json).await {
                Ok(result) => {
                    println!("Plan execution result:");
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => eprintln!("Error executing plan: {}", e),
            }
        }

        Commands::Context => {
            let ctx = crate::project_context::ProjectContext::detect(std::path::Path::new("."));
            ctx.display();
        }

        Commands::Sessions { limit } => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            sessions_cmd::run_list(&ready_client, limit).await?;
        }

        Commands::Continue { id } => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            ask_cmd::run_continue(&ready_client, id).await?;
        }

        Commands::Stats => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            sessions_cmd::run_stats(&ready_client).await?;
        }

        Commands::Review { branch, file, critical, output } => {
            crate::review::run_review(url, branch, file, critical, output).await?;
        }

        Commands::Watch { pattern, task, test, debounce, quiet } => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            watch::run_watch(&ready_client, watch::WatchConfig {
                pattern,
                task,
                test,
                debounce,
                quiet,
            }).await?;
        }

        Commands::Doctor { only_errors, json } => {
            let client_opt = MerabClient::new(url).ok();
            let report = doctor::run_doctor(client_opt.as_ref(), only_errors).await;

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                doctor::print_report(&report);
            }

            if report.has_errors() {
                std::process::exit(1);
            }
        }

        Commands::Config(cmd) => {
            match cmd {
                crate::config_cmd::ConfigCommands::List => crate::config_cmd::config_list()?,
                crate::config_cmd::ConfigCommands::Get { key } => crate::config_cmd::config_get(&key)?,
                crate::config_cmd::ConfigCommands::Set { key, value } => crate::config_cmd::config_set(&key, &value)?,
                crate::config_cmd::ConfigCommands::Edit => crate::config_cmd::config_edit()?,
                crate::config_cmd::ConfigCommands::Path => crate::config_cmd::config_path()?,
            }
        }

        Commands::Index(cmd) => {
            let ready_client = bootstrap::ensure_ready(url).await?;
            let project_path = std::env::current_dir()?.to_string_lossy().to_string();
            match cmd {
                IndexCommands::Build => {
                    index_cmd::run_build(&ready_client, &project_path).await?;
                }
                IndexCommands::Search { query, kind, limit } => {
                    index_cmd::run_search(&ready_client, &project_path, &query, kind, limit).await?;
                }
            }
        }

        Commands::SelfUpgrade { task, yes, dry_run } => {
            let merab_root = crate::detect_merab_root()?;
            let cfg = crate::self_upgrade::SelfUpgradeConfig { task, yes, dry_run, merab_root };
            crate::self_upgrade::run_self_upgrade(&client, cfg).await?;
        }
    }

    Ok(())
}
