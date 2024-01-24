use state::DocumentCache;
use std::num::NonZeroUsize;
use tracing::{info, Level};

use crate::{document::extract_dir, state::State};

pub const FILES_PER_THREAD: usize = 128;

lazy_static::lazy_static! {
    pub static ref MAX_THREADS: usize = std::thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap()).into();
}

pub mod db;
pub mod document;
pub mod error;
pub mod htmx;
pub mod router;
pub mod state;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");

    let host = "127.0.0.1";
    let port = "3002";

    let addr = format!("{host}:{port}");

    let mut state = State::new(&db_url).await;

    let (existing, new) = extract_dir(&state, "content").await.unwrap();

    for file in existing.into_iter() {
        state.cache.set(file.file_name.clone(), file).unwrap();
    }

    for file in new.into_iter() {
        state.cache.set(file.file_name.clone(), file).unwrap();
    }

    info!("Now listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("error while starting TCP listener");

    let router = router::router(state);

    axum::serve(listener, router)
        .await
        .expect("error while starting server");
}
