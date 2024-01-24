use async_recursion::async_recursion;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::Encode;
use tracing::{debug, error};

use crate::state::State;
use crate::FILES_PER_THREAD;
use crate::{error::KnawledgeError, MAX_THREADS};
use std::ffi::OsStr;
use std::time::Instant;
use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
    thread::ScopedJoinHandle,
};

#[derive(Debug, Default, Encode)]
pub struct DocumentModel {
    pub id: uuid::Uuid,
    pub file_name: String,
    pub root_dir: String,
    pub content: String,
    pub title: Option<String>,
    pub reading_time: Option<i32>,
    pub tags: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Document> for DocumentModel {
    fn from(value: Document) -> Self {
        let Document {
            file_name,
            root_dir,
            content,
            created,
            title,
            reading_time,
            tags,
        } = value;

        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4(),
            file_name,
            root_dir,
            content,
            title,
            reading_time,
            tags: Some(tags.join(",")),
            created_at: created,
            updated_at: now,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Document {
    /// File name with extension
    pub file_name: String,

    /// Full path starting from the initial registered dir
    pub root_dir: String,

    /// Document markdown content
    pub content: String,

    pub created: DateTime<Utc>,

    pub title: Option<String>,

    pub reading_time: Option<i32>,

    pub tags: Vec<String>,
}

impl From<DocumentModel> for Document {
    fn from(value: DocumentModel) -> Self {
        let DocumentModel {
            file_name,
            root_dir,
            content,
            title,
            reading_time,
            tags,
            created_at,
            ..
        } = value;
        Self {
            file_name,
            root_dir,
            content,
            created: created_at,
            title,
            reading_time,
            tags: tags
                .map(|s| s.split(',').map(|s| s.to_owned()).collect::<Vec<_>>())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct DocumentMeta {}

#[async_recursion(?Send)]
pub async fn extract_dir(
    state: &State,
    path: impl AsRef<Path> + 'async_recursion,
) -> Result<(Vec<Document>, Vec<Document>), KnawledgeError> {
    let entries = fs::read_dir(&path)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    debug!("Loading {}", path.as_ref().display());

    let mut files_existing = vec![];
    let mut files_processed = vec![];

    for entry in entries.iter() {
        if entry.path().is_dir() {
            let (existing, processed) = extract_dir(state, entry.path()).await?;
            files_processed.extend(processed);
            files_existing.extend(existing);
        }
    }

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

    let root = path.as_ref().display();

    let existing = sqlx::query_as!(
        DocumentModel,
        "SELECT * FROM documents WHERE file_name = ANY($1) AND root_dir = $2",
        &file_names[..],
        root.to_string()
    )
    .fetch_all(&state.db.pool)
    .await
    .unwrap();

    for item in existing {
        if let Some(idx) = markdown_files
            .iter()
            .position(|el| &PathBuf::from(format!("{}/{}", item.root_dir, item.file_name)) == el)
        {
            debug!("Already exists: {} ", item.file_name);
            markdown_files.swap_remove(idx);
            files_existing.push(Document::from(item));
        }
    }

    let files_total = markdown_files.len();
    let mut files_remaining = files_total;

    while files_remaining > 0 {
        let mut batches: Vec<&[PathBuf]> = vec![&[]; *MAX_THREADS];

        #[allow(clippy::needless_range_loop)]
        for i in 0..*MAX_THREADS {
            let start = i * FILES_PER_THREAD;
            let mut end = (i + 1) * FILES_PER_THREAD;
            if end > files_total {
                end = files_total;
                batches[i] = &markdown_files[start..end];
                files_remaining -= end - start;
                break;
            }
            batches[i] = &markdown_files[start..end];
            files_remaining -= FILES_PER_THREAD;
        }

        type TaskWithStart<'a> = (
            ScopedJoinHandle<'a, Result<Vec<Document>, KnawledgeError>>,
            Instant,
        );

        std::thread::scope(|scope| {
            let mut tasks: Vec<TaskWithStart> = Vec::with_capacity(*MAX_THREADS);

            for batch in batches {
                if batch.is_empty() {
                    continue;
                }

                let task = scope.spawn(move || {
                    let mut files = vec![];
                    for file_path in batch {
                        let file = read_md_file(file_path)?;
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
                        files_processed.extend(processed);
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
    }

    for file in files_processed.iter() {
        let DocumentModel {
            id,
            file_name,
            root_dir,
            content,
            title,
            reading_time,
            tags,
            created_at,
            updated_at,
        } = DocumentModel::from(file.clone());
        sqlx::query!(
        "INSERT INTO documents VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT DO NOTHING",
        id,
        file_name,
        root_dir,
        content,
        title,
        reading_time,
        tags,
        created_at,
        updated_at
    )
        .execute(&state.db.pool)
        .await
        .unwrap();
    }

    Ok((files_existing, files_processed))
}

pub fn find_title_from_header(content: &str) -> Option<String> {
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

fn read_md_file(path: impl AsRef<Path>) -> Result<Document, KnawledgeError> {
    debug!("Processing {}", path.as_ref().display());

    let file_name = path
        .as_ref()
        .file_name()
        .unwrap_or(OsStr::new("__unknown"))
        .to_str()
        .unwrap_or("__unknown")
        .to_string();

    let root_dir = path
        .as_ref()
        .to_str()
        .map(|s| s.rsplit_once('/').unwrap().0)
        .unwrap_or("__unknown")
        .to_string();

    let mut document = Document {
        file_name,
        root_dir,
        created: Utc::now(),
        ..Default::default()
    };

    let content = std::fs::read_to_string(&path)?;
    collect_metadata(&mut document, content);

    Ok(document)
}

fn collect_metadata(document: &mut Document, content: String) {
    if !content.starts_with("---") {
        return document.content = content;
    }

    if content.len() < 4 {
        return document.content = content;
    }

    let Some(end_i) = &content[3..].find("---") else {
        return document.content = content[3..].to_string();
    };

    // Offset to account for the skipped ---
    let meta_str = &content[3..*end_i + 2];

    if meta_str.is_empty() {
        return document.content = content[end_i + 6..].to_string();
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
            document.tags.push(tag.trim().to_string());
            continue;
        }

        if line.starts_with("tags") {
            in_tags = true;
            continue;
        }

        if line.starts_with("title") {
            if let Some(title) = read_meta_line("title", line) {
                document.title = Some(title);
            }
        }
    }

    let content = &content[end_i + 6..];
    document.reading_time = Some(calculate_reading_time(content));

    document.content = content.to_string();
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
