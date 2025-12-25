use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;
use torrentino_core::SanitizedConfig;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<SanitizedConfig> {
    Json(state.sanitized_config())
}
