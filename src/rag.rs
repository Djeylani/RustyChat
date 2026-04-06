use std::path::Path;
use tokio::task;
use walkdir::WalkDir;
use crate::ollama::{OllamaEmbeddingRequest, OllamaEmbeddingResponse};
use crate::db::{clear_document_chunks_for_prefix, init_db};
use rusqlite::params;
use reqwest::Client;

pub struct IndexStats {
    pub files_indexed: usize,
    pub chunks_indexed: usize,
    pub chunks_replaced: usize,
}

pub async fn index_directory(dir_path: &str, model: &str) -> Result<IndexStats, Box<dyn std::error::Error + Send + Sync>> {
    let dir_path = dir_path.to_string();
    let model = model.to_string();
    let work = task::spawn_blocking(move || collect_index_jobs(&dir_path))
        .await
        .map_err(|e| format!("Indexing task failed to start: {e}"))??;

    let client = Client::new();
    let conn = init_db();
    let chunks_replaced = clear_document_chunks_for_prefix(&conn, &work.normalized_prefix) as usize;
    let mut chunks_indexed = 0usize;

    for (file_path, chunk) in &work.jobs {
        let req = OllamaEmbeddingRequest {
            model: model.clone(),
            prompt: chunk.clone(),
        };

        let resp = client.post("http://localhost:11434/api/embeddings")
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(format!("Embedding request failed with status {}", resp.status()).into());
        }

        let emb_resp = resp.json::<OllamaEmbeddingResponse>().await?;
        let embedding_blob = bincode::serialize(&emb_resp.embedding)?;
        conn.execute(
            "INSERT INTO document_chunks (file_path, content, embedding) VALUES (?1, ?2, ?3)",
            params![file_path, chunk, embedding_blob],
        )?;
        chunks_indexed += 1;
    }

    Ok(IndexStats {
        files_indexed: work.files_indexed,
        chunks_indexed,
        chunks_replaced,
    })
}

struct IndexWork {
    normalized_prefix: String,
    jobs: Vec<(String, String)>,
    files_indexed: usize,
}

fn collect_index_jobs(dir_path: &str) -> Result<IndexWork, Box<dyn std::error::Error + Send + Sync>> {
    let normalized_prefix = Path::new(dir_path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(dir_path).to_path_buf())
        .to_string_lossy()
        .to_string();

    let mut jobs = Vec::new();
    let mut files_indexed = 0usize;

    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && is_text_file(path) {
            if let Ok(content) = std::fs::read_to_string(path) {
                files_indexed += 1;
                let file_path = path.to_string_lossy().to_string();
                for chunk in split_content(&content, 1000) {
                    jobs.push((file_path.clone(), chunk));
                }
            }
        }
    }

    Ok(IndexWork {
        normalized_prefix,
        jobs,
        files_indexed,
    })
}

fn is_text_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    matches!(ext, "rs" | "md" | "txt" | "py" | "js" | "ts" | "toml" | "json" | "c" | "cpp" | "h")
}

fn split_content(content: &str, chunk_size: usize) -> Vec<String> {
    content.chars()
        .collect::<Vec<char>>()
        .chunks(chunk_size)
        .map(|c| c.iter().collect::<String>())
        .collect()
}

pub async fn get_context(query: &str, model: &str, limit: usize) -> Result<String, String> {
    let client = Client::new();
    let conn = init_db();
    
    let req = OllamaEmbeddingRequest {
        model: model.to_string(),
        prompt: query.to_string(),
    };

    let resp = client.post("http://localhost:11434/api/embeddings")
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Could not reach Ollama embeddings API: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Embeddings API returned status {}. Check that the embedding model supports /api/embeddings.",
            resp.status()
        ));
    }

    let emb_resp = resp
        .json::<OllamaEmbeddingResponse>()
        .await
        .map_err(|e| format!("Failed to parse embeddings response: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT file_path, content, embedding FROM document_chunks")
        .map_err(|e| format!("Failed to read indexed documents: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let file_path: String = row.get(0)?;
            let content: String = row.get(1)?;
            let embedding_blob: Vec<u8> = row.get(2)?;
            let embedding: Vec<f32> = bincode::deserialize(&embedding_blob).unwrap_or_default();
            Ok((file_path, content, embedding))
        })
        .map_err(|e| format!("Failed to iterate indexed documents: {e}"))?;

    let mut results: Vec<(f32, String, String)> = Vec::new();
    for row in rows {
        if let Ok((file_path, content, embedding)) = row {
            if embedding.is_empty() {
                continue;
            }
            let score = cosine_similarity(&emb_resp.embedding, &embedding);
            results.push((score, file_path, content));
        }
    }

    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results.iter()
        .take(limit)
        .map(|r| format!("Source: {}\n{}", r.1, r.2))
        .collect::<Vec<String>>()
        .join("\n---\n"))
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    let dot_product: f32 = v1.iter().zip(v2).map(|(a, b)| a * b).sum();
    let mag1: f32 = v1.iter().map(|a| a * a).sum::<f32>().sqrt();
    let mag2: f32 = v2.iter().map(|a| a * a).sum::<f32>().sqrt();
    if mag1 == 0.0 || mag2 == 0.0 { 0.0 } else { dot_product / (mag1 * mag2) }
}
