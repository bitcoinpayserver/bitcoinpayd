use std::env;
use std::sync::Arc;
use axum::serve;
use tokio::net::TcpListener;
use tonic::service::Routes;
use crate::payment::payment_service_server::PaymentServiceServer;
use crate::services::payment::BPaymentService;
use crate::services::wallet::BWalletService;
use crate::storage::database::PostgresDb;
use crate::wallet::wallet_service_server::WalletServiceServer;

pub struct Config {
    pub server_address: String,
    pub db_host: String,
    pub db_port: i32,
    pub db_username: String,
    pub db_name: String,
    pub db_password: String,
}

impl Config {
    const HOST_KEY: &'static str = "HOST";
    const PORT_KEY: &'static str = "PORT";
    const DB_HOST_KEY: &'static str = "DB_HOST";
    const DB_PORT_KEY: &'static str = "DB_PORT";
    const DB_USER_KEY: &'static str = "DB_USER";
    const DB_NAME: &'static str = "DB_NAME";
    const DB_PASSWORD: &'static str = "DB_PASSWORD";

    pub fn new() -> Self {
        let host = env::var(Self::HOST_KEY).unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var(Self::PORT_KEY).unwrap_or_else(|_| "50051".to_string());
        let db_host = env::var(Self::DB_HOST_KEY).unwrap_or_else(|_| "localhost".to_string());
        let db_port = env::var(Self::DB_PORT_KEY).unwrap_or_else(|_| "5432".to_string()).parse::<i32>().unwrap();
        let db_user = env::var(Self::DB_USER_KEY).unwrap_or_else(|_| "postgres".to_string());
        let db_name = env::var(Self::DB_NAME).unwrap_or_else(|_| "postgres".to_string());
        let db_password = env::var(Self::DB_PASSWORD).unwrap_or_else(|_| "password".to_string());

        let server_address = format!("{}:{}", host, port);
        Self {
            server_address,
            db_host,
            db_port,
            db_username: db_user,
            db_name,
            db_password
        }
    }
}

pub async fn start_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new();

    let postgres_db = Arc::new(PostgresDb::new(Some(&config)).await?);

    postgres_db.create_tables(
        &[
            BWalletService::CREATE_TABLE,
            BWalletService::CREATE_CHILD_KEYS_TABLE,
            BPaymentService::CREATE_TABLE,
        ]
    ).await?;

    let wallet_service = BWalletService::new(postgres_db.clone());
    let wallet_server = WalletServiceServer::new(wallet_service);
    let payment_service = BPaymentService::new(postgres_db);
    let payment_server = PaymentServiceServer::new(payment_service);

    let grpc_router = Routes::new(wallet_server)
        .add_service(payment_server)
        .prepare().into_axum_router();

    let listener = TcpListener::bind(config.server_address).await?;

    serve(listener, grpc_router).await?;

    Ok(())
}