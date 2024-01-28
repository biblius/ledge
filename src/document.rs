use async_recursion::async_recursion;
use sqlx::types::chrono::{DateTime, Utc};
use tracing::{debug, error, info};

use crate::db::Database;
use crate::error::KnawledgeError;
use crate::{FILES_PER_THREAD, MAX_THREADS};
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::thread::ScopedJoinHandle;
use std::time::Instant;
use std::{fmt::Debug, path::Path};

#[derive(Debug, Default, Clone)]
pub struct Document {
    pub id: uuid::Uuid,

    /// File name with extension
    pub file_name: String,

    /// Full path starting from the initial registered dir
    pub directory: uuid::Uuid,

    /// Document markdown content
    pub content: String,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub title: Option<String>,

    pub reading_time: Option<i32>,

    pub tags: Option<String>,
}

impl Document {
    pub fn read_md_file(
        directory: uuid::Uuid,
        path: impl AsRef<Path>,
    ) -> Result<Self, KnawledgeError> {
        debug!("Processing {}", path.as_ref().display());

        let file_name = path
            .as_ref()
            .file_name()
            .unwrap_or(OsStr::new("__unknown"))
            .to_str()
            .unwrap_or("__unknown")
            .to_string();

        let mut document = Self {
            id: uuid::Uuid::new_v4(),
            file_name,
            directory,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            ..Default::default()
        };

        let content = std::fs::read_to_string(&path)?;

        document.collect_metadata(content);

        Ok(document)
    }

    fn collect_metadata(&mut self, content: String) {
        if !content.starts_with("---") {
            return self.content = content;
        }

        if content.len() < 4 {
            return self.content = content;
        }

        let Some(end_i) = &content[3..].find("---") else {
            return self.content = content[3..].to_string();
        };

        // Offset to account for the skipped ---
        let meta_str = &content[3..*end_i + 2];

        if meta_str.is_empty() {
            return self.content = content[end_i + 6..].to_string();
        }

        let mut in_tags = false;

        for line in meta_str.lines() {
            let line = line.trim();

            if in_tags && !line.starts_with('-') {
                in_tags = false;
            }

            if in_tags {
                let Some((_, tag)) = line.split_once('-') else {
                    continue;
                };
                self.tags.as_mut().unwrap().push_str(tag.trim());
                self.tags.as_mut().unwrap().push(',');
                continue;
            }

            if line.starts_with("tags") {
                self.tags = Some(String::new());
                in_tags = true;
                continue;
            }

            if line.starts_with("title") {
                if let Some(title) = read_meta_line("title", line) {
                    self.title = Some(title);
                }
            }
        }

        let content = &content[end_i + 6..];
        self.reading_time = Some(calculate_reading_time(content));

        self.content = content.to_string();

        if self.title.is_none() {
            self.title = find_title_from_h1(content);
        }
    }
}

#[derive(Debug, Default)]
pub struct Directory {
    pub id: uuid::Uuid,
    pub name: String,
    pub parent: Option<uuid::Uuid>,
    pub path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_recursion(?Send)]
pub async fn process_directory(
    db: &Database,
    path: impl AsRef<Path> + 'async_recursion,
    parent: Option<uuid::Uuid>,
) -> Result<(), KnawledgeError> {
    let entries = fs::read_dir(&path)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let full_path = path.as_ref().canonicalize()?.display().to_string();
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
            let parent = db.get_dir_by_name_and_parent(dir_name, parent_id).await?;

            match parent {
                Some(dir) => dir,
                None => {
                    db.insert_directory(&full_path, dir_name, Some(parent_id))
                        .await?
                }
            }
        }
        None => {
            let root = db.get_root_dir_by_name(dir_name).await?;
            match root {
                Some(dir) => dir,
                None => db.insert_directory(&full_path, dir_name, None).await?,
            }
        }
    };

    for entry in entries.iter() {
        if entry.path().is_dir() {
            process_directory(db, entry.path(), Some(directory_entry.id)).await?;
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

    let existing = db.list_existing(directory_entry.id, &file_names).await?;

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
        db.insert_document(file).await?;
    }

    info!("Existing files: {amt_files_existing} Processed files: {amt_files_processed}",);

    Ok(())
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

pub fn find_title_from_h1(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            let Some((_, title)) = line.split_once('#') else {
                continue;
            };

            return Some(title.trim().to_string());
        }
    }

    None
}

fn calculate_reading_time(content: &str) -> i32 {
    let words = content.split(' ').collect::<Vec<_>>().len();
    ((words / 200) as f32 * 0.60) as i32
}

fn read_meta_line(tag: &str, input: &str) -> Option<String> {
    input
        .split_once(&format!("{tag}:"))
        .map(|(_, val)| val.trim().to_string())
}
