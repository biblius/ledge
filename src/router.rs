use axum::{
    http::{HeaderValue, Response},
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};
use htmxpress::HtmxElement;
use markdown::Options;
use minijinja::context;
use tower_http::{services::ServeFile, trace::TraceLayer};

use crate::{
    document::find_title_from_header,
    error::KnawledgeError,
    htmx::{MainDocumentHtmx, SidebarDocumentHtmx},
    state::{DocumentCache, State},
};
use std::fmt::Write;

pub fn router(state: State) -> Router {
    Router::new()
        .route(
            "/styles.css",
            get_service(ServeFile::new("public/styles.css")),
        )
        .route(
            "/htmx.min.js",
            get_service(ServeFile::new("public/htmx.min.js")),
        )
        .route("/index.js", get_service(ServeFile::new("public/index.js")))
        .route("/", get(index))
        .route("/main/*path", get(document_main))
        .route("/documents", get(documents))
        .route("/*path", get(document))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn documents(
    mut state: axum::extract::State<State>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let documents = state.cache.list()?;

    let docs = documents.into_iter().fold(String::new(), |mut acc, d| {
        if d.file_name.ends_with("index.md") {
            return acc;
        }

        let title = d
            .title
            .or(find_title_from_header(&d.content))
            .unwrap_or_else(|| d.file_name.clone());

        let _ = write!(
            acc,
            "{}",
            SidebarDocumentHtmx::new(title, d.file_name).to_htmx()
        );
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
        .map(|doc| markdown::to_html_with_options(&doc.content, &Options::gfm()).unwrap())
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
    mut state: axum::extract::State<State>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, KnawledgeError> {
    let main = state
        .cache
        .get_ref(&path.0)?
        .map(|mut doc| {
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
    mut state: axum::extract::State<State>,
    path: axum::extract::Path<String>,
) -> Result<impl IntoResponse, KnawledgeError> {
    dbg!(&path);

    let doc = state
        .cache
        .get_ref(&path.0)?
        .map(|mut doc| {
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
