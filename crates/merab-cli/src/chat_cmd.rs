use anyhow::Result;
use merab_ai::ChatMessage;

use crate::client::MerabClient;

pub async fn run_chat(
    client: &MerabClient,
    resume: bool,
    session: Option<String>,
    new: bool,
    list: bool,
) -> Result<()> {
    if list {
        let project_path = std::env::current_dir()?.to_string_lossy().to_string();
        let convs = client.conv_list(&project_path, 20).await?;
        println!("Conversaciones guardadas:");
        println!("{:<12} {:<20} {:<6} {}", "ID", "FECHA", "MSGS", "TITULO");
        for conv in convs {
            let fecha = conv.updated_at.split('T').next().unwrap_or(&conv.updated_at);
            let titulo = conv.title.as_deref().unwrap_or("(sin título)");
            println!("{:<12} {:<20} {:<6} {}", &conv.id[..8], fecha, conv.message_count, titulo);
        }
        return Ok(());
    }

    let project_path = std::env::current_dir()?.to_string_lossy().to_string();
    let conv_id = if new {
        let conv = client.conv_create(&project_path).await?;
        println!("[merab] Chat iniciado. Sesión: {}", &conv.id[..8]);
        conv.id
    } else if let Some(ref prefix) = session {
        let convs = client.conv_list(&project_path, 100).await?;
        let found = convs.iter().find(|c| c.id.starts_with(prefix));
        match found {
            Some(conv) => {
                println!(
                    "[merab] Retomando sesión {} ({} mensajes)",
                    &conv.id[..8],
                    conv.message_count
                );
                conv.id.clone()
            }
            None => {
                println!(
                    "[merab] No se encontró conversación con prefijo '{}'. Iniciando nueva...",
                    prefix
                );
                let conv = client.conv_create(&project_path).await?;
                conv.id
            }
        }
    } else if resume {
        match client.conv_get_last(&project_path).await? {
            Some(conv) => {
                println!(
                    "[merab] Retomando sesión {} ({} mensajes)",
                    &conv.id[..8],
                    conv.message_count
                );
                conv.id
            }
            None => {
                println!("[merab] No hay conversaciones previas. Iniciando nueva...");
                let conv = client.conv_create(&project_path).await?;
                conv.id
            }
        }
    } else {
        match client.conv_get_last(&project_path).await? {
            Some(conv) => {
                println!(
                    "[merab] Retomando sesión {} ({} mensajes)",
                    &conv.id[..8],
                    conv.message_count
                );
                conv.id
            }
            None => {
                let conv = client.conv_create(&project_path).await?;
                println!("[merab] Chat iniciado. Sesión: {}", &conv.id[..8]);
                conv.id
            }
        }
    };

    let history = client.conv_get_messages(&conv_id).await?;
    let context: Vec<ChatMessage> = history
        .iter()
        .map(|m| {
            if m.role == "user" {
                ChatMessage::user(&m.content)
            } else {
                ChatMessage::assistant(&m.content)
            }
        })
        .collect();

    crate::chat_ui::start_chat_session_with_history(client, &conv_id, context).await?;
    Ok(())
}
