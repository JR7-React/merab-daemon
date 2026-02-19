use anyhow::Result;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::client::MerabClient;
use crate::event_tail;

pub async fn run_ask(client: &MerabClient, question: &str, test: bool, bg: bool) -> Result<()> {
    if bg {
        match client.job_submit(question).await {
            Ok(job_id) => println!(
                "Job submitted: {}\nUse `merab jobs log {}` to follow progress.",
                job_id, job_id
            ),
            Err(e) => eprintln!("Error: {}", e),
        }
        return Ok(());
    }

    let event_file = std::env::temp_dir()
        .join(format!("merab-events-{}.jsonl", std::process::id()));
    let event_file_str = event_file.to_string_lossy().to_string();

    let running = Arc::new(AtomicBool::new(true));
    let tail_handle = event_tail::start_event_tail(&event_file, running.clone());

    let result = if test {
        client.ai_orchestrate_with_tests(question, &event_file_str).await
    } else {
        client.ai_orchestrate_stream(question, &event_file_str).await
    };

    running.store(false, Ordering::Relaxed);
    let _ = tail_handle.join();
    let _ = std::fs::remove_file(&event_file);

    match result {
        Ok(resp) => {
            if let Some(usage) = &resp.usage {
                println!(
                    "\nTokens: {} input / {} output",
                    crate::format_number(usage.input),
                    crate::format_number(usage.output)
                );
                if let Some(cost) = merab_core::estimate_cost(&usage.model, usage.input, usage.output) {
                    println!("Costo estimado: ${:.4}", cost);
                }
            }
            if let Some(artifacts) = &resp.artifacts {
                crate::print_artifact_summary(artifacts);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
    Ok(())
}

pub async fn run_continue(client: &MerabClient, id: Option<String>) -> Result<()> {
    let project_path = std::env::current_dir()?.to_string_lossy().to_string();

    let session_opt = if let Some(ref session_id) = id {
        match client.session_list(&project_path, 50).await {
            Ok(sessions) => sessions.into_iter().find(|s| s.id.starts_with(session_id.as_str())),
            Err(e) => {
                eprintln!("Error al buscar sesión: {}", e);
                None
            }
        }
    } else {
        match client.session_get_last(&project_path).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: {}", e);
                None
            }
        }
    };

    let session = match session_opt {
        Some(s) => s,
        None => {
            println!("No hay sesión previa para continuar en este proyecto.");
            println!("Usa `merab ask` para iniciar una nueva sesión.");
            return Ok(());
        }
    };

    println!("Retomando sesión {}", &session.id[..8.min(session.id.len())]);
    println!("Tarea original: {}", session.task);
    println!("Fecha: {}", session.display_date());
    println!("Estado anterior: {}", session.status);
    println!();

    let mut continuation = format!(
        "Continuando sesión anterior.\n\nLo que se hizo:\n{}\n\n",
        session.summary
    );

    if !session.artifacts.files_created.is_empty() || !session.artifacts.files_modified.is_empty() {
        continuation.push_str("Archivos tocados:\n");
        for f in &session.artifacts.files_created {
            continuation.push_str(&format!("  + {}\n", f));
        }
        for f in &session.artifacts.files_modified {
            continuation.push_str(&format!("  ~ {}\n", f));
        }
        continuation.push('\n');
    }

    continuation.push_str(&format!(
        "Tarea pendiente: {}\n\nContinúa desde donde quedamos.",
        session.task
    ));

    println!("Enviando contexto al agente...\n");

    let event_file = std::env::temp_dir()
        .join(format!("merab-events-{}.jsonl", std::process::id()));
    let event_file_str = event_file.to_string_lossy().to_string();

    let running = Arc::new(AtomicBool::new(true));
    let tail_handle = event_tail::start_event_tail(&event_file, running.clone());

    let result = client.ai_orchestrate_stream(&continuation, &event_file_str).await;

    running.store(false, Ordering::Relaxed);
    let _ = tail_handle.join();
    let _ = std::fs::remove_file(&event_file);

    match result {
        Ok(resp) => {
            if let Some(ref usage) = resp.usage {
                println!(
                    "\nTokens: {} input / {} output",
                    crate::format_number(usage.input),
                    crate::format_number(usage.output)
                );
                if let Some(cost) = merab_core::estimate_cost(&usage.model, usage.input, usage.output) {
                    println!("Costo estimado: ${:.4}", cost);
                }
            }
            if let Some(artifacts) = &resp.artifacts {
                crate::print_artifact_summary(artifacts);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
    Ok(())
}
