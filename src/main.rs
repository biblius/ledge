use clap::Parser;
use std::num::NonZeroUsize;
use tracing::info;

use crate::{
    auth::{db::AuthDatabase, Auth},
    config::{Config, StartArgs},
    document::db::DocumentDb,
    state::Documents,
};

pub const FILES_PER_THREAD: usize = 128;

lazy_static::lazy_static! {
    pub static ref MAX_THREADS: usize = std::thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap()).into();
}

pub mod auth;
pub mod config;
pub mod chunk;
pub mod config;
pub mod db;
pub mod document;
pub mod error;
pub mod llm;
pub mod notifiy;
pub mod router;
pub mod state;
pub mod vector_db;

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

    let Config {
        title,
        directories,
        admin,
    } = Config::read(config_path).expect("invalid config file");

    let document_db = DocumentDb::new(db_pool.clone()).await;
    let auth_db = AuthDatabase::new(db_pool.clone()).await;

    let state = Documents::new(document_db.clone(), title, directories);
    state.sync().await.expect("error in state sync");

    // let (tx, rx) = std::sync::mpsc::channel();

    // let roots = database
    //     .list_root_paths()
    //     .await
    //     .expect("unable to process roots")
    //     .into_iter()
    //     .collect::<HashSet<_>>();

    // let notifier = NotifyHandler::new(database.clone(), roots, rx);

    // let handle = notifier.run().expect("could not start watcher");

    // let handle = NotifierHandle { tx, handle };

    info!("Now listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("error while starting TCP listener");

    let router = router::router(state, admin.map(|config| Auth::new(auth_db, config)));

    axum::serve(listener, router)
        .await
        .expect("error while starting server");
}
