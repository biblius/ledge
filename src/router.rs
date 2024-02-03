use axum::{
    http::{HeaderValue, Response},
    response::IntoResponse,
    routing::get,
    Router,
};
use htmxpress::HtmxElement;
use markdown::Options;
use minijinja::context;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::info;

use crate::{
    db::models::DirectoryEntry,
    document::{DocumentData, DocumentMeta},
    error::KnawledgeError,
    htmx::{MainDocumentHtmx, SidebarContainer, SidebarDirectoryHtmx, SidebarDocumentHtmx},
    state::State,
};
use std::{collections::HashMap, fmt::Write, str::FromStr};

pub fn router(state: State) -> Router {
    Router::new()
        .nest_service("/public", ServeDir::new("public"))
        .route("/", get(index))
        .route("/main/*id", get(document_main))
        .route("/meta/*id", get(document_meta))
        .route("/side", get(sidebar_init))
        .route("/side/*id", get(sidebar_entries))
        .route("/*path", get(document))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn document_meta(
    state: axum::extract::State<State>,
    path: axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let doc_path = state.db.get_document_path(path.0).await?;

    let Some(path) = doc_path else {
        return Err(KnawledgeError::DoesNotExist(path.0.to_string()));
    };

    let meta = DocumentMeta::read_from_file(path)?;

    Ok(String::new())
}

pub async fn sidebar_entries(
    state: axum::extract::State<State>,
    path: axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let documents = state.db.list_entries(path.0).await?;

    let htmx = documents.into_iter().fold(
        String::new(),
        |mut acc,
         DirectoryEntry {
             id,
             name,
             r#type,
             title,
             custom_id,
             ..
         }| {
            let title = title.unwrap_or_else(|| name.clone());
            match r#type.as_str() {
                "f" => {
                    let _ = write!(
                        acc,
                        "{}",
                        SidebarDocumentHtmx::new(
                            title,
                            custom_id.unwrap_or_else(|| id.to_string())
                        )
                        .to_htmx()
                    );
                }
                "d" => {
                    let _ = write!(
                        acc,
                        "{}",
                        SidebarDirectoryHtmx::new(
                            title,
                            custom_id.unwrap_or_else(|| id.to_string())
                        )
                        .to_htmx()
                    );
                }
                _ => unreachable!(),
            }
            acc
        },
    );

    Ok(htmx_response(htmx))
}

pub async fn sidebar_init(
    state: axum::extract::State<State>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let documents = state.db.list_roots_with_entries().await?;

    let docs = documents.into_iter().fold(
        HashMap::new(),
        |mut acc: HashMap<_, SidebarContainer>,
         DirectoryEntry {
             id,
             name,
             parent,
             r#type,
             title,
             custom_id,
         }| {
            if name.ends_with("index.md") {
                return acc;
            }

            // Root directories have no parent
            if parent.is_none() {
                acc.insert(id, SidebarContainer::new(name));
                return acc;
            }

            let title = title.unwrap_or_else(|| name.clone());
            let parent = parent.unwrap();

            // list_roots() returns an ordered list with the roots
            // always as the first elements
            let Some(parent) = acc.get_mut(&parent) else {
                return acc;
            };

            match r#type.as_str() {
                "f" => {
                    parent.documents.push(SidebarDocumentHtmx::new(
                        title,
                        custom_id.unwrap_or_else(|| id.to_string()),
                    ));
                }
                "d" => {
                    parent
                        .directories
                        .push(SidebarDirectoryHtmx::new(title, id.to_string()));
                }
                _ => unreachable!(),
            }

            acc
        },
    );

    let docs = docs.values().fold(String::new(), |mut acc, el| {
        let _ = write!(acc, "{}", el.to_htmx());
        acc
    });

    Ok(htmx_response(docs))
}

pub async fn index(
    state: axum::extract::State<State>,
) -> Result<impl IntoResponse, KnawledgeError> {
    info!("Loading index");

    let doc_path = state.db.get_index_path().await?;
    let Some(path) = doc_path else {
        return Ok(Response::new(String::from("Hello world")));
    };

    let index = DocumentData::read_from_disk(path)?;
    let main = markdown::to_html_with_options(&index.content, &Options::gfm()).unwrap();
    let template = state.context.get_template("index")?;
    let main = template.render(context! {main => main})?;

    Ok(htmx_response(main))
}

pub async fn document(
    state: axum::extract::State<State>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let uuid = uuid::Uuid::from_str(&path);

    let Ok(uuid) = uuid else {
        if let Some(path) = state.db.get_document_path_by_custom_id(&path).await? {
            info!("Reading {path}");
            let mut document = DocumentData::read_from_disk(path)?;
            document.content =
                markdown::to_html_with_options(&document.content, &Options::gfm()).unwrap();
            let main = MainDocumentHtmx::from(document).to_htmx();

            let page = state.context.get_template("index")?;
            let page = page.render(context! {main => main})?;

            return Ok(htmx_response(page));
        } else {
            return Err(KnawledgeError::DoesNotExist(path.0));
        }
    };

    let doc_path = state.db.get_document_path(uuid).await?;

    let Some(path) = doc_path else {
        return Err(KnawledgeError::NotFound(path.0.to_string()));
    };

    info!("Reading {path}");

    let mut doc = DocumentData::read_from_disk(path)?;
    doc.content = markdown::to_html_with_options(&doc.content, &Options::gfm()).unwrap();

    let main = MainDocumentHtmx::from(doc).to_htmx();
    let template = state.context.get_template("index")?;

    let response = template.render(context! {main => main})?;

    Ok(htmx_response(response))
}

pub async fn document_main(
    state: axum::extract::State<State>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let uuid = uuid::Uuid::from_str(&path);
    let Ok(uuid) = uuid else {
        if let Some(path) = state.db.get_document_path_by_custom_id(&path).await? {
            let index = DocumentData::read_from_disk(path)?;
            let index = markdown::to_html_with_options(&index.content, &Options::gfm()).unwrap();

            return Ok(htmx_response(index));
        } else if path.0 == "index" {
            let Some(path) = state.db.get_index_path().await? else {
                return Ok(Response::new(String::from("Hello world")));
            };
            let index = DocumentData::read_from_disk(path)?;
            let index = markdown::to_html_with_options(&index.content, &Options::gfm()).unwrap();

            return Ok(htmx_response(index));
        } else {
            return Err(KnawledgeError::DoesNotExist(path.0));
        }
    };

    let doc = state
        .db
        .get_document_path(uuid)
        .await?
        .map(|path| {
            info!("Reading {path}");
            let mut doc = DocumentData::read_from_disk(path).unwrap();
            doc.content = markdown::to_html_with_options(&doc.content, &Options::gfm()).unwrap();
            MainDocumentHtmx::from(doc).to_htmx()
        })
        .unwrap_or("Hello world".to_string());

    Ok(htmx_response(doc))
}

fn htmx_response(res: String) -> Response<String> {
    let mut response = Response::new(res);

    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/html; charset=utf8"),
    );

    response
}
