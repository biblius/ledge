use crate::{config::Config, db::Database, notifiy::NotifierHandle};
use minijinja::Environment;
use std::sync::Arc;

lazy_static::lazy_static! {
    pub static ref INDEX: String =
        std::fs::read_to_string("public/index.html").expect("missing template index.html");
}

#[derive(Debug, Clone)]
pub struct State {
    pub context: Environment<'static>,

    pub db: Database,

    pub config: Config,
    // pub tx: Arc<NotifierHandle>,
}

impl State {
    pub fn new(db: Database, config: Config) -> Self {
        let mut context = Environment::new();

        context
            .add_template("index", &INDEX)
            .expect("unable to load template");

        Self {
            context,
            db,
            config, // tx: Arc::new(tx),
        }
    }
}
