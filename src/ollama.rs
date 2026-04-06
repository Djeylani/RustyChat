use serde::{Deserialize, Serialize};
use serde_json::Value;

/* ================= OLLAMA API STRUCTURES ================= */

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(default = "default_stream")]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

fn default_stream() -> bool {
    false
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaChatResponse {
    pub message: OllamaMessage,
    pub done: bool,
    #[serde(default)]
    pub response: String,
}

/* ================= EMBEDDINGS ================= */

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaEmbeddingRequest {
    pub model: String,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaEmbeddingResponse {
    pub embedding: Vec<f32>,
}
