/// Database model
#[derive(Debug, Default)]
pub struct Document {
    /// File name with extension
    pub file_name: String,
    /// Directory ID that contains this file
    pub directory: uuid::Uuid,
    /// Canonicalised path
    pub path: String,
}

impl Document {
    pub fn new(directory: uuid::Uuid, name: String, path: String) -> Self {
        let document = Self {
            file_name: name,
            directory,
            path,
        };

        document
    }
}

/// Used for querying both files and directories.
/// The type is either 'f' or 'd'.
/// Only directories have the parent field.
#[derive(Debug)]
pub struct DirectoryEntry {
    pub id: uuid::Uuid,
    pub name: String,
    pub parent: Option<uuid::Uuid>,
    pub r#type: String,

    // Files only
    pub title: Option<String>,
    pub custom_id: Option<String>,
}
