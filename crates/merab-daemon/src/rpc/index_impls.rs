use walkdir::WalkDir;
use merab_store::IndexedSymbol;
use merab_core::MerabError;
use crate::parser::extract_symbols;
use crate::rpc::server::{MerabRpc, to_rpc_error};
use jsonrpsee::types::ErrorObjectOwned;

pub async fn index_build(rpc: &MerabRpc, project_path: String) -> Result<u64, ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    db.index_clear_project(&project_path)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;

    let mut count = 0u64;
    for entry in WalkDir::new(&project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .filter(|e| !e.path().to_string_lossy().contains("/target/"))
        .filter(|e| !e.path().to_string_lossy().contains("\\target\\"))
    {
        let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let symbols = extract_symbols(entry.path(), &content);
        for sym in &symbols {
            let indexed = IndexedSymbol {
                name: sym.name.clone(),
                kind: sym.kind.to_string(),
                file: entry.path().to_string_lossy().to_string(),
                line: sym.line,
                signature: sym.signature.clone(),
            };
            db.index_insert_symbol(&project_path, &indexed)
                .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
        }
        count += symbols.len() as u64;
    }
    Ok(count)
}

pub async fn index_search(
    rpc: &MerabRpc,
    project_path: String,
    query: String,
    kind: Option<String>,
    limit: usize,
) -> Result<Vec<IndexedSymbol>, ErrorObjectOwned> {
    let db = rpc.db.lock().await;
    let kind_ref = kind.as_deref();
    let results = db.index_search(&project_path, &query, kind_ref, limit)
        .map_err(|e| to_rpc_error(MerabError::Store(e.to_string())))?;
    Ok(results)
}
