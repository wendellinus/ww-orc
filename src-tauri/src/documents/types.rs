use serde::Serialize;
use std::path::PathBuf;

pub struct StoredImage {
    pub id: String,
    pub workspace_id: String,
    pub original_name: String,
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub mime_type: String,
    pub byte_size: i64,
    pub width: i64,
    pub height: i64,
}

pub struct DocumentRecord {
    pub image_id: String,
    pub workspace_id: String,
    pub original_name: String,
    pub relative_path: String,
    pub status: String,
    pub text: String,
    pub error_message: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrDocument {
    pub image_id: String,
    pub workspace_id: String,
    pub file_name: String,
    pub image_path: String,
    pub status: String,
    pub text: String,
    pub error_message: Option<String>,
    pub created_at: i64,
}
