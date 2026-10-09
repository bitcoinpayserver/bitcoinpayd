use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[derive(Debug)]
pub struct PaymentInfoQuery {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PaymentStatus {
    Pending,
    AwaitingConfirmation { tx_id: String, v_out: i32, amount: Decimal },
    Confirmed,
    Expired,
}

impl PaymentStatus {
    pub fn to_string(&self) -> String {
        match self {
            PaymentStatus::Pending { .. } => stringify!(Pending).to_string(),
            PaymentStatus::AwaitingConfirmation { .. } => stringify!(AwaitingConfirmation).to_string(),
            PaymentStatus::Confirmed => stringify!(Confirmed).to_string(),
            PaymentStatus::Expired => stringify!(Expired).to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Clone)]
#[derive(Debug)]
pub struct Utxo {
    pub tx_id: String,
    pub v_out: i32,
    pub amount: Decimal,
    pub address: Option<String>,
    pub webhook_url: String,
    pub block_height: Option<i64>,
    pub confirmations: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct AddressInfo {
    pub address: String,
    pub webhook_url: String,
}

pub mod grpc {
    use crate::core::models::http;
    use crate::payment::{PaymentRequest, PaymentResponse};
    use bitcoin::{Address, Network};
    use chrono::{DateTime, Utc};
    use rust_decimal::Decimal;
    use serde_json::from_value;
    use std::collections::HashMap;
    use std::str::FromStr;
    use tokio_postgres::Row;
    use uuid::Uuid;

    impl PaymentRequest {
        pub fn from_http(request: http::PaymentRequest) -> Self {
            Self {
                owner_id: request.owner_id,
                amount: request.amount,
                network: request.network,
                currency_code: request.currency_code,
                metadata: request.metadata,
            }
        }
    }

    impl PaymentResponse {
        pub fn from_db(row: Row) -> PaymentResponse {
            let id:
                Uuid = row.get("id");
            let owner_id: Uuid = row.get("owner_id");
            let amount: Decimal = row.get("amount");
            let currency_code = row.get("currency_code");
            let network = row.get("network");
            let address = Address::from_str(row.get("address")).unwrap()
                .require_network(Network::from_str(network).unwrap()).unwrap().to_string();
            let index: i32 = row.get("child_key_index");
            let payment_status = row.get("status");
            let metadata_json: Option<serde_json::Value> = row.get("metadata");
            let metadata: HashMap<String, String> = match metadata_json {
                Some(metadata) => from_value(metadata).unwrap(),
                None => HashMap::new(),
            };
            let created_at: DateTime<Utc> = row.get("created_at");

            PaymentResponse {
                id: id.to_string(),
                owner_id: owner_id.to_string(),
                amount: amount.to_string(),
                currency_code,
                address,
                child_key_index: index,
                network: network.to_string(),
                payment_status,
                metadata,
                created_at: created_at.timestamp(),
            }
        }
    }
}

pub mod http {
    use crate::payment;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize)]
    pub struct PaymentRequest {
        pub owner_id: String,
        pub amount: String,
        pub network: String,
        pub currency_code: String,
        pub metadata: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct PaymentResponse {
        pub id: String,
        pub owner_id: String,
        pub amount: String,
        pub currency_code: String,
        pub address: String,
        pub child_key_index: i32,
        pub network: String,
        pub payment_status: String,
        pub metadata: HashMap<String, String>,
    }

    impl PaymentResponse {
        pub fn from_grpc(response: payment::PaymentResponse) -> Self {
            Self {
                id: response.id,
                owner_id: response.owner_id,
                amount: response.amount,
                currency_code: response.currency_code,
                address: response.address,
                child_key_index: response.child_key_index,
                network: response.network,
                payment_status: response.payment_status,
                metadata: response.metadata,
            }
        }
    }
}