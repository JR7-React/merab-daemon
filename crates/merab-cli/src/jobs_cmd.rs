use anyhow::Result;
use clap::Subcommand;

use crate::client::MerabClient;

#[derive(Subcommand)]
pub enum JobsCommands {
    /// List all background jobs
    List {
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    /// Show log output of a job
    Log {
        /// Job ID
        id: String,
    },
    /// Wait until a job completes
    Wait {
        /// Job ID
        id: String,
    },
    /// Cancel a running job
    Cancel {
        /// Job ID
        id: String,
    },
}

pub async fn run(client: &MerabClient, cmd: JobsCommands) -> Result<()> {
    match cmd {
        JobsCommands::List { limit } => {
            match client.job_list(Some(limit)).await {
                Ok(jobs) if jobs.is_empty() => println!("No hay jobs activos."),
                Ok(jobs) => {
                    println!("{:<38} {:<12} {:<20} {}", "JOB ID", "ESTADO", "CREADO", "TAREA");
                    println!("{}", "─".repeat(100));
                    for j in jobs {
                        let task_preview = if j.task.len() > 30 {
                            format!("{}...", &j.task[..30])
                        } else {
                            j.task.clone()
                        };
                        println!("{:<38} {:<12} {:<20} {}", j.id, j.status, j.created_at, task_preview);
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        JobsCommands::Log { id } => {
            match client.job_log(&id).await {
                Ok(log) => print!("{}", log),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        JobsCommands::Wait { id } => {
            println!("Esperando job {}...", id);
            loop {
                match client.job_status(&id).await {
                    Ok(Some(job)) => {
                        if job.status == "completed" || job.status == "failed" || job.status == "cancelled" {
                            println!("Job {}: {}", id, job.status);
                            if let Some(secs) = job.duration_secs {
                                println!("Duración: {}s", secs);
                            }
                            break;
                        }
                        print!(".");
                        use std::io::Write;
                        std::io::stdout().flush()?;
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                    Ok(None) => {
                        eprintln!("Job {} no encontrado.", id);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        break;
                    }
                }
            }
        }
        JobsCommands::Cancel { id } => {
            match client.job_cancel(&id).await {
                Ok(true) => println!("Job {} cancelado.", id),
                Ok(false) => println!("No se pudo cancelar job {} (¿ya terminó?).", id),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }
    Ok(())
}
