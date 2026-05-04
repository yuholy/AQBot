use serde_json::{json, Value};

use crate::AppState;

pub async fn collect_context(
    state: &AppState,
    query: &str,
) -> Value {
    let kb_ids = match aqbot_core::repo::knowledge::list_knowledge_bases(&state.sea_db).await {
        Ok(bases) => bases
            .into_iter()
            .filter(|base| base.enabled)
            .map(|base| base.id)
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    let mem_ids = match aqbot_core::repo::memory::list_namespaces(&state.sea_db).await {
        Ok(namespaces) => namespaces.into_iter().map(|namespace| namespace.id).collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };

    if kb_ids.is_empty() && mem_ids.is_empty() {
        return json!({});
    }

    let rag = crate::indexing::collect_rag_context(
        &state.sea_db,
        &state.master_key,
        &state.vector_store,
        &kb_ids,
        &mem_ids,
        query,
        4,
    )
    .await;

    if rag.context_parts.is_empty() && rag.source_results.is_empty() {
        return json!({});
    }

    json!({
        "ragContext": rag.context_parts,
        "ragSources": rag.source_results,
    })
}
