use std::str::FromStr;
use base64::Engine;
use base64::engine::general_purpose;
use bip321::Bip321Uri;
use bitcoin::{Address, Amount};
use qrcode::{EcLevel, QrCode};
use qrcode::render::svg;
use rust_decimal::Decimal;
use tonic::Status;
use crate::core::models::PaymentStatus;

pub mod wallet;
pub mod models;

pub fn encode_bitcoin_uri(
    address: &String,
    amount: &Decimal,
    label: &String,
    message: &String,
    network: &String,
) -> Result<String, Status> {
    let mut uri: Bip321Uri = Bip321Uri::new();

    let address = match Address::from_str(address) {
        Ok(address) => address,
        Err(_) => return Err(Status::invalid_argument("invalid address")),
    };

    match Amount::from_btc(amount.as_f64()) {
        Ok(amount) => match uri.set_amount(amount) {
            Ok(_) => {}
            Err(_) => return Err(Status::invalid_argument("invalid amount")),
        },
        Err(_) => return Err(Status::invalid_argument("invalid amount")),
    }

    uri.set_label(label.to_string());
    uri.set_message(message.to_string());

    let is_testnet: bool = network == "testnet"
        || network == "signet"
        || network == "testnet4"
        || network == "regtest";
    if is_testnet {
        match uri.push_tb(address, false) {
            Ok(_) => {}
            Err(_) => return Err(Status::invalid_argument("invalid testnet address")),
        }
    } else {
        match uri.set_address(address) {
            Ok(_) => {}
            Err(_) => return Err(Status::invalid_argument("invalid address")),
        };
    }

    match uri.checked_uppercase() {
        Some(uppercase_uri) => Ok(uppercase_uri),
        None => Ok(uri.to_string()),
    }
}

pub fn svg_qr_code(data: &str, static_dir: String) -> Result<String, Status> {
    let code = match QrCode::with_error_correction_level(data, EcLevel::H) {
        Ok(code) => code,
        Err(_) => return Err(Status::internal("failed to generate qr code")),
    };
    let svg_string = code.render::<svg::Color>().min_dimensions(256, 256).build();

    let logo_bytes = match std::fs::read(format!("{static_dir}/icons/bitcoin.png")) {
        Ok(bytes) => bytes,
        Err(_) => return Err(Status::internal("failed to read QR code logo")),
    };
    let logo_base64 = general_purpose::STANDARD.encode(&logo_bytes);

    let inline_logo_url = format!("data:image/png;base64,{}", logo_base64);

    let logo_size = 18;
    let logo_x_y = (100 - logo_size) / 2;
    let background_margin = 0;
    let background_size = logo_size + (background_margin * 2);
    let background_radius = background_size / 2;
    let center = 50;

    let overlay_tag = format!(
        "<defs>
            <clipPath id=\"circle-clip\">
                <circle cx=\"{}%\" cy=\"{}%\" r=\"{}%\" />
            </clipPath>
        </defs>

        <circle cx=\"{}%\" cy=\"{}%\" r=\"{}%\" fill=\"#FFFFFF\" />

        <image href=\"{}\"
               x=\"{}%\"
               y=\"{}%\"
               width=\"{}%\"
               height=\"{}%\"
               clip-path=\"url(#circle-clip)\" />
        </svg>",
        center,
        center,
        background_radius,
        center,
        center,
        background_radius,
        inline_logo_url,
        logo_x_y,
        logo_x_y,
        logo_size,
        logo_size,
    );

    let final_svg = svg_string.replace("<?xml version=\"1.0\" standalone=\"yes\"?>", "").replace("</svg>", &overlay_tag).replace(
        "<rect x=\"0\" y=\"0\"",
        "<rect x=\"0\" y=\"0\" rx=\"3%\" ry=\"3%\"",
    );
    Ok(final_svg)
}

pub fn payment_status_from_confirmations(
    confirmations: Option<i32>,
    min_confirmations: i32,
    tx_id: String,
    v_out: i32,
    amount: Decimal
) -> PaymentStatus {
    match confirmations {
        None => PaymentStatus::Pending,
        Some(conf) =>  {
            if conf < min_confirmations {
                return PaymentStatus::AwaitingConfirmation { tx_id, v_out, amount }
            }
            PaymentStatus::Confirmed
        }
    }
}