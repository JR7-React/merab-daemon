use crate::client::MerabClient;

pub async fn run_list(client: &MerabClient) -> anyhow::Result<()> {
    let current = MerabClient::current_project_path();
    let projects = client.project_list(Some(20)).await?;

    if projects.is_empty() {
        println!("No hay proyectos registrados. Usa 'merab ask' para comenzar.");
        return Ok(());
    }

    println!("{:<3} {:<40} {:>8}  {}", "", "Proyecto", "Sessions", "Última actividad");
    println!("{}", "─".repeat(70));

    for p in &projects {
        let marker = if p.path == current { "*" } else { " " };
        let name = if p.path.len() > 38 {
            format!("...{}", &p.path[p.path.len()-35..])
        } else {
            p.path.clone()
        };
        println!("{:<3} {:<40} {:>8}  {}",
            marker, name, p.session_count, p.last_active);
    }
    Ok(())
}

pub fn run_switch(path: &str) -> anyhow::Result<()> {
    let resolved = std::path::Path::new(path).canonicalize()
        .map_err(|_| anyhow::anyhow!("Directorio no encontrado: {}", path))?;
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("No se encontró config dir"))?
        .join("merab");
    std::fs::create_dir_all(&config_dir)?;
    std::fs::write(config_dir.join("active-project"), resolved.to_string_lossy().as_bytes())?;
    println!("Proyecto activo: {}", resolved.display());
    Ok(())
}
