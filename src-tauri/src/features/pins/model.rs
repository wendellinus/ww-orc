use serde::Serialize;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub id: String,
    pub workspace_id: String,
    pub image_id: String,
    pub zoom: f64,
    pub is_open: bool,
    pub created_at: i64,
}
