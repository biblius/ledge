use crate::{document::db::DocumentDb, document::process_root_directory, error::KnawledgeError};
use minijinja::Environment;
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::sync::RwLock;
use tracing::{trace, warn};

lazy_static::lazy_static! {
    pub static ref INDEX: String =
        std::fs::read_to_string("public/index.html").expect("missing template index.html");

    pub static ref UNAUTHORIZED: String =
        std::fs::read_to_string("public/error/unauthorized.html").expect("missing template error/unauthorized.html");
}

#[derive(Debug, Clone)]
pub struct Documents {
    pub context: Environment<'static>,

    pub db: DocumentDb,

    /// The document title for the front end
    pub title: Arc<Option<String>>,

    /// The list of directories to initially include for the public page.
    /// Maps names to directory paths.
    pub directories: Arc<RwLock<HashMap<String, String>>>,
}

impl Documents {
    pub fn new(
        db: DocumentDb,
        title: Option<String>,
        directories: HashMap<String, String>,
    ) -> Self {
        let mut context = Environment::new();

        context
            .add_template("index", &INDEX)
            .expect("unable to load `index` template");

        context
            .add_template("unauthorized", &UNAUTHORIZED)
            .expect("unable to load `unauthorized` template");

        Self {
            context,
            db,
            title: Arc::new(title),
            directories: Arc::new(RwLock::new(directories)),
        }
    }

    pub async fn sync(&self) -> Result<(), KnawledgeError> {
        let directories = self.directories.read().await;

        let paths = directories
            .values()
            .map(String::to_owned)
            .collect::<Vec<_>>();

        let full_paths = paths
            .iter()
            .map(|p| Path::new(p).canonicalize())
            .filter_map(Result::ok)
            .filter_map(|p| Some(p.to_str()?.to_owned()))
            .collect::<Vec<_>>();

        // Trim any root dirs that should not be loaded
        self.db.trim_roots(&full_paths).await?;

        // Trim any files no longer on fs
        let file_paths = self.db.get_all_file_paths().await?;
        for path in file_paths {
            if let Err(e) = tokio::fs::metadata(&path).await {
                warn!("Error while reading file {path}, trimming");
                trace!("Error: {e}");
                self.db.remove_doc_by_path(&path).await?;
            }
        }

        for (alias, path) in directories.iter() {
            process_root_directory(&self.db, path, alias).await?;
        }

        Ok(())
    }
}
