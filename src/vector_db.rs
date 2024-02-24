use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DocumentPayload {
    pub content: String,
}
