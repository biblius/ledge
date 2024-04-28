use super::{models::Document, Directory, DocumentMeta};
use crate::{document::models::DirectoryEntry, error::LedgeknawError};
use sqlx::PgPool;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct DocumentDb {
    pool: sqlx::PgPool,
}

impl DocumentDb {
    pub async fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Retrieve all paths from the documents table
    pub async fn get_all_file_paths(&self) -> Result<Vec<String>, LedgeknawError> {
        Ok(
            sqlx::query!("SELECT path FROM documents UNION SELECT path FROM directories",)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .filter_map(|el| el.path)
                .collect(),
        )
    }

    /// Insert a child directory entry to the DB
    pub async fn insert_dir(
        &self,
        path: &str,
        name: &str,
        parent: uuid::Uuid,
    ) -> Result<Directory, LedgeknawError> {
        sqlx::query_as!(
            Directory,
            "INSERT INTO directories(path, name, parent) VALUES($1, $2, $3) RETURNING *",
            path,
            name,
            parent
        )
        .fetch_one(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    /// Insert a root directory entry to the DB
    pub async fn insert_root_dir(
        &self,
        path: &str,
        name: &str,
        alias: &str,
    ) -> Result<Directory, LedgeknawError> {
        sqlx::query_as!(
            Directory,
            "INSERT INTO directories(path, name, alias) VALUES($1, $2, $3) RETURNING *",
            path,
            name,
            alias
        )
        .fetch_one(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    pub async fn insert_doc(
        &self,
        document: &Document,
        meta: &DocumentMeta,
    ) -> Result<(), LedgeknawError> {
        let Document {
            file_name,
            directory,
            path,
        } = document;

        let DocumentMeta {
            custom_id,
            title,
            tags,
            ..
        } = meta;

        sqlx::query!(
            "INSERT INTO documents(file_name, directory, path, custom_id, title, tags) VALUES($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
            file_name,
            directory,
            path,
            custom_id.as_ref(),
            title.as_ref(),
            tags.as_ref().map(|el|el.join(","))
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(LedgeknawError::from)
    }

    pub async fn get_index_id_path(&self) -> Result<Option<(uuid::Uuid, String)>, LedgeknawError> {
        Ok(
            sqlx::query!("SELECT id, path FROM documents WHERE file_name = 'index.md' LIMIT 1")
                .fetch_optional(&self.pool)
                .await?
                .map(|el| (el.id, el.path)),
        )
    }

    pub async fn get_doc_path(&self, id: uuid::Uuid) -> Result<Option<String>, LedgeknawError> {
        Ok(sqlx::query!("SELECT path FROM documents WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await?
            .map(|el| el.path))
    }

    pub async fn get_doc_id_path_by_custom_id(
        &self,
        custom_id: &str,
    ) -> Result<Option<(uuid::Uuid, String)>, LedgeknawError> {
        Ok(sqlx::query!(
            "SELECT id, path FROM documents WHERE custom_id = $1",
            custom_id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|el| (el.id, el.path)))
    }

    pub async fn list_root_paths(&self) -> Result<Vec<String>, LedgeknawError> {
        Ok(
            sqlx::query!("SELECT path FROM directories WHERE parent IS NULL",)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|el| el.path)
                .collect(),
        )
    }

    pub async fn get_dir_by_path(&self, path: &str) -> Result<Option<Directory>, LedgeknawError> {
        sqlx::query_as!(Directory, "SELECT * FROM directories WHERE path = $1", path)
            .fetch_optional(&self.pool)
            .await
            .map_err(LedgeknawError::from)
    }

    pub async fn get_root_by_path(&self, path: &str) -> Result<Option<Directory>, LedgeknawError> {
        sqlx::query_as!(
            Directory,
            "SELECT * FROM directories WHERE path = $1 AND parent IS NULL",
            path
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    pub async fn list_document_in_dir(
        &self,
        directory: uuid::Uuid,
        file_names: &[String],
    ) -> Result<Vec<Document>, LedgeknawError> {
        sqlx::query_as!(
            Document,
            "SELECT file_name, directory, path 
             FROM documents WHERE file_name = ANY($1) AND directory = $2",
            file_names,
            directory
        )
        .fetch_all(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    pub async fn list_roots(&self) -> Result<Vec<DirectoryEntry>, LedgeknawError> {
        sqlx::query_as_unchecked!(
            DirectoryEntry,
            r#"
                SELECT id, parent, name, 'd' AS type, alias AS title, NULL AS custom_id
                FROM directories WHERE parent IS NULL
        "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    pub async fn list_entries(
        &self,
        id: uuid::Uuid,
    ) -> Result<Vec<DirectoryEntry>, LedgeknawError> {
        sqlx::query_as_unchecked!(
            DirectoryEntry,
            r#"
                SELECT doc.id, dir.id AS parent, doc.file_name AS name, 'f' AS type, doc.title, doc.custom_id
                FROM documents doc
                INNER JOIN directories dir
                ON doc.directory = dir.id AND dir.id = $1
                UNION
                SELECT id, parent, name, 'd' AS type, alias AS title, NULL AS custom_id
                FROM directories WHERE parent = $1
        "#,
            id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    pub async fn get_dir_by_name_and_parent(
        &self,
        name: &str,
        id: uuid::Uuid,
    ) -> Result<Option<Directory>, LedgeknawError> {
        sqlx::query_as!(
            Directory,
            "SELECT * FROM directories WHERE name=$1 AND parent=$2",
            name,
            id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    pub async fn get_root_dir_by_name(
        &self,
        name: &str,
    ) -> Result<Option<Directory>, LedgeknawError> {
        sqlx::query_as!(
            Directory,
            "SELECT * FROM directories WHERE name=$1 AND parent IS NULL",
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(LedgeknawError::from)
    }

    pub async fn update_doc_by_path(
        &self,
        path: &str,
        meta: &DocumentMeta,
    ) -> Result<(), LedgeknawError> {
        let DocumentMeta {
            custom_id,
            title,
            reading_time,
            tags,
        } = meta;
        sqlx::query!(
            r#"
            UPDATE documents SET 
            custom_id = $1,
            title = $2,
            reading_time = $3,
            tags = $4
            WHERE path = $5 
        "#,
            custom_id.as_ref(),
            title.as_ref(),
            reading_time.as_ref(),
            tags.as_ref().map(|t| t.join(",")),
            path
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_dir(&self, path: &str) -> Result<(), LedgeknawError> {
        sqlx::query_as!(Directory, "DELETE FROM directories WHERE path = $1", path)
            .fetch_optional(&self.pool)
            .await
            .map(|_| ())
            .map_err(LedgeknawError::from)
    }

    pub async fn remove_file_by_path(&self, path: &str) -> Result<(), LedgeknawError> {
        sqlx::query!("DELETE FROM documents WHERE path = $1", path)
            .execute(&self.pool)
            .await?;

        sqlx::query!("DELETE FROM directories WHERE path = $1", path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete any root directories from the DB not in `paths`.
    pub async fn trim_roots(&self, paths: &[String]) -> Result<(), LedgeknawError> {
        // https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-do-a-select--where-foo-in--query
        let count = sqlx::query!(
            "
            DELETE FROM directories
            WHERE path != ALL($1) AND parent IS NULL",
            paths
        )
        .execute(&self.pool)
        .await?;
        debug!("Trimmed {} directories", count.rows_affected());
        Ok(())
    }
}
