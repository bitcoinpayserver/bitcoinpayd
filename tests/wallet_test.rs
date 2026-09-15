use bitcoinpayd::wallet::wallet_service_server::WalletService;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use bitcoin::Network;
use tonic::Request;
use uuid::Uuid;
use bitcoinpayd::core::wallet::HDWallet;
use bitcoinpayd::services::wallet::BWalletService;
use bitcoinpayd::storage::database::PostgresDb;
use bitcoinpayd::wallet::WalletRequest;

const X_PUB_KEY: &str = "tpubDG5KEfa5LG3NC7RLsJxxajR9MF7vZqKd1YexvS4LpVthKeu9gnGoNpt8FnUtb4iHnb4GbLD37LFHyCq4ouydRAesPhe1sZg4cqdiFSRvr92";
#[test]
fn test_new_address_derivation_on_testnet() {
    let mut addresses = HashSet::new();
    for index in 0 .. 2000 {
        let result = HDWallet::new_address(X_PUB_KEY.to_string(), index, Network::Testnet);

        assert!(result.is_ok());

        let address = result.unwrap().to_string();

        assert!(!address.is_empty());
        assert!(address.starts_with("tb"));
        assert!(addresses.insert(address));
    }
}

#[tokio::test]
async fn test_creating_wallet_on_testnet() {
    let test_postgres = PostgresDb::new(None).await;
    assert!(test_postgres.is_ok());

    let db = Arc::new(test_postgres.unwrap());

    let result = db.create_tables(&[BWalletService::CREATE_TABLE]).await;
    assert!(result.is_ok());

    let wallet_service = BWalletService::new(db);

    let request = Request::new(WalletRequest { owner_x_pubkey: X_PUB_KEY.to_string() });
    let result = wallet_service.create(request).await;

    assert!(result.is_ok());

    let response = result.unwrap().into_inner();

    assert!(!response.owner_id.is_empty());

    let uuid_result = Uuid::from_str(&response.owner_id);
    assert!(uuid_result.is_ok())
}