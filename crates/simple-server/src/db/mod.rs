pub(crate) mod actions;
pub(crate) mod events;
pub(crate) mod routines;

pub(crate) use sqlx::PgPool;

use anyhow::Context as _;

pub(crate) async fn connect(url: &str) -> anyhow::Result<PgPool> {
    tracing::info!("connecting to database");
    let pool = PgPool::connect(url).await.context("connect to postgres")?;
    tracing::info!("connected");
    Ok(pool)
}

pub(crate) async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    tracing::info!("running migrations");
    sqlx::migrate!().run(pool).await.context("run migrations")?;
    tracing::info!("migrations complete");
    Ok(())
}
