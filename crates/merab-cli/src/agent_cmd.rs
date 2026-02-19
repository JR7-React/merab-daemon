use anyhow::Result;
use merab_core::AgentManifest;
use std::path::PathBuf;

use crate::client::MerabClient;

pub async fn register(client: &MerabClient, manifest: PathBuf) -> Result<()> {
    let content = std::fs::read_to_string(&manifest)?;
    let agent_manifest: AgentManifest = toml::from_str(&content)?;
    let record = client.register_agent(agent_manifest).await?;
    println!("Registered agent: {} (id: {})", record.manifest.name, record.id);
    Ok(())
}

pub async fn list(client: &MerabClient) -> Result<()> {
    let agents = client.list_agents().await?;
    if agents.is_empty() {
        println!("No agents registered.");
    } else {
        println!("{:<38} {:<20} {:<12} {:<8} {:>6}", "ID", "NAME", "PROTOCOL", "STATUS", "PID");
        println!("{}", "-".repeat(84));
        for a in agents {
            println!(
                "{:<38} {:<20} {:<12} {:<8} {:>6}",
                a.id,
                a.name,
                format!("{:?}", a.protocol).to_lowercase(),
                format!("{:?}", a.status).to_lowercase(),
                a.pid.map(|p| p.to_string()).unwrap_or_default(),
            );
        }
    }
    Ok(())
}

pub async fn status(client: &MerabClient, id: &str) -> Result<()> {
    let record = client.get_agent(id).await?;
    println!("{}", serde_json::to_string_pretty(&record)?);
    Ok(())
}

pub async fn start(client: &MerabClient, id: &str) -> Result<()> {
    let record = client.start_agent(id).await?;
    println!("Started agent {} (pid: {:?})", record.manifest.name, record.pid);
    Ok(())
}

pub async fn stop(client: &MerabClient, id: &str) -> Result<()> {
    let record = client.stop_agent(id).await?;
    println!("Stopped agent {}", record.manifest.name);
    Ok(())
}

pub async fn unregister(client: &MerabClient, id: &str) -> Result<()> {
    client.unregister_agent(id).await?;
    println!("Unregistered agent {id}");
    Ok(())
}

pub async fn send(client: &MerabClient, from: &str, to: &str, content: &str) -> Result<()> {
    let msg = client.send_message(from, to, content).await?;
    println!("Message sent (id: {})", msg.id);
    Ok(())
}

pub async fn broadcast(client: &MerabClient, from: &str, content: &str) -> Result<()> {
    let msg = client.broadcast_message(from, content).await?;
    println!("Broadcast sent (id: {})", msg.id);
    Ok(())
}

pub async fn messages(client: &MerabClient, agent_id: &str) -> Result<()> {
    let msgs = client.get_messages(agent_id).await?;
    if msgs.is_empty() {
        println!("No pending messages.");
    } else {
        println!("{:<38} {:<38} {:<10} {}", "MESSAGE ID", "FROM", "TYPE", "CONTENT");
        println!("{}", "-".repeat(100));
        for m in msgs {
            let msg_type = if m.to_agent.is_some() { "direct" } else { "broadcast" };
            println!("{:<38} {:<38} {:<10} {}", m.id, m.from_agent, msg_type, m.content);
        }
    }
    Ok(())
}

pub async fn ack(client: &MerabClient, message_id: &str) -> Result<()> {
    client.ack_message(message_id).await?;
    println!("Message {message_id} acknowledged.");
    Ok(())
}

pub async fn tools(client: &MerabClient, agent_id: &str) -> Result<()> {
    let tools = client.list_tools(agent_id).await?;
    if tools.is_empty() {
        println!("No tools available.");
    } else {
        println!("{:<30} {}", "TOOL", "DESCRIPTION");
        println!("{}", "-".repeat(70));
        for t in tools {
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
            println!("{:<30} {}", name, desc);
        }
    }
    Ok(())
}

pub async fn call(client: &MerabClient, agent_id: &str, tool_name: &str, arguments: &str) -> Result<()> {
    let args: serde_json::Value = serde_json::from_str(arguments)?;
    let result = client.call_tool(agent_id, tool_name, args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub async fn a2a_discover(client: &MerabClient, url: &str) -> Result<()> {
    let card = client.a2a_discover(url).await?;
    println!("{}", serde_json::to_string_pretty(&card)?);
    Ok(())
}

pub async fn a2a_send(client: &MerabClient, url: &str, skill: &str, input: &str) -> Result<()> {
    let args: serde_json::Value = serde_json::from_str(input)?;
    println!("Sending task to {}...", url);
    let response = client.a2a_send(url, skill, args).await?;
    println!("Task submitted. ID: {} (Status: {})", response.task_id, response.status);

    let mut status = response.status;
    while status != "completed" && status != "failed" && status != "cancelled" {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        print!(".");
        use std::io::Write;
        std::io::stdout().flush()?;

        let details = client.a2a_get_task(url, &response.task_id).await?;
        status = details.status;

        if status == "completed" {
            println!("\nTask Completed!");
            println!("Output: {}", serde_json::to_string_pretty(&details.output)?);
        } else if status == "failed" {
            println!("\nTask Failed!");
            println!("Error: {:?}", details.error);
        }
    }
    Ok(())
}
