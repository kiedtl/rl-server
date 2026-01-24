use std::time::Duration;

use crate::morgue::Morgue;

use axum::{
    extract::{Multipart, Json, Query, State},
    http::{self, Method, StatusCode},
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

#[derive(Clone)]
pub enum ApiError {
    // NotFound,
    BadRequest,
    BadRequestCustom(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use StatusCode as SC;

        match self {
            // ApiError::NotFound => (SC::NOT_FOUND, "Not found.").into_response(),
            ApiError::BadRequest => (SC::BAD_REQUEST, "Bad request.").into_response(),
            ApiError::BadRequestCustom(s) => (SC::BAD_REQUEST, s).into_response(),
        }
    }
}

pub async fn upload_morgue(
    State(state): State<AppState>,
    body: axum::body::Bytes,
)
    -> Result<AnyOf2<String, ApiError>, String>
{
    match upload_morgue_helper(state, body).await {
        Ok(Ok(_)) => Ok(AnyOf2::A("Success".into())),
        Ok(Err(e)) => Ok(AnyOf2::B(ApiError::BadRequestCustom(e))),
        Err(e) => Err(e),
    }
}

pub async fn upload_morgue_form(
    State(state): State<AppState>,
    mut multipart: Multipart,
)
    -> Result<Response, String>
{
    let Ok(Some(field)) = multipart.next_field().await else {
        return Ok(ApiError::BadRequest.into_response());
    };

    match field.name() {
        Some("file") => {
            let Ok(data) = field.bytes().await else {
                return Ok(ApiError::BadRequest.into_response());
            };
            match upload_morgue_helper(state, data).await {
                Ok(Ok(morgue_id)) => {
                    let loc = format!("/s/{morgue_id}");
                    Ok((StatusCode::SEE_OTHER, [(http::header::LOCATION, loc)]).into_response())
                },
                Ok(Err(s)) => Ok(ApiError::BadRequestCustom(s).into_response()),
                Err(e) => Err(e),
            }
        }
        _ => return Ok(ApiError::BadRequest.into_response()),
    }
}

async fn upload_morgue_helper(
    state: AppState,
    body: axum::body::Bytes,
)
    -> Result<Result<i64, String>, String>
{
    info!("api: handling upload_morgue");
    let mut conn = state.db.lock().await;

    let body_str = std::str::from_utf8(&body).map_err(|e| e.to_string())?;
    let q: Morgue = match serde_json::from_str(body_str) {
        Ok(m) => m,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let seed = q.info.seed.cast_signed(); // Sqlite3 doesn't support u64
    let timestamp = q.info.end_datetime.to_datetime().timestamp();

    let already_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM Scores WHERE date = ? AND seed = ?)"
    )
        .bind(timestamp)
        .bind(seed)
        .fetch_one(&mut *conn)
        .await
        .map_err(|v| v.to_string())?;

    if already_exists {
        return Ok(Err(format!("Morgue with timestamp {} and seed {} already exists", timestamp, q.info.seed)));
    }

    let player_id = get_or_create_player_id(&mut *conn, &q.info.username)
        .await
        .map_err(|e| e.to_string())?;

    let morgue_id: i64 = sqlx::query_scalar(
        "INSERT INTO Scores
            (date, player, result, morgue, end_level, slain_foes, stabbed_foes, seed)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id;"
    )
        .bind(timestamp)
        .bind(player_id)
        .bind(q.result())
        .bind(body_str)
        .bind(q.info.level)
        .bind(q.stats.vanquished_foes.total() as u32)
        .bind(q.stats.stabbed_foes.total() as u32)
        .bind(seed)
        .fetch_one(&mut *conn)
        .await
        .map_err(|err| err.to_string())?;

    Ok(Ok(morgue_id))
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
