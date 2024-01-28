use sqlx::types::chrono::{DateTime, Utc};
use tracing::debug;

use crate::error::KnawledgeError;
use std::ffi::OsStr;
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
    }
}

#[derive(Debug, Default)]
pub struct Directory {
    pub id: uuid::Uuid,
    pub name: String,
    pub parent: Option<uuid::Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

fn calculate_reading_time(content: &str) -> i32 {
    let words = content.split(' ').collect::<Vec<_>>().len();
    ((words / 200) as f32 * 0.60) as i32
}

fn read_meta_line(tag: &str, input: &str) -> Option<String> {
    input
        .split_once(&format!("{tag}:"))
        .map(|(_, val)| val.trim().to_string())
}
