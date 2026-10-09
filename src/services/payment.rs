use crate::core::models::{AddressInfo, PaymentStatus};
use crate::core::wallet::HDWallet;
use crate::payment::payment_service_server::PaymentService;
use crate::payment::GetPaymentRequest;
use crate::payment::{PaymentRequest, PaymentResponse, PaymentStatusRequest, PaymentStatusResponse};
use crate::server::Config;
use crate::services::wallet::BWalletService;
use crate::storage::database::PostgresDb;
use bitcoin::Network;
use chrono::{DateTime, Utc};
use deadpool_postgres::Transaction;
use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::{from_value, to_value, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::spawn;
use tonic::{async_trait, Request, Response, Status};
use uuid::Uuid;

pub struct BPaymentService {
    config: Config,
    http_client: Client,
    db: Arc<PostgresDb>,
}

impl BPaymentService {
    pub const CREATE_TABLE: &'static str = "CREATE TABLE IF NOT EXISTS bpayments (
        id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
        owner_id UUID NOT NULL REFERENCES bwallets(id),
        amount NUMERIC(24, 8) NOT NULL,
        tx_id VARCHAR(64),
        v_out INT,
        currency_code VARCHAR(4) NOT NULL,
        address VARCHAR(164) NOT NULL UNIQUE,
        child_key_index INT NOT NULL CHECK(child_key_index >= 0 AND child_key_index <= 2147483647),
        network VARCHAR(24) NOT NULL,
        status VARCHAR(24) NOT NULL,
        metadata JSONB,
        created_at TIMESTAMPTZ NOT NULL DEFAULT now()
    )";

    pub const INSERT: &'static str = "INSERT INTO bpayments (
        owner_id,
        amount,
        currency_code,
        address,
        child_key_index,
        network,
        status,
        metadata
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *";

    pub const GET: &'static str = "SELECT * FROM bpayments WHERE id = $1";

    pub const UPDATE_PAYMENT_STATUS: &'static str = "UPDATE bpayments SET tx_id = $2, v_out = $3, status = $4 WHERE id = $1";
    
    pub fn new(config: &Config, http_client: Client, db: Arc<PostgresDb>) -> Self {
        Self {
            config: config.clone(),
            http_client,
            db
        }
    }

    async fn save_payment(
        tx: &Transaction<'_>,
        owner_id: Uuid,
        amount: String,
        currency_code: String,
        address: String,
        child_key_index: u32,
        network: Network,
        status: String,
        metadata: HashMap<String, String>,
    ) -> Result<PaymentResponse, Status> {
        let statement = tx.prepare(Self::INSERT).await.map_err(|_|
            Status::internal("failed to prepare save payment statement")
        )?;

        let amount = Decimal::from_str(amount.as_str()).map_err(|_|
            Status::invalid_argument("amount argument not a decimal value")
        )?;

        if amount.is_zero() || amount.is_sign_negative() {
            return Err(Status::invalid_argument("invalid payment amount"));
        }

        let meta = to_value(&metadata).map_err(|_|
            Status::invalid_argument("invalid metadata")
        )?;

        let row = tx.query_one(
            &statement,
            &[&owner_id, &amount, &currency_code, &address, &(child_key_index as i32), &network.to_string(), &status, &meta],
        ).await.map_err(|_|
            Status::internal("could not save payment into database")
        )?;

        let json: Option<Value> = row.get("metadata");

        let id: Uuid = row.get("id");
        let owner_id: Uuid = row.get("owner_id");
        let amount: Decimal = row.get("amount");
        let child_key_index: i32 = row.get("child_key_index");
        let payment_meta = match json {
            Some(meta) =>
                from_value(meta).map_err(|_| Status::invalid_argument("invalid metadata"))?,
            None => HashMap::new()
        };
        let created_at: DateTime<Utc> = row.get("created_at");

        let payment_response = PaymentResponse {
            id: id.to_string(),
            owner_id: owner_id.to_string(),
            amount: amount.to_string(),
            currency_code: row.get("currency_code"),
            address: row.get("address"),
            child_key_index,
            network: row.get("network"),
            payment_status: row.get("status"),
            metadata: payment_meta,
            created_at: created_at.timestamp(),
        };
        Ok(payment_response)
    }

    async fn save_payment_status(&self, payment_id: Uuid, tx_id: String, v_out: i32, status: String) -> Result<(), Status> {
        let mut connection = self.db.get_pool().get().await.map_err(|_|
            Status::internal("could not update payment status")
        )?;
        let db_tx = connection.transaction().await.map_err(|_|
            Status::internal("could not update payment status")
        )?;

        db_tx.execute(Self::UPDATE_PAYMENT_STATUS, &[&payment_id, &tx_id, &v_out, &status]).await.map_err(|_|
            Status::internal("could not update payment status")
        )?;

        db_tx.commit().await.map_err(|_|
            Status::internal("could not update payment status")
        )?;

        Ok(())
    }
}

#[async_trait]
impl PaymentService for BPaymentService {
    async fn receive(&self, request: Request<PaymentRequest>) -> Result<Response<PaymentResponse>, Status> {
        let payment_request: PaymentRequest = request.into_inner();
        let mut connection = self.db.get_pool().get().await.map_err(|_|
            Status::internal("could not setup connection to database")
        )?;
        let db_tx = connection.transaction().await.map_err(|_|
            Status::internal("could not initiate transaction from_db database")
        )?;

        let owner_id = Uuid::parse_str(&payment_request.owner_id).map_err(|_|
             Status::internal("invalid owner id")
        )?;
        let owner_x_pubkey = BWalletService::owner_x_pubkey(&db_tx, owner_id).await?;

        let network = Network::from_str(payment_request.network.to_lowercase().as_str()).map_err(|_|
            Status::invalid_argument("invalid network")
        )?;

        let new_index = BWalletService::new_child_key_index(&db_tx, owner_id).await?;

        let new_address = HDWallet::new_address(owner_x_pubkey.to_string(), new_index, network)?;

        BWalletService::save_child_key(&db_tx, owner_id, new_index).await?;

        let payment_response = match Self::save_payment(
            &db_tx,
            owner_id,
            payment_request.amount,
            payment_request.currency_code,
            new_address.to_string(),
            new_index,
            payment_request.network.to_lowercase().parse().unwrap(),
            PaymentStatus::Pending.to_string(),
            payment_request.metadata
        ).await {
            Ok(payment_response) => payment_response,
            Err(status) => {
                // Clean up failed new index
                return Err(status)
            }
        };

        db_tx.commit().await.map_err(|_|
            Status::internal("could not commit payment transaction to database")
        )?;

        let payment_id = payment_response.id.to_string();
        let payment_address = payment_response.address.to_string();

        let xplorerbt_url = self.config.xplorerbt_url.to_string();
        let api_url = self.config.url.to_string();
        let http_client = self.http_client.clone();
        spawn(async move {
            let url = format!("{}/api/v1/check/payment", xplorerbt_url);
            let webhook_url = format!("{}/check/payment?id={}", api_url, payment_id);
            let address_info = AddressInfo {
                address: payment_address,
                webhook_url
            };
            http_client.post(url)
                .json(&address_info)
                .send().await.unwrap();
        });

        Ok(Response::new(payment_response))
    }

    async fn get_payment(&self, request: Request<GetPaymentRequest>) -> Result<Response<PaymentResponse>, Status> {
        let id = Uuid::parse_str(request.into_inner().id.as_str()).map_err(|_|
            Status::invalid_argument("invalid id")
        ).unwrap();
        let mut connection = self.db.get_pool().get().await.map_err(|_|
            Status::internal("could not initiate connection from database")
        )?;
        let db_tx = connection.transaction().await.map_err(|_|
            Status::internal("could not initiate transaction from database")
        )?;
        let row = PostgresDb::tx_query_one(
            &db_tx,
            Self::GET,
            &[&id]
        ).await.map_err(|_|
            Status::internal("could not fetch payment from database")
        ).unwrap();
        db_tx.commit().await.map_err(|_|
            Status::internal("could not commit transaction to database")
        )?;
        let payment = PaymentResponse::from_db(row);
        Ok(Response::new(payment))
    }

    async fn update_payment_status(&self, request: Request<PaymentStatusRequest>) -> Result<Response<PaymentStatusResponse>, Status> {
        let payment_status_request = request.into_inner();
        let payment_id = Uuid::parse_str(payment_status_request.payment_id.as_str()).map_err(|_|
            Status::invalid_argument("invalid payment id")
        )?;

        self.save_payment_status(
            payment_id,
            payment_status_request.tx_id,
            payment_status_request.v_out as i32,
            payment_status_request.status
        ).await?;

        Ok(Response::new(PaymentStatusResponse::default()))
    }
}