use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrTextBlock {
    pub text: String,
    pub box_points: Vec<OcrPoint>,
}

pub struct OcrOutput {
    pub text: String,
    pub blocks: Vec<OcrTextBlock>,
}
