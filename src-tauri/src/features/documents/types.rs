use serde::Serialize;

use crate::features::ocr::types::OcrTextBlock;

pub struct DocumentRecord {
    pub image_id: String,
    pub workspace_id: String,
    pub original_name: String,
    pub relative_path: String,
    pub status: String,
    pub text: String,
    pub blocks_json: String,
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
    pub blocks: Vec<OcrTextBlock>,
    pub error_message: Option<String>,
    pub created_at: i64,
}
