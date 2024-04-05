use crate::{
    auth::Auth,
    document::models::DirectoryEntry,
    document::{DocumentData, DocumentMeta},
    error::KnawledgeError,
    state::Documents,
};
use axum::{http::Method, response::IntoResponse, routing::get, Json, Router};
use axum_macros::debug_handler;
use std::str::FromStr;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::info;

use self::admin::admin_router;

mod admin;

pub fn router(state: Documents, auth: Option<Auth>) -> Router {
    let router = public_router(state.clone());

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::POST]);

    if let Some(auth) = auth {
        router.merge(admin_router(state, auth))
    } else {
        router
    }
    .layer(TraceLayer::new_for_http())
    .layer(cors)
}

fn public_router(state: Documents) -> Router {
    Router::new()
        .nest_service("/public", ServeDir::new("public"))
        .route("/meta/:id", get(document_meta))
        .route("/side", get(sidebar_init))
        .route("/side/:id", get(sidebar_entries))
        .route("/document", get(index))
        .route("/document/:path", get(document))
        .with_state(state)
}

#[debug_handler]
pub async fn index(
    state: axum::extract::State<Documents>,
) -> Result<impl IntoResponse, KnawledgeError> {
    info!("Loading index");
    let doc_path = state.db.get_index_id_path().await?;
    let Some((id, path)) = doc_path else {
        return Err(KnawledgeError::NotFound("index.md".to_string()));
    };
    let index = DocumentData::read_from_disk(id, path)?;
    Ok(Json(index).into_response())
}

pub async fn document(
    state: axum::extract::State<Documents>,
    path: axum::extract::Path<String>,
) -> Result<Json<DocumentData>, KnawledgeError> {
    let uuid = uuid::Uuid::from_str(&path);

    let Ok(uuid) = uuid else {
        let Some((id, path)) = state.db.get_doc_id_path_by_custom_id(&path).await? else {
            return Err(KnawledgeError::NotFound(path.0));
        };

        info!("Reading {path}");
        let document = DocumentData::read_from_disk(id, path)?;
        return Ok(Json(document));
    };

    let doc_path = state.db.get_doc_path(uuid).await?;

    let Some(path) = doc_path else {
        return Err(KnawledgeError::NotFound(path.0.to_string()));
    };

    info!("Reading {path}");
    let document = DocumentData::read_from_disk(uuid, path)?;
    Ok(Json(document))
}

pub async fn document_meta(
    state: axum::extract::State<Documents>,
    path: axum::extract::Path<uuid::Uuid>,
) -> Result<Json<DocumentMeta>, KnawledgeError> {
    let doc_path = state.db.get_doc_path(path.0).await?;
    let Some(path) = doc_path else {
        return Err(KnawledgeError::NotFound(path.0.to_string()));
    };
    let meta = DocumentMeta::read_from_file(path)?;
    Ok(Json(meta))
}

pub async fn sidebar_init(
    state: axum::extract::State<Documents>,
) -> Result<Json<Vec<DirectoryEntry>>, KnawledgeError> {
    let docs = state.db.list_roots().await?;
    Ok(Json(docs))
}

pub async fn sidebar_entries(
    state: axum::extract::State<Documents>,
    path: axum::extract::Path<uuid::Uuid>,
) -> Result<Json<Vec<DirectoryEntry>>, KnawledgeError> {
    let files = state.db.list_entries(path.0).await?;
    Ok(Json(files))
}
