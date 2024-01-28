use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    thread::ScopedJoinHandle,
    time::Instant,
};

use async_recursion::async_recursion;
use minijinja::Environment;
use tracing::{debug, error, info};

use crate::{
    db::Database,
    document::{Directory, Document},
    error::KnawledgeError,
    notifiy::NotifierHandle,
    FILES_PER_THREAD, MAX_THREADS,
};

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

    #[async_recursion(?Send)]
    pub async fn process_directory(
        &self,
        path: impl AsRef<Path> + 'async_recursion,
        parent: Option<uuid::Uuid>,
    ) -> Result<(), KnawledgeError> {
        let entries = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let full_path = path.as_ref().display().to_string();
        let dir_name = path
            .as_ref()
            .file_name()
            .ok_or(KnawledgeError::InvalidDirectory(format!(
                "{full_path}: unsupported directory"
            )))?;

        debug!("Loading {full_path}");

        let dir_name = dir_name
            .to_str()
            .ok_or(KnawledgeError::InvalidDirectory(format!(
                "{dir_name:?}: not valid utf-8"
            )))?;

        let directory_entry: Directory = match parent {
            Some(parent_id) => {
                let parent = self
                    .db
                    .get_dir_by_name_and_parent(dir_name, parent_id)
                    .await?;

                match parent {
                    Some(dir) => dir,
                    None => self.db.insert_directory(dir_name, Some(parent_id)).await?,
                }
            }
            None => {
                let root = self.db.get_root_dir_by_name(dir_name).await?;
                match root {
                    Some(dir) => dir,
                    None => self.db.insert_directory(dir_name, None).await?,
                }
            }
        };

        for entry in entries.iter() {
            if entry.path().is_dir() {
                self.process_directory(entry.path(), Some(directory_entry.id))
                    .await?;
            }
        }

        let mut files_processed = vec![];
        let mut markdown_files = vec![];
        let mut file_names = vec![];

        for entry in entries.iter() {
            let path = entry.path();
            let Some(ext) = path.extension() else {
                continue;
            };

            let Some(ext) = ext.to_str() else {
                continue;
            };

            if ext != "md" {
                continue;
            }

            if let Some(name) = path.file_name() {
                if let Some(name) = name.to_str() {
                    file_names.push(name.to_string());
                }
            }
            markdown_files.push(path);
        }

        let existing = self
            .db
            .list_existing(directory_entry.id, &file_names)
            .await?;

        let mut amt_files_existing = 0;
        for item in existing {
            let idx = markdown_files.iter().position(|el| {
                let Some(file_name) = el.iter().last() else {
                    return false;
                };

                let Some(file_name) = file_name.to_str() else {
                    return false;
                };

                item.file_name == file_name
            });

            if let Some(idx) = idx {
                debug!("Already exists: {} ", item.file_name);
                markdown_files.swap_remove(idx);
                amt_files_existing += 1;
            }
        }

        process_files(directory_entry.id, markdown_files, &mut files_processed)?;

        let amt_files_processed = files_processed.len();
        for file in files_processed {
            self.db.insert_document(file).await?;
        }

        info!("Existing files: {amt_files_existing} Processed files: {amt_files_processed}",);

        Ok(())
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

fn process_files(
    directory: uuid::Uuid,
    file_paths: Vec<PathBuf>,
    files: &mut Vec<Document>,
) -> Result<(), KnawledgeError> {
    let files_total = file_paths.len();

    let mut files_remaining = files_total;

    while files_remaining > 0 {
        let mut batches: Vec<&[PathBuf]> = vec![&[]; *MAX_THREADS];

        for (i, batch) in batches.iter_mut().enumerate() {
            let start = i * FILES_PER_THREAD;

            let mut end = (i + 1) * FILES_PER_THREAD;

            if end > files_total {
                end = files_total;

                *batch = &file_paths[start..end];

                files_remaining -= end - start;

                break;
            }

            *batch = &file_paths[start..end];

            files_remaining -= FILES_PER_THREAD;
        }

        type TaskWithStart<'a> = (
            ScopedJoinHandle<'a, Result<Vec<Document>, KnawledgeError>>,
            Instant,
        );

        batches.retain(|batch| !batch.is_empty());

        if batches.len() > 1 {
            debug!("Processing multiple batches");
            std::thread::scope(|scope| {
                let mut tasks: Vec<TaskWithStart> = Vec::with_capacity(*MAX_THREADS);

                for batch in batches {
                    if batch.is_empty() {
                        continue;
                    }

                    let task = scope.spawn(move || {
                        let mut files = vec![];
                        for file_path in batch {
                            let file = Document::read_md_file(directory, file_path)?;
                            files.push(file);
                        }
                        Ok(files)
                    });

                    debug!("Spawned thread {:?}", task.thread().id());

                    tasks.push((task, Instant::now()));
                }

                for (task, start) in tasks {
                    let id = task.thread().id();
                    let result = task.join();
                    match result {
                        Ok(Ok(processed)) => {
                            files.extend(processed);
                            debug!(
                                "Thread {:?} finished in {}ms",
                                id,
                                Instant::now().duration_since(start).as_nanos() as f32 * 0.001
                            );
                        }
                        Ok(Err(e)) => error!("Error occurred while processing files: {e:?}"),
                        Err(e) => error!("Error occurred while processing files: {e:?}"),
                    }
                }
            });
        } else {
            debug!("Processing single batch");
            for file_path in batches[0] {
                let file = Document::read_md_file(directory, file_path)?;
                files.push(file);
            }
        }
    }

    Ok(())
}
