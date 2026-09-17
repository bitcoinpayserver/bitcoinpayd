use std::error::Error;
use crate::server::Config;
use deadpool_postgres::{Manager, Pool, Transaction};
use std::str::FromStr;
use postgresql_embedded::{PostgreSQL, Settings};
use tokio_postgres::types::ToSql;
use tokio_postgres::{NoTls, Row};
use tonic::Status;

pub struct PostgresDb {
    pool: Pool,
    db_process: Option<PostgreSQL>
}

impl PostgresDb {
    const MAX_CONNECTIONS: usize = 18;

    pub async fn new(config: Option<&Config>) -> Result<Self, Status> {
        let (connection_string , db) = if let Some(config) = config {
            (format!(
                "host={} user={} password={} dbname={}",
                config.db_host,
                config.db_username,
                config.db_password,
                config.db_name
            ),
             None
            )
        } else { Self::test_db().await? };


        let postgres_config = tokio_postgres::Config::from_str(connection_string.as_str()).map_err(|_|
            Status::invalid_argument("Invalid postgres database configuration")
        )?;

        let pool_manager = Manager::new(postgres_config, NoTls);

        let pool = Pool::builder(pool_manager)
            .max_size(Self::MAX_CONNECTIONS).build().unwrap();

        Ok(Self { pool, db_process: db })
    }

    pub async fn create_tables(&self, commands: &[&str]) -> Result<(), Box<dyn Error>> {
        let mut connection = self.pool.get().await.unwrap();
        let transaction = connection.transaction().await?;

        for command in commands {
            transaction.batch_execute(command).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub fn get_pool(&self) -> &Pool {
        &self.pool
    }
    
    // Normal queries
    pub async fn query_one(&self, query: &str, args: &[&(dyn ToSql + Sync)]) -> Result<Row, Box<dyn Error>> {
        let connection = self.pool.get().await?;
        let statement = connection.prepare(query).await?;
        let row = connection.query_one(&statement, args).await?;
        Ok(row)
    }

    // Transactional queries
    pub async fn tx_query_one(tx: &Transaction<'_>, query: &str, args: &[&(dyn ToSql + Sync)]) -> Result<Row, Box<dyn Error>> {
        let statement = tx.prepare(query).await?;
        let row = tx.query_one(&statement, args).await?;
        Ok(row)
    }

    async fn test_db() -> Result<(String, Option<PostgreSQL>), Status> {
        let settings = Settings::new();
        let mut postgres = PostgreSQL::new(settings);
        postgres.setup().await.map_err(|_|
            Status::internal("failed to setup test database")
        )?;
        postgres.start().await.map_err(|_|
            Status::internal("failed to start test database")
        )?;

        let db_name = "bitcoinpay_test_db";
        postgres.create_database(db_name).await.map_err(|_|
            Status::internal("failed to create test database")
        )?;
        let conn_s = postgres.settings().url(db_name);
        Ok((conn_s, Some(postgres)))
    }
}