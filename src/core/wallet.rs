use bitcoin::bip32::{ChildNumber, Xpub};
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::All;
use bitcoin::{Address, Network, XOnlyPublicKey};
use std::str::FromStr;
use tonic::Status;

pub struct HDWallet;

impl HDWallet {
    pub fn new_address(
        x_pub_key: String,
        index: u32,
        network: Network,
    ) -> Result<Address, Status> {
        let secp256k1_context = Secp256k1::new();
        let new_child_pub_key = Self::child_key(&secp256k1_context, x_pub_key, index)?;
        let address = Address::p2tr(&secp256k1_context, new_child_pub_key, None, network);
        Ok(address)
    }

    fn child_key(secp256k1_context: &Secp256k1<All>, x_pub_key: String, index: u32, ) -> Result<XOnlyPublicKey, Status> {
        let account_number = ChildNumber::from_normal_idx(0).map_err(|_|
            Status::internal("failed to parse child account number")
        )?;
        let child_key_number = ChildNumber::from_normal_idx(index).map_err(|_|
            Status::internal("failed to parse child key number")
        )?;
        let key_path = &[account_number, child_key_number];

        let parent_key = Xpub::from_str(x_pub_key.as_str()).map_err(|_|
            Status::internal("failed to parse x-pubkey")
        )?;

        let child_key = parent_key.derive_pub(&secp256k1_context, &key_path).map_err(|_|
        Status::internal("failed to derive child key")
        )?.to_x_only_pub();

        Ok(child_key)
    }
}