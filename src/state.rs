use std::{collections::HashMap, sync::Arc};

use minijinja::Environment;

use crate::{db::Database, document::Document, error::KnawledgeError, notifiy::NotifierHandle};

lazy_static::lazy_static! {
    pub static ref INDEX: String =
        std::fs::read_to_string("public/index.html").expect("missing template");
}

#[derive(Debug, Clone)]
pub struct State {
    pub context: Environment<'static>,

    pub db: Database,

    pub cache: HashMap<String, Document>,

    pub tx: Arc<NotifierHandle>,
}

impl State {
    pub async fn new(db: Database, tx: NotifierHandle) -> Self {
        let mut context = Environment::new();

        context
            .add_template("index", &INDEX)
            .expect("unable to load template");

        Self {
            context,
            db,
            cache: HashMap::new(),
            tx: Arc::new(tx),
        }
    }

    pub async fn cache_index(&mut self) -> Result<(), KnawledgeError> {
        let index = self.db.get_index().await?;
        if let Some(index) = index {
            self.cache.set("index.md".to_string(), index)?;
        }
        Ok(())
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
