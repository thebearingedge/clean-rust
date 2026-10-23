use axum::{
    body::Body,
    http::{StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use serde_json::Value;
use uuid::Uuid;

pub async fn get_response_parts(res: Response<Body>) -> (StatusCode, String, Option<Value>) {
    let status = res.status();
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).ok();
    (status, content_type, json)
}

const UUID_BASE: u128 = 0x0000_0000_0000_7000_8000_0000_0000_0000;

pub fn create_todo_id(n: u128) -> Uuid {
    Uuid::from_u128(UUID_BASE + n)
}

pub fn get_todo_ids(data: &Value) -> Vec<String> {
    data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["todoId"].as_str().unwrap().to_owned())
        .collect()
}
