use self::db::DocumentDb;
use self::models::Document;
use crate::error::LedgeknawError;
use crate::{FILES_PER_THREAD, MAX_THREADS};
use async_recursion::async_recursion;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{self, DirEntry};
use std::path::PathBuf;
use std::thread::ScopedJoinHandle;
use std::time::Instant;
use std::{fmt::Debug, path::Path};
use tracing::{debug, error, info};

pub mod db;
pub mod models;

/// Document read from the fs with its metadata.
#[derive(Debug, Default, Serialize)]
pub struct DocumentData {
    /// Database ID
    pub id: uuid::Uuid,
    /// Document markdown content
    pub content: String,
    /// Metadata
    pub meta: DocumentMeta,
}

impl DocumentData {
    pub fn read_from_disk(id: uuid::Uuid, path: impl AsRef<Path>) -> Result<Self, LedgeknawError> {
        debug!("Reading {}", path.as_ref().display());

        let mut data = Self {
            id,
            ..Default::default()
        };
        let content = fs::read_to_string(path)?;
        let (meta, content) = DocumentMeta::from_str(&content)?;
        data.content = content.to_string();
        data.meta = meta;
        Ok(data)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DocumentMeta {
    /// A user specified identifier for the document for
    /// URLs on Ledgeknaw. Prioritised over the document UUID.
    #[serde(alias = "id")]
    pub custom_id: Option<String>,
    pub title: Option<String>,
    pub reading_time: Option<i32>,
    pub tags: Option<Vec<String>>,
}

impl DocumentMeta {
    fn name_from_fs(path: impl AsRef<Path>) -> String {
        path.as_ref()
            .file_name()
            .unwrap_or(OsStr::new("__unknown"))
            .to_str()
            .unwrap_or("__unknown")
            .to_string()
    }

    pub fn read_from_file(path: impl AsRef<Path>) -> Result<Self, LedgeknawError> {
        debug!("Reading {}", path.as_ref().display());
        let content = fs::read_to_string(path)?;
        Ok(Self::from_str(&content)?.0)
    }

    /// Used when we already read the file from the fs.
    /// Returns the read meta and the remainder of the content.
    pub fn from_str(content: &str) -> Result<(Self, &str), LedgeknawError> {
        let mut data = Self {
            title: Self::find_title_from_h1(content),
            ..Default::default()
        };

        if !content.starts_with("---") {
            return Ok((data, content));
        }

        if content.len() < 4 {
            return Ok((data, content));
        }

        let Some(end_i) = &content[3..].find("---") else {
            return Ok((data, &content[3..]));
        };

        // Offset to account for the skipped ---
        let meta_str = &content[3..*end_i + 2];

        if meta_str.is_empty() {
            return Ok((data, &content[end_i + 6..]));
        }

        data = serde_yaml::from_str(meta_str)?;

        let content = &content[end_i + 6..];

        data.reading_time = Some(Self::calculate_reading_time(content));

        if data.title.is_none() {
            data.title = Self::find_title_from_h1(content);
        }

        Ok((data, content))
    }

    fn find_title_from_h1(content: &str) -> Option<String> {
        for line in content.lines() {
            let line = line.trim();
            let Some((_, title)) = line.split_once('#') else {
                continue;
            };

            return Some(title.trim().to_string());
        }

        None
    }

    fn calculate_reading_time(content: &str) -> i32 {
        let words = content.split(' ').collect::<Vec<_>>().len();
        ((words / 200) as f32 * 0.60) as i32
    }
}

#[derive(Debug, Default)]
pub struct Directory {
    pub id: uuid::Uuid,
    pub name: String,
    pub path: String,

    /// Present only in root directories
    pub alias: Option<String>,

    /// Present only in nested directories
    pub parent: Option<uuid::Uuid>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_recursion]
pub async fn process_directory(
    db: &DocumentDb,
    path: impl AsRef<Path> + 'async_recursion + Send,
    parent_id: uuid::Uuid,
) -> Result<(), LedgeknawError> {
    let full_path = path.as_ref().canonicalize()?.display().to_string();
    debug!("Loading {full_path}");

    // Normalize dir name
    let dir_name = get_valid_name(path.as_ref())?;

    // Attempt to find existing parent
    let parent = db.get_dir_by_name_and_parent(dir_name, parent_id).await?;
    let directory = match parent {
        Some(dir) => dir,
        None => db.insert_dir(&full_path, dir_name, parent_id).await?,
    };

    // Scan contents, call this fn again on directories
    let entries = fs::read_dir(&path)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    for entry in entries.iter() {
        if entry.path().is_dir() {
            process_directory(db, entry.path(), directory.id).await?;
        }
    }

    read_and_store_directory_files(db, &entries, &directory).await?;

    Ok(())
}

pub async fn process_root_directory(
    db: &DocumentDb,
    path: impl AsRef<Path>,
    alias: &str,
) -> Result<(), LedgeknawError> {
    let entries = fs::read_dir(&path)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let full_path = path.as_ref().canonicalize()?.display().to_string();
    debug!("Loading {full_path}");

    let dir_name = get_valid_name(path.as_ref())?;

    let root = db.get_root_dir_by_name(dir_name).await?;
    let directory = match root {
        Some(dir) => dir,
        None => db.insert_root_dir(&full_path, dir_name, alias).await?,
    };

    for entry in entries.iter() {
        if entry.path().is_dir() {
            process_directory(db, entry.path(), directory.id).await?;
        }
    }

    read_and_store_directory_files(db, &entries, &directory).await?;

    Ok(())
}

async fn read_and_store_directory_files(
    db: &DocumentDb,
    entries: &[DirEntry],
    directory_entry: &Directory,
) -> Result<(), LedgeknawError> {
    // Collect md files
    let mut md_files = vec![];
    let mut file_names = vec![];

    for entry in entries.iter() {
        let path = entry.path();

        if path.is_dir() {
            continue;
        }

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

        md_files.push(path);
    }

    // Compare with existing entries from DB

    let existing = db
        .list_document_in_dir(directory_entry.id, &file_names)
        .await?;
    let mut amt_files_existing = 0;

    for item in existing {
        let idx = md_files.iter().position(|el| {
            let Some(file_name) = el.iter().last() else {
                return false;
            };

            let Some(file_name) = file_name.to_str() else {
                return false;
            };

            item.file_name == file_name
        });

        // Remove file from processing list if it exists

        if let Some(idx) = idx {
            debug!("Already exists: {}", item.file_name);
            md_files.swap_remove(idx);
            amt_files_existing += 1;
        }
    }

    let files_processed = process_files(directory_entry.id, md_files)?;

    for (file, meta) in files_processed.iter() {
        db.insert_doc(file, meta).await?;
    }

    info!(
        "{} - Existing files: {amt_files_existing} Processed files: {}",
        directory_entry.name,
        files_processed.len()
    );

    Ok(())
}

fn process_files(
    directory: uuid::Uuid,
    file_paths: Vec<PathBuf>,
) -> Result<Vec<(Document, DocumentMeta)>, LedgeknawError> {
    let files_total = file_paths.len();
    let mut files_remaining = files_total;

    let mut files = vec![];

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
            ScopedJoinHandle<'a, Result<Vec<(Document, DocumentMeta)>, LedgeknawError>>,
            Instant,
        );

        batches.retain(|batch| !batch.is_empty());

        if batches.len() > 1 {
            debug!("Processing multiple ({}) batches", batches.len());
            std::thread::scope(|scope| {
                let mut tasks: Vec<TaskWithStart> = Vec::with_capacity(*MAX_THREADS);

                for batch in batches {
                    if batch.is_empty() {
                        continue;
                    }

                    let task = scope.spawn(move || {
                        let mut files = vec![];
                        for file_path in batch {
                            let file_name = DocumentMeta::name_from_fs(file_path.canonicalize()?);
                            let document = Document::new(
                                directory,
                                file_name,
                                file_path.display().to_string(),
                            );
                            let document_meta = DocumentMeta::read_from_file(file_path)?;
                            files.push((document, document_meta));
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
                let file_name = DocumentMeta::name_from_fs(file_path.canonicalize()?);
                let document = Document::new(
                    directory,
                    file_name,
                    file_path.canonicalize()?.display().to_string(),
                );
                let document_meta = DocumentMeta::read_from_file(file_path)?;
                files.push((document, document_meta));
            }
        }
    }

    Ok(files)
}

fn get_valid_name(path: &Path) -> Result<&str, LedgeknawError> {
    let dir_name = path
        .file_name()
        .ok_or(LedgeknawError::InvalidDirectory(format!(
            "{}: unsupported directory",
            path.display()
        )))?;
    dir_name
        .to_str()
        .ok_or(LedgeknawError::InvalidDirectory(format!(
            "{dir_name:?}: not valid utf-8"
        )))
}
