use std::str::FromStr;
use std::sync::Arc;
use bitcoin::bip32::Xpub;
use tonic::{async_trait, Request, Response, Status};
use uuid::Uuid;
use crate::storage::database::PostgresDb;
use crate::wallet::wallet_service_server::WalletService;
use crate::wallet::{WalletRequest, WalletResponse};

pub struct BWalletService {
    db: Arc<PostgresDb>
}

impl BWalletService {
    pub const CREATE_TABLE: &'static str = "CREATE TABLE IF NOT EXISTS bwallets (
        id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
        owner_x_pubkey VARCHAR(162) NOT NULL UNIQUE,
        created_at TIMESTAMPTZ NOT NULL DEFAULT now()
    );";

    pub const CREATE_CHILD_KEYS_TABLE: &'static str = "CREATE TABLE IF NOT EXISTS bchild_keys (
        id UUID DEFAULT gen_random_uuid() PRIMARY KEY ,
        wallet_id UUID NOT NULL REFERENCES bwallets (id),
        index INT NOT NULL CHECK(index >= 0 AND index <= 2147483647),
        created_at TIMESTAMPTZ NOT NULL DEFAULT now()
    );";

    pub const INSERT: &'static str = "INSERT INTO bwallets (owner_x_pubkey) VALUES ($1) RETURNING id";

    pub fn new(db: Arc<PostgresDb>) -> Self {
        Self {
            db
        }
    }
}

#[async_trait]
impl WalletService for BWalletService {
    async fn create(&self, request: Request<WalletRequest>) -> Result<Response<WalletResponse>, Status> {
        let owner_x_pubkey = match Xpub::from_str(request.into_inner().owner_x_pubkey.as_str()) {
            Ok(x_pub) => x_pub,
            Err(_) => return Err(Status::invalid_argument("invalid extended public key"))
        };

        let id: Uuid = match self.db.query_one(Self::INSERT, &[&owner_x_pubkey.to_string()]).await {
            Ok(response) => response.get("id"),
            Err(_) => return Err(Status::internal("Something went wrong."))
        };

        Ok(Response::new(WalletResponse { owner_id: id.to_string() }))
    }

    async fn restore(&self, request: Request<WalletRequest>) -> Result<Response<WalletResponse>, Status> {
        todo!()
    }
}