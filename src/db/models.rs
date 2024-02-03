/// Used for querying both files and directories.
/// The type is either 'f' or 'd'.
/// Only directories have the parent field.
#[derive(Debug)]
pub struct DirectoryEntry {
    pub id: uuid::Uuid,
    pub name: String,
    pub parent: Option<uuid::Uuid>,
    pub r#type: String,
    pub title: Option<String>,
    pub custom_id: Option<String>,
}
