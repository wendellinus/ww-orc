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
