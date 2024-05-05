use clap::Parser;
use std::num::NonZeroUsize;
use tracing::info;

use crate::{
    config::{Config, StartArgs},
    document::db::DocumentDb,
    state::DocumentService,
};

pub const FILES_PER_THREAD: usize = 128;

lazy_static::lazy_static! {
    pub static ref MAX_THREADS: usize = std::thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap()).into();
}

pub mod config;
pub mod db;
pub mod document;
pub mod error;
pub mod router;
pub mod state;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let StartArgs {
        config_path,
        address: host,
        port,
        log_level: level,
    } = StartArgs::parse();

    tracing_subscriber::fmt().with_max_level(level).init();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let db_pool = db::create_pool(&db_url).await;

    db::migrate(&db_pool).await;

    let addr = format!("{host}:{port}");

    let Config { title, directories } = Config::read(config_path).expect("invalid config file");

    let document_db = DocumentDb::new(db_pool.clone()).await;

    let documents = DocumentService::new(document_db.clone(), title, directories);
    documents.sync().await.expect("error in state sync");

    info!("Now listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("error while starting TCP listener");

    let router = router::router(documents);

    axum::serve(listener, router)
        .await
        .expect("error while starting server");
}
