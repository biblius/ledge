use sqlx::PgPool;

pub(super) async fn create_pool(url: &str) -> PgPool {
    sqlx::postgres::PgPool::connect(url)
        .await
        .expect("error while connecting to db")
}

pub(super) async fn migrate(pool: &PgPool) {
    sqlx::migrate!()
        .run(pool)
        .await
        .expect("error in migrations")
}
