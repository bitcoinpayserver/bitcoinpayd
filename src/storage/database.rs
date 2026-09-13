use std::error::Error;
use crate::server::Config;
use deadpool_postgres::{Manager, Pool};
use std::str::FromStr;
use tokio_postgres::types::ToSql;
use tokio_postgres::{NoTls, Row};

pub struct PostgresDb {
    pool: Pool
}

impl PostgresDb {
    const MAX_CONNECTIONS: usize = 18;

    pub fn new(config: &Config) -> Self {
        let connection_string = format!(
            "host={} user={} password={} dbname={}",
            config.db_host,
            config.db_username,
            config.db_password,
            config.db_name
        );

        let postgres_config = tokio_postgres::Config::from_str(connection_string.as_str()).unwrap();

        let pool_manager = Manager::new(postgres_config, NoTls);

        let pool = Pool::builder(pool_manager)
            .max_size(Self::MAX_CONNECTIONS).build().unwrap();

        Self {
            pool
        }
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

    pub async fn query_one(&self, query: &str, args: &[&(dyn ToSql + Sync)]) -> Result<Row, Box<dyn Error>> {
        let connection = self.pool.get().await?;
        let statement = connection.prepare(query).await?;
        let row = connection.query_one(&statement, args).await?;
        Ok(row)
    }
}