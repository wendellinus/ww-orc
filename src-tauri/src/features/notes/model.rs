use serde::Serialize;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub workspace_id: String,
    pub text: String,
    pub color: String,
    pub revision: i64,
    pub is_open: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
