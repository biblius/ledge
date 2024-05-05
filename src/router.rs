use crate::{
    document::models::DirectoryEntry,
    document::{DocumentData, DocumentMeta},
    error::LedgeknawError,
    state::DocumentService,
};
use axum::{http::Method, response::IntoResponse, routing::get, Json, Router};
use axum_macros::debug_handler;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::info;

pub fn router(state: DocumentService) -> Router {
    let router = public_router(state.clone());

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::POST]);

    router.layer(TraceLayer::new_for_http()).layer(cors)
}

fn public_router(state: DocumentService) -> Router {
    Router::new()
        .nest_service(
            "/",
            ServeDir::new("dist").fallback(ServeFile::new("dist/index.html")),
        )
        .route("/meta/:id", get(document_meta))
        .route("/side", get(sidebar_init))
        .route("/side/:id", get(sidebar_entries))
        .route("/document", get(index))
        .route("/document/:id", get(document))
        .with_state(state)
}

#[debug_handler]
pub async fn index(
    state: axum::extract::State<DocumentService>,
) -> Result<impl IntoResponse, LedgeknawError> {
    info!("Loading index");
    let doc_path = state.db.get_index_id_path().await?;
    let Some((id, path)) = doc_path else {
        return Err(LedgeknawError::NotFound("index.md".to_string()));
    };
    let index = DocumentData::read_from_disk(id, path)?;
    Ok(Json(index).into_response())
}

pub async fn document(
    state: axum::extract::State<DocumentService>,
    path: axum::extract::Path<String>,
) -> Result<Json<DocumentData>, LedgeknawError> {
    Ok(Json(state.read_file(path.0).await?))
}

pub async fn document_meta(
    state: axum::extract::State<DocumentService>,
    id: axum::extract::Path<uuid::Uuid>,
) -> Result<Json<DocumentMeta>, LedgeknawError> {
    Ok(Json(state.get_file_meta(*id).await?))
}

pub async fn sidebar_init(
    state: axum::extract::State<DocumentService>,
) -> Result<Json<Vec<DirectoryEntry>>, LedgeknawError> {
    let docs = state.db.list_roots().await?;
    Ok(Json(docs))
}

pub async fn sidebar_entries(
    state: axum::extract::State<DocumentService>,
    path: axum::extract::Path<uuid::Uuid>,
) -> Result<Json<Vec<DirectoryEntry>>, LedgeknawError> {
    let files = state.db.list_entries(*path).await?;
    Ok(Json(files))
}
