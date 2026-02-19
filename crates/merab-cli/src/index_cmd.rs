use anyhow::Result;
use crate::client::MerabClient;

pub async fn run_build(client: &MerabClient, project_path: &str) -> Result<()> {
    println!("Indexando proyecto...");
    let count = client.index_build(project_path.to_string()).await?;
    println!("Índice listo: {} símbolos", count);
    Ok(())
}

pub async fn run_search(
    client: &MerabClient,
    project_path: &str,
    query: &str,
    kind: Option<String>,
    limit: usize,
) -> Result<()> {
    let symbols = client
        .index_search(project_path.to_string(), query.to_string(), kind, limit)
        .await?;
    if symbols.is_empty() {
        println!("Sin resultados para '{}'", query);
        return Ok(());
    }
    for s in symbols.iter().take(limit) {
        println!(
            "{:<8} {}:{}\n         {}",
            s.kind, s.file, s.line, s.signature
        );
    }
    Ok(())
}
