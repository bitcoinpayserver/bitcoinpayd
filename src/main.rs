use std::process::exit;
use crate::server::start_server;

mod core;
mod server;
mod services;
mod storage;

pub mod payment {
    tonic::include_proto!("bitcoinpayserver.v1.services.payment");
}

pub mod wallet {
    tonic::include_proto!("bitcoinpayserver.v1.services.wallet");
}

#[tokio::main]
async fn main() {
    match start_server().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Failed to start server: {}", e);
            exit(1);
        }
    };
}
