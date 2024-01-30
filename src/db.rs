use crate::{
    document::{process_directory, Directory, Document},
    error::KnawledgeError,
};

#[derive(Debug, Clone)]
pub struct Database {
    pool: sqlx::PgPool,
}

#[derive(Debug)]
pub struct DirectoryEntry {
    pub id: uuid::Uuid,
    pub name: String,
    pub parent: Option<uuid::Uuid>,
    pub r#type: String,
    pub title: Option<String>,
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

    pub async fn get_index(&self) -> Result<Option<Document>, KnawledgeError> {
        sqlx::query_as!(
            Document,
            "SELECT * FROM documents WHERE file_name = 'index.md'"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn get_document(&self, id: uuid::Uuid) -> Result<Option<Document>, KnawledgeError> {
        sqlx::query_as!(Document, "SELECT * FROM documents WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await
            .map_err(KnawledgeError::from)
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

    pub async fn nuke_dir(&self, path: &str) -> Result<(), KnawledgeError> {
        sqlx::query_as!(Directory, "DELETE FROM directories WHERE path = $1", path)
            .fetch_optional(&self.pool)
            .await
            .map(|_| ())
            .map_err(KnawledgeError::from)
    }

    pub async fn list_existing(
        &self,
        directory: uuid::Uuid,
        file_names: &[String],
    ) -> Result<Vec<Document>, KnawledgeError> {
        sqlx::query_as!(
            Document,
            "SELECT * FROM documents WHERE file_name = ANY($1) AND directory = $2",
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
                (SELECT dir.id, dir.parent, dir.name, 'd' AS type, NULL AS title 
                FROM directories dir WHERE dir.parent IS NULL),
            docs AS
                (SELECT d.id, d.directory AS parent, d.file_name AS name, 'f' AS type, d.title
                FROM documents d INNER JOIN roots ON d.directory = roots.id),
            dirs AS
                (SELECT d.id, d.parent, d.name, 'd' AS type, NULL as title 
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
            SELECT 
                doc.id, dir.id AS parent, doc.file_name AS name, 'f' AS type, doc.title 
                FROM documents doc 
            INNER JOIN directories dir 
                ON doc.directory = dir.id 
                AND dir.id = $1 
            UNION 
            SELECT id, parent, name, 'd' AS type, NULL AS title 
            FROM directories WHERE parent = $1
            "#,
            id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(KnawledgeError::from)
    }

    pub async fn insert_document(&self, document: Document) -> Result<(), KnawledgeError> {
        let Document {
            id,
            file_name,
            directory,
            content,
            title,
            reading_time,
            tags,
            created_at,
            updated_at,
        } = document;

        sqlx::query!(
            "INSERT INTO documents VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT DO NOTHING",
            id,
            file_name,
            directory,
            content,
            title,
            reading_time,
            tags,
            created_at,
            updated_at
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
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

    pub async fn insert_directory(
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

    pub async fn remove_file(&self, dir: &str, file: &str) -> Result<(), KnawledgeError> {
        sqlx::query!(
            "DELETE FROM documents WHERE id = (SELECT id FROM directories WHERE path = $1) AND file_name = $2",
            dir,
            file
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_root(&self, old: &str, new: &str) -> Result<(), KnawledgeError> {
        sqlx::query!("DELETE FROM directories WHERE path = $1", old)
            .execute(&self.pool)
            .await?;
        process_directory(self, new, None).await?;
        Ok(())
    }
}
