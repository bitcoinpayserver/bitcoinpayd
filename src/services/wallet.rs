use std::str::FromStr;
use std::sync::Arc;
use bitcoin::bip32::Xpub;
use deadpool_postgres::Transaction;
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

    pub const GET_BY_ID: &'static str = "SELECT * FROM bwallets WHERE id = $1";

    pub const INSERT_CHILD_KEY_INDEX: &'static str = "INSERT INTO bchild_keys (\
        wallet_id,
        index
    ) VALUES ($1, $2) RETURNING id";

    pub const GET_MAX_INDEX_BY_WALLET_ID: &'static str = "SELECT MAX(index) FROM bchild_keys WHERE wallet_id = $1";

    pub fn new(db: Arc<PostgresDb>) -> Self {
        Self {
            db
        }
    }

    pub async fn owner_x_pubkey(tx: &Transaction<'_>, owner_id: Uuid) -> Result<Xpub, Status> {
        let x_pubkey_row = PostgresDb::tx_query_one(tx, BWalletService::GET_BY_ID, &[&owner_id]).await.map_err(|_|
            Status::internal("could not retrieve owner x-pubkey from database")
        )?;
        let x_pubkey = Xpub::from_str(x_pubkey_row.get("owner_x_pubkey")).map_err(|_|
            Status::internal("invalid owner x-pubkey")
        )?;
        Ok(x_pubkey)
    }

    pub async fn new_child_key_index(tx: &Transaction<'_>, owner_id: Uuid) -> Result<u32, Status> {
        let statement = tx.prepare(Self::GET_MAX_INDEX_BY_WALLET_ID).await.map_err(|_|
            Status::internal("could not retrieve last child key index from database")
        )?;
        let row = tx.query_one(&statement, &[&owner_id]).await.map_err(|_|
            Status::internal("could not retrieve last child key index from database")
        )?;

        let last_index: Option<i32> = row.get("max");
        let new_index = match last_index {
            Some(last_index) => match last_index.checked_add(1) {
                Some(new_index) => new_index as u32,
                None => return Err(Status::internal("Max index exceeded"))
            },
            None => return Ok(0u32),
        };

        Ok(new_index)
    }

    pub async fn save_child_key(tx: &Transaction<'_>, wallet_id: Uuid, index: u32) -> Result<Uuid, Status> {
        let statement = tx.prepare(Self::INSERT_CHILD_KEY_INDEX).await.map_err(|_|
        Status::internal("failed to prepare save new child key index statement")
        )?;
        let row = tx.query_one(&statement, &[&wallet_id, &(index as i32)]).await.map_err(|e| {
            eprintln!("{}", e);
            Status::internal("could not save child key index to database")
        }
        )?;
        let id = row.get("id");
        Ok(id)
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
            Err(e) => return Err(Status::internal("Something went wrong."))
        };

        Ok(Response::new(WalletResponse { owner_id: id.to_string() }))
    }

    async fn restore(&self, request: Request<WalletRequest>) -> Result<Response<WalletResponse>, Status> {
        todo!()
    }
}