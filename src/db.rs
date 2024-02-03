use crate::{
    document::{Directory, Document, DocumentMeta},
    error::KnawledgeError,
};
use models::DirectoryEntry;

pub mod models;

#[derive(Debug, Clone)]
pub struct Database {
    pool: sqlx::PgPool,
}

impl Database {
    pub async fn new(url: &str) -> Self {
        let pool = sqlx::postgres::PgPool::connect(url)
            .await
            .expect("error while connecting to db");

        Self { pool }
    }

    pub async fn migrate(&self) {
        sqlx::migrate!()
            .run(&self.pool)
            .await
            .expect("error in migrations")
    }

    pub async fn insert_dir(
        &self,
        path: &str,
        name: &str,
        parent: Option<uuid::Uuid>,
    ) -> Result<Directory, KnawledgeError> {
        sqlx::query_as!(
            Directory,
            "INSERT INTO directories(path, name, parent) VALUES($1, $2, $3) RETURNING *",
            path,
            name,
            parent
        )
        .fetch_one(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn insert_doc(
        &self,
        document: &Document,
        meta: &DocumentMeta,
    ) -> Result<(), KnawledgeError> {
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
        .map_err(KnawledgeError::from)
    }

    pub async fn get_index_path(&self) -> Result<Option<String>, KnawledgeError> {
        Ok(
            sqlx::query!("SELECT path FROM documents WHERE file_name = 'index.md' LIMIT 1")
                .fetch_optional(&self.pool)
                .await?
                .map(|el| el.path),
        )
    }

    pub async fn get_doc_path(&self, id: uuid::Uuid) -> Result<Option<String>, KnawledgeError> {
        Ok(sqlx::query!("SELECT path FROM documents WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await?
            .map(|el| el.path))
    }

    pub async fn get_doc_path_by_custom_id(
        &self,
        custom_id: &str,
    ) -> Result<Option<String>, KnawledgeError> {
        Ok(
            sqlx::query!("SELECT path FROM documents WHERE custom_id = $1", custom_id)
                .fetch_optional(&self.pool)
                .await?
                .map(|el| el.path),
        )
    }

    pub async fn list_root_paths(&self) -> Result<Vec<String>, KnawledgeError> {
        Ok(
            sqlx::query!("SELECT path FROM directories WHERE parent IS NULL",)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|el| el.path)
                .collect(),
        )
    }

    pub async fn get_dir_by_path(&self, path: &str) -> Result<Option<Directory>, KnawledgeError> {
        sqlx::query_as!(Directory, "SELECT * FROM directories WHERE path = $1", path)
            .fetch_optional(&self.pool)
            .await
            .map_err(KnawledgeError::from)
    }

    pub async fn get_root_by_path(&self, path: &str) -> Result<Option<Directory>, KnawledgeError> {
        sqlx::query_as!(
            Directory,
            "SELECT * FROM directories WHERE path = $1 AND parent IS NULL",
            path
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn list_document_in_dir(
        &self,
        directory: uuid::Uuid,
        file_names: &[String],
    ) -> Result<Vec<Document>, KnawledgeError> {
        sqlx::query_as!(
            Document,
            "SELECT file_name, directory, path 
             FROM documents WHERE file_name = ANY($1) AND directory = $2",
            file_names,
            directory
        )
        .fetch_all(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn list_roots_with_entries(&self) -> Result<Vec<DirectoryEntry>, KnawledgeError> {
        sqlx::query_as_unchecked!(
            DirectoryEntry,
            r#"
                WITH
                roots AS
                    (SELECT dir.id, dir.parent, dir.name, 'd' AS type, NULL AS title, NULL AS custom_id
                    FROM directories dir WHERE dir.parent IS NULL),
                docs AS
                    (SELECT d.id, d.directory AS parent, d.file_name AS name, 'f' AS type, d.title, d.custom_id
                    FROM documents d INNER JOIN roots ON d.directory = roots.id),
                dirs AS
                    (SELECT d.id, d.parent, d.name, 'd' AS type, NULL as title, NULL AS custom_id
                    FROM directories d INNER JOIN roots ON d.parent = roots.id)
                SELECT * FROM docs
                UNION
                SELECT * FROM dirs
                UNION
                SELECT * FROM roots
                ORDER BY parent DESC
        "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn list_entries(
        &self,
        id: uuid::Uuid,
    ) -> Result<Vec<DirectoryEntry>, KnawledgeError> {
        sqlx::query_as_unchecked!(
            DirectoryEntry,
            r#"
                SELECT doc.id, dir.id AS parent, doc.file_name AS name, 'f' AS type, doc.title, doc.custom_id
                FROM documents doc
                INNER JOIN directories dir
                ON doc.directory = dir.id AND dir.id = $1
                UNION
                SELECT id, parent, name, 'd' AS type, NULL AS title, NULL AS custom_id
                FROM directories WHERE parent = $1
        "#,
            id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn get_dir_by_name_and_parent(
        &self,
        name: &str,
        id: uuid::Uuid,
    ) -> Result<Option<Directory>, KnawledgeError> {
        sqlx::query_as!(
            Directory,
            "SELECT * FROM directories WHERE name=$1 AND parent=$2",
            name,
            id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn get_root_dir_by_name(
        &self,
        name: &str,
    ) -> Result<Option<Directory>, KnawledgeError> {
        sqlx::query_as!(
            Directory,
            "SELECT * FROM directories WHERE name=$1 AND parent IS NULL",
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn update_doc_by_path(
        &self,
        path: &str,
        meta: &DocumentMeta,
    ) -> Result<(), KnawledgeError> {
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

    pub async fn remove_dir(&self, path: &str) -> Result<(), KnawledgeError> {
        sqlx::query_as!(Directory, "DELETE FROM directories WHERE path = $1", path)
            .fetch_optional(&self.pool)
            .await
            .map(|_| ())
            .map_err(KnawledgeError::from)
    }

    pub async fn remove_doc(&self, path: &str) -> Result<(), KnawledgeError> {
        sqlx::query!("DELETE FROM documents WHERE path = $1", path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
