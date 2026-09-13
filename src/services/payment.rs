use std::sync::Arc;
use tonic::{async_trait, Request, Response, Status};
use crate::payment::payment_service_server::PaymentService;
use crate::payment::{PaymentRequest, PaymentResponse};
use crate::storage::database::PostgresDb;

pub struct BPaymentService {
    db: Arc<PostgresDb>,
}

impl BPaymentService {
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