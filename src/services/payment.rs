use std::sync::Arc;
use tonic::{async_trait, Request, Response, Status};
use crate::payment::payment_service_server::PaymentService;
use crate::payment::{PaymentRequest, PaymentResponse};
use crate::storage::database::PostgresDb;

pub struct BPaymentService {
    db: Arc<PostgresDb>,
}

impl BPaymentService {
    pub const CREATE_TABLE: &'static str = "CREATE TABLE IF NOT EXISTS bpayments (
        id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
        owner_id UUID NOT NULL REFERENCES bwallets(id),
        amount NUMERIC(24) NOT NULL,
        currency_code VARCHAR(4) NOT NULL,
        address VARCHAR(164) NOT NULL UNIQUE,
        child_key_index INT NOT NULL CHECK(child_key_index >= 0 AND child_key_index <= 2147483647),
        network VARCHAR(24) NOT NULL,
        status VARCHAR(24) NOT NULL,
        metadata JSONB,
        created_at TIMESTAMPTZ NOT NULL DEFAULT now()
    )";
    
    pub fn new(db: Arc<PostgresDb>) -> Self {
        Self {
            db
        }
    }
}

#[async_trait]
impl PaymentService for BPaymentService {
    async fn receive(&self, request: Request<PaymentRequest>) -> Result<Response<PaymentResponse>, Status> {
        todo!()
    }
}