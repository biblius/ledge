#[derive(Debug, Clone)]
pub struct Database {
    pub pool: sqlx::PgPool,
}
