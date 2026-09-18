use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub image_count: i64,
}
