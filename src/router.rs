use crate::{
    auth::Auth,
    document::models::DirectoryEntry,
    document::{DocumentData, DocumentMeta},
    error::KnawledgeError,
    htmx::{
        DocumentHeadHtmx, MainDocumentHtmx, SidebarContainer, SidebarDirectoryHtmx,
        SidebarDocumentHtmx,
    },
    state::Documents,
};
use axum::{
    http::{HeaderValue, Response},
    response::IntoResponse,
    routing::get,
    Router,
};
use htmxpress::HtmxElement;
use markdown::Options;
use minijinja::context;
use std::{fmt::Write, str::FromStr};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::info;

use self::admin::admin_router;

mod admin;

const DEFAULT_TITLE: &str = "Knawledger";

pub fn router(state: Documents, auth: Option<Auth>) -> Router {
    let router = public_router(state.clone());

    if let Some(auth) = auth {
        router.merge(admin_router(state, auth))
    } else {
        router
    }
    .layer(TraceLayer::new_for_http())
}

fn public_router(state: Documents) -> Router {
    Router::new()
        .nest_service("/public", ServeDir::new("public"))
        .route("/", get(index_page))
        .route("/main/:id", get(document_main))
        .route("/meta/:id", get(document_meta))
        .route("/side", get(sidebar_init))
        .route("/side/:id", get(sidebar_entries))
        .route("/document/:path", get(document_page))
        .with_state(state)
}

pub async fn index_page(
    state: axum::extract::State<Documents>,
) -> Result<impl IntoResponse, KnawledgeError> {
    info!("Loading index");

    let doc_path = state.db.get_index_path().await?;

    // Try to find the custom index.md, return default if missing
    let Some(path) = doc_path else {
        let template = state.context.get_template("index")?;
        let page_title = state.title.as_deref().unwrap_or(DEFAULT_TITLE);
        let main = template
            .render(context! { title => "Knawledger", page_title => page_title,  main => "The only true wisdom lies in knowing you know nothing." })?;
        return Ok(htmx_response(main));
    };

    let index = DocumentData::read_from_disk(path)?;

    let title = index.meta.title.as_deref().unwrap_or(DEFAULT_TITLE);

    let main = markdown::to_html_with_options(&index.content, &Options::gfm()).unwrap();

    let template = state.context.get_template("index")?;

    let page_title = state.title.as_deref().unwrap_or(DEFAULT_TITLE);

    let main =
        template.render(context! { title => title, page_title => page_title,  main => main })?;

    Ok(htmx_response(main))
}

pub async fn document_page(
    state: axum::extract::State<Documents>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let uuid = uuid::Uuid::from_str(&path);

    let Ok(uuid) = uuid else {
        let Some(path) = state.db.get_doc_path_by_custom_id(&path).await? else {
            return Err(KnawledgeError::NotFound(path.0));
        };

        info!("Reading {path}");

        let mut document = DocumentData::read_from_disk(path)?;

        document.content =
            markdown::to_html_with_options(&document.content, &Options::gfm()).unwrap();

        let title = document
            .meta
            .title
            .as_deref()
            .unwrap_or(DEFAULT_TITLE)
            .to_string();

        let main = MainDocumentHtmx::new_page(document).to_htmx();

        let template = state.context.get_template("index")?;
        let page_title = state.title.as_deref().unwrap_or(DEFAULT_TITLE);
        let page =
            template.render(context! { title => title, page_title => page_title, main => main })?;

        return Ok(htmx_response(page));
    };

    let doc_path = state.db.get_doc_path(uuid).await?;

    let Some(path) = doc_path else {
        return Err(KnawledgeError::NotFound(path.0.to_string()));
    };

    info!("Reading {path}");

    let mut document = DocumentData::read_from_disk(path)?;
    document.content = markdown::to_html_with_options(&document.content, &Options::gfm()).unwrap();

    let title = document
        .meta
        .title
        .as_deref()
        .unwrap_or(DEFAULT_TITLE)
        .to_string();

    let main = MainDocumentHtmx::new_page(document).to_htmx();

    let template = state.context.get_template("index")?;
    let page_title = state.title.as_deref().unwrap_or(DEFAULT_TITLE);
    let page =
        template.render(context! { title => title, page_title => page_title, main => main })?;

    Ok(htmx_response(page))
}

pub async fn document_main(
    state: axum::extract::State<Documents>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let uuid = uuid::Uuid::from_str(&path);

    let read_data = |path: &str| -> Result<String, KnawledgeError> {
        let mut document = DocumentData::read_from_disk(path)?;
        document.content =
            markdown::to_html_with_options(&document.content, &Options::gfm()).unwrap();
        Ok(MainDocumentHtmx::new_main(
            document.meta.title.as_deref().map(DocumentHeadHtmx::new),
            document,
        )
        .to_htmx())
    };

    let Ok(uuid) = uuid else {
        if let Some(ref path) = state.db.get_doc_path_by_custom_id(&path).await? {
            let response = read_data(path)?;

            return Ok(htmx_response(response));
        }

        if path.0 != "index" {
            return Err(KnawledgeError::NotFound(path.0));
        }

        let Some(path) = state.db.get_index_path().await? else {
            return Ok(Response::new(String::new()));
        };

        let response = read_data(&path)?;
        return Ok(htmx_response(response));
    };

    let Some(path) = state.db.get_doc_path(uuid).await? else {
        return Err(KnawledgeError::NotFound(path.0));
    };

    let response = read_data(&path)?;
    Ok(htmx_response(response))
}

pub async fn document_meta(
    state: axum::extract::State<Documents>,
    path: axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let doc_path = state.db.get_doc_path(path.0).await?;

    let Some(path) = doc_path else {
        return Err(KnawledgeError::NotFound(path.0.to_string()));
    };

    let meta = DocumentMeta::read_from_file(path)?;

    let title = meta.title.as_deref().map(DocumentHeadHtmx::new);

    if let Some(title) = title {
        return Ok(htmx_response(title.to_htmx()));
    }

    Ok(Response::new(String::new()))
}

pub async fn sidebar_init(
    state: axum::extract::State<Documents>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let docs = state.db.list_roots_with_entries().await?;

    let mut documents = vec![];

    for DirectoryEntry {
        id,
        name,
        parent,
        r#type,
        title,
        custom_id,
    } in docs
    {
        if name.ends_with("index.md") {
            continue;
        }

        // Root directories have no parent
        let Some(parent) = parent else {
            documents.push(SidebarContainer::new(id, name));
            continue;
        };

        let title = title.unwrap_or_else(|| name.clone());

        // list_roots() returns an ordered list with the roots
        // always as the first elements so we should have parents here
        let Some(ref mut parent) = documents.iter_mut().find(|d| d.id == parent) else {
            continue;
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
    }

    let docs = documents.into_iter().fold(String::new(), |mut acc, el| {
        let _ = write!(acc, "{}", el.to_htmx());
        acc
    });

    Ok(htmx_response(docs))
}

pub async fn sidebar_entries(
    state: axum::extract::State<Documents>,
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

fn htmx_response(res: String) -> Response<String> {
    let mut response = Response::new(res);

    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/html; charset=utf8"),
    );

    response
}
