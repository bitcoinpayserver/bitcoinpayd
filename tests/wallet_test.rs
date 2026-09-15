use std::collections::HashSet;
use bitcoin::Network;
use bitcoinpayd::core::wallet::HDWallet;

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