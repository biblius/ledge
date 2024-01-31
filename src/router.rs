use axum::{
    http::{HeaderValue, Response},
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use htmxpress::HtmxElement;
use markdown::Options;
use minijinja::context;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{
    db::DirectoryEntry,
    document::Document,
    error::KnawledgeError,
    htmx::{MainDocumentHtmx, SidebarContainer, SidebarDirectoryHtmx, SidebarDocumentHtmx},
    state::State,
};
use std::{collections::HashMap, fmt::Write, str::FromStr};

pub fn router(state: State) -> Router {
    Router::new()
        .nest_service("/public", ServeDir::new("public"))
        .route(
            "/favicon.ico",
            get(Redirect::permanent("/public/favicon.ico")),
        )
        .route("/", get(index))
        .route("/main/*path", get(document_main))
        .route("/documents", get(documents))
        .route("/side/*id", get(sidebar_entries))
        .route("/*path", get(document))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
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
             ..
         }| {
            let title = title.unwrap_or_else(|| name.clone());
            match r#type.as_str() {
                "f" => {
                    let _ = write!(acc, "{}", SidebarDocumentHtmx::new(title, id).to_htmx());
                }
                "d" => {
                    let _ = write!(acc, "{}", SidebarDirectoryHtmx::new(title, id).to_htmx());
                }
                _ => unreachable!(),
            }
            acc
        },
    );

    let mut response = Response::new(htmx);

    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/html; charset=utf8"),
    );

    Ok(response)
}

pub async fn documents(
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
                    parent.documents.push(SidebarDocumentHtmx::new(title, id));
                }
                "d" => {
                    parent
                        .directories
                        .push(SidebarDirectoryHtmx::new(title, id));
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

    let mut response = Response::new(docs);

    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/html; charset=utf8"),
    );

    Ok(response)
}

pub async fn index(
    state: axum::extract::State<State>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let main = state
        .cache
        .get("index.md")
        .map(|doc| markdown::to_html_with_options(&doc.file_name, &Options::gfm()).unwrap())
        .unwrap_or("Hello world".to_string());

    let template = state.context.get_template("index")?;
    let main = template.render(context! {main => main})?;
    let mut response = Response::new(main);

    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/html; charset=utf8"),
    );

    Ok(response)
}

pub async fn document(
    state: axum::extract::State<State>,
    path: axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let main = state
        .db
        .get_document_path(path.0)
        .await?
        .map(|doc| {
            let mut doc = Document::collect_data(doc).unwrap();
            doc.content = markdown::to_html_with_options(&doc.content, &Options::gfm()).unwrap();
            MainDocumentHtmx::from(doc).to_htmx()
        })
        .unwrap_or("Hello world".to_string());

    let template = state.context.get_template("index")?;
    let main = template.render(context! {main => main})?;
    let mut response = Response::new(main);

    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/html; charset=utf8"),
    );

    Ok(response)
}

pub async fn document_main(
    state: axum::extract::State<State>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let uuid = uuid::Uuid::from_str(&path);
    let Ok(uuid) = uuid else {
        if path.0 == "index" {
            let index = state
                .cache
                .get("index.md")
                .map(|doc| markdown::to_html_with_options(&doc.file_name, &Options::gfm()).unwrap())
                .unwrap_or("Hello world".to_string());

            let mut response = Response::new(index);

            response.headers_mut().insert(
                "content-type",
                HeaderValue::from_static("text/html; charset=utf8"),
            );

            return Ok(response);
        } else {
            return Err(KnawledgeError::DoesNotExist(path.0));
        }
    };

    let doc = state
        .db
        .get_document_path(uuid)
        .await?
        .map(|doc| {
            let mut doc = Document::collect_data(doc).unwrap();
            doc.content = markdown::to_html_with_options(&doc.content, &Options::gfm()).unwrap();
            MainDocumentHtmx::from(doc).to_htmx()
        })
        .unwrap_or("Hello world".to_string());

    let mut response = Response::new(doc);

    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/html; charset=utf8"),
    );

    Ok(response)
}
