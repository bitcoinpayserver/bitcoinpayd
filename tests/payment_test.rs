use bitcoinpayd::wallet::wallet_service_server::WalletService;
use bitcoinpayd::payment::payment_service_server::PaymentService;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use bitcoin::{Address, Network};
use chrono::DateTime;
use tonic::Request;
use uuid::Uuid;
use bitcoinpayd::payment::PaymentRequest;
use bitcoinpayd::services::payment::BPaymentService;
use bitcoinpayd::services::wallet::BWalletService;
use bitcoinpayd::storage::database::PostgresDb;
use bitcoinpayd::wallet::WalletRequest;

#[tokio::test]
async fn test_receiving_payment_request() {
    let (wallet_service, payment_service) = setup_test_services().await;

    let wallet_id = setup_wallet(wallet_service).await;

    let payment_request = Request::new(PaymentRequest {
        owner_id: wallet_id.clone(),
        amount: "3.70000000".to_string(),
        network: Network::Regtest.to_string(),
        currency_code: "XBT".to_string(),
        metadata: HashMap::new(),
    });

    let payment_response = payment_service.receive(payment_request).await.unwrap().into_inner();

    assert_eq!(payment_response.owner_id, wallet_id);
    assert_eq!(payment_response.amount, "3.70000000");
    assert_eq!(payment_response.network, "regtest");
    assert_eq!(payment_response.currency_code, "XBT");
    assert!(payment_response.metadata.is_empty());

    assert!(!payment_response.address.is_empty());
    let address = Address::from_str(payment_response.address.as_str()).unwrap();
    assert!(address.is_valid_for_network(Network::Regtest));

    assert!(!payment_response.id.is_empty());
    let uuid = Uuid::from_str(&payment_response.id);
    assert!(uuid.is_ok());

    assert!(!payment_response.child_key_index.is_negative());

    let time = DateTime::from_timestamp_millis(payment_response.created_at);
    assert!(time.is_some())
}

#[tokio::test]
async fn test_rejecting_invalid_payment_request() {
    let (wallet_service, payment_service) = setup_test_services().await;
    let wallet_id = setup_wallet(wallet_service).await;

    let invalid_payment_request = Request::new(PaymentRequest {
        owner_id: wallet_id.clone(),
        amount: "-3.70000000".to_string(),
        network: "unknown".to_string(),
        currency_code: "BTC".to_string(),
        metadata: Default::default(),
    });
    let payment_response = payment_service.receive(invalid_payment_request).await;
    assert!(payment_response.is_err());
}

async fn setup_test_services() -> (BWalletService, BPaymentService) {
    let postgres = PostgresDb::new(None).await.unwrap();
    let db = Arc::new(postgres);

    db.create_tables(
        &[
            BWalletService::CREATE_TABLE,
            BWalletService::CREATE_CHILD_KEYS_TABLE,
            BPaymentService::CREATE_TABLE
        ]
    ).await.unwrap();

    let wallet_service = BWalletService::new(db.clone());
    let payment_service = BPaymentService::new(db.clone());
    (wallet_service, payment_service)
}

async fn setup_wallet(wallet_service: BWalletService) -> String {
    let new_wallet_request = Request::new(WalletRequest {
        owner_x_pubkey: "tpubDG5KEfa5LG3NC7RLsJxxajR9MF7vZqKd1YexvS4LpVthKeu9gnGoNpt8FnUtb4iHnb4GbLD37LFHyCq4ouydRAesPhe1sZg4cqdiFSRvr92".to_string()
    });

    let new_wallet_response = wallet_service.create(new_wallet_request).await.unwrap().into_inner();

    assert!(!new_wallet_response.owner_id.is_empty());
    let uuid = Uuid::from_str(&new_wallet_response.owner_id);
    assert!(uuid.is_ok());

    uuid.unwrap().to_string()
}