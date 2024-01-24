use std::collections::HashMap;

use minijinja::Environment;

use crate::{db::Database, document::Document, error::KnawledgeError};

lazy_static::lazy_static! {
    pub static ref INDEX: String =
        std::fs::read_to_string("public/index.html").expect("missing template");
}

#[derive(Debug, Clone)]
pub struct State {
    pub context: Environment<'static>,

    pub db: Database,

    pub cache: HashMap<String, Document>,
}

impl State {
    pub async fn migrate(&self) {
        sqlx::migrate!()
            .run(&self.db.pool)
            .await
            .expect("error in migrations")
    }

    pub async fn new(db_url: &str) -> Self {
        let mut env = Environment::new();

        env.add_template("index", &INDEX)
            .expect("unable to load template");

        let client = sqlx::postgres::PgPool::connect(db_url)
            .await
            .expect("error while connecting to db");

        Self {
            context: env,
            db: Database { pool: client },
            cache: HashMap::new(),
        }
    }
}

pub trait DocumentCache {
    fn list(&mut self) -> Result<Vec<Document>, KnawledgeError>;
    fn set(&mut self, file_name: String, document: Document) -> Result<(), KnawledgeError>;
    fn get_ref(&mut self, file_name: &str) -> Result<Option<Document>, KnawledgeError>;
}

impl DocumentCache for HashMap<String, Document> {
    fn set(&mut self, file_name: String, document: Document) -> Result<(), KnawledgeError> {
        self.insert(file_name.to_string(), document);
        Ok(())
    }

    fn get_ref(&mut self, file_name: &str) -> Result<Option<Document>, KnawledgeError> {
        Ok(self.get(file_name).cloned())
    }

    fn list(&mut self) -> Result<Vec<Document>, KnawledgeError> {
        Ok(self.values().cloned().collect())
    }
}
