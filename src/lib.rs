pub mod core;

pub mod server;
pub mod services;

pub mod storage;

pub mod payment {
    tonic::include_proto!("bitcoinpayserver.v1.services.payment");
}

pub mod wallet {
    tonic::include_proto!("bitcoinpayserver.v1.services.wallet");
}