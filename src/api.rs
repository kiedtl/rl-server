use std::time::Duration;

use crate::morgue::Morgue;

use axum::{
    extract::{Json, Query, State},
    http::{Method, StatusCode},
    response::{Response, Redirect, IntoResponse},
};
use chrono::{DateTime, Utc};
use object_store::{ObjectStore, signer::Signer};
use serde_json;
use serde::{Serialize, Deserialize};
use sqlx::Row;
use uuid::Uuid;

use log::{info, error};

use crate::{ AppState };

pub enum AnyOf2<A, B> {
    A(A), B(B)
}

impl<A, B> IntoResponse for AnyOf2<A, B>
where
    A: IntoResponse,
    B: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            AnyOf2::A(i) => i.into_response(),
            AnyOf2::B(i) => i.into_response(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerError {
    inner: String,
}

impl From<anyhow::Error> for ServerError {
    fn from(e: anyhow::Error) -> Self {
        Self { inner: format!("{e:#}") }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        error!("handled api error: {}", self.inner);
        (StatusCode::INTERNAL_SERVER_ERROR, self.inner).into_response()
    }
}

#[derive(Copy, Clone)]
pub enum ApiError {
    NotFound,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use StatusCode as SC;

        match self {
            ApiError::NotFound => (SC::NOT_FOUND, "Not found.").into_response(),
        }
    }
}

pub async fn upload_morgue(
    State(state): State<AppState>,
    body: axum::body::Bytes,
)
    -> Result<AnyOf2<String, ApiError>, String>
{
    info!("api: handling upload_morgue");

    let body_str = std::str::from_utf8(&body).map_err(|e| e.to_string())?;
    let q: Morgue = serde_json::from_str(body_str).map_err(|e| e.to_string())?;

    let mut conn = state.db.lock().await;

    let player_id = get_or_create_player_id(&mut *conn, &q.info.username)
        .await
        .map_err(|e| e.to_string())?;

    let r = sqlx::query(
        "INSERT INTO Scores
            (date, player, result, morgue, end_level, slain_foes, stabbed_foes)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7);"
    )
        .bind(q.info.end_datetime.to_datetime().timestamp())
        .bind(player_id)
        .bind(q.result())
        .bind(body_str)
        .bind(q.info.level)
        .bind(q.stats.vanquished_foes.total() as u32)
        .bind(q.stats.stabbed_foes.total() as u32)
        .execute(&mut *conn)
        .await
        .map_err(|err| err.to_string())?;

    if r.rows_affected() != 1 {
        Err("Internal database error".to_string())
    } else {
        Ok(AnyOf2::A("Success".to_string()))
    }
}

async fn get_or_create_player_id(
    conn: &mut sqlx::SqliteConnection,
    name: &str,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query("SELECT id FROM Players WHERE name = $1")
        .bind(name)
        .fetch_optional(&mut *conn)
        .await?;

    if let Some(row) = result {
        Ok(row.get("id"))
    } else {
        // Insert and return new id
        let id: i64 = sqlx::query_scalar("INSERT INTO Players (name) VALUES ($1) RETURNING id")
            .bind(name)
            .fetch_one(&mut *conn)
            .await?;
        Ok(id)
    }
}
