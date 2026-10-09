use crate::core::models::{http, AddressInfo, PaymentInfoQuery, PaymentStatus, Utxo};
use crate::core::{payment_status_from_confirmations, encode_bitcoin_uri, svg_qr_code};
use crate::network::grpc_to_http_status;
use crate::payment::payment_service_client::PaymentServiceClient;
use crate::payment::{GetPaymentRequest, PaymentRequest, PaymentStatusRequest};
use crate::server::Config;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::Event;
use axum::response::{Html, IntoResponse, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream::unfold;
use html_escape::encode_text_to_string;
use rust_decimal::Decimal;
use std::convert::Infallible;
use std::str::FromStr;
use std::time::Duration;
use reqwest::Client;
use tokio::spawn;
use tokio::time::interval;
use tonic::codegen::tokio_stream::Stream;
use tonic::transport::{Channel, Uri};
use tonic::Request;
use tower_http::services::ServeDir;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    config: Config,
    http_client: Client,
    payment_service_client: PaymentServiceClient<Channel>
}


pub async fn setup_routes(config: &Config) -> Router {
    let server_uri = Uri::from_str(config.url.as_str()).unwrap();
    let grpc_channel = Channel::builder(server_uri).connect_lazy();
    let http_client = Client::new();
    let payment_service_client = PaymentServiceClient::new(grpc_channel);
    let state = AppState {
        config: config.clone(),
        http_client,
        payment_service_client
    };
    Router::new()
        .route("/", get(heartbeat))
        .route("/receive/payment", post(receive_payment))
        .route("/checkout/payment", get(payment_details_page))
        .route("/payment/status", get(subscribe_to_payment_status))
        .route("/check/payment", post(receive_payment_status))
        .nest_service("/static", ServeDir::new(config.static_dir.clone()))
        .with_state(state)
}

async fn heartbeat() -> impl IntoResponse {
    (StatusCode::OK, "BitcoinPay Server: Online")
}

async fn receive_payment(
    State(state): State<AppState>,
    Json(data): Json<http::PaymentRequest>
) -> impl IntoResponse {
    let mut payment_service_client = state.payment_service_client;

    let payment_request = Request::new(PaymentRequest::from_http(data));
    let response = payment_service_client.receive(payment_request).await.unwrap();
    let payment = response.into_inner();
    let payment_id = payment.id.to_string();
    let payment_address = payment.address.to_string();

    spawn(async move {
        let url = format!("{}/api/v1/check/payment", state.config.xplorerbt_url);
        let webhook_url = format!("{}/check/payment?id={}", state.config.url, payment_id);
        let address_info = AddressInfo {
            address: payment_address,
            webhook_url
        };
        state.http_client.post(url)
            .json(&address_info)
            .send().await.unwrap();
    });

    let http_response = http::PaymentResponse::from_grpc(payment);
    Json(http_response).into_response()
}

async fn payment_details_page(
    State(state): State<AppState>,
    Query(params): Query<PaymentInfoQuery>
) -> impl IntoResponse {
    let payment_request = GetPaymentRequest { id: params.id };
    let grpc_payment_request = Request::new(payment_request);
    let mut payment_service_client = state.payment_service_client;
    let payment = match payment_service_client.get_payment(grpc_payment_request).await {
        Ok(payment_response) => payment_response.into_inner(),
        Err(status) => return grpc_to_http_status(status).into_response(),
    };

    let payment_id = payment.id;
    let mut address = "".to_string();
    encode_text_to_string(payment.address.as_str(), &mut address);

    let mut payment_status = "".to_string();
    encode_text_to_string(payment.payment_status.to_uppercase(), &mut payment_status);

    let amount = Decimal::from_str(payment.amount.as_str()).unwrap().normalize();

    let mut label = "Payment".to_string();
    match payment.metadata.get("label") {
        Some(l) => label = l.to_string(),
        None => {}
    };
    let mut message = "Payment".to_string();
    match payment.metadata.get("message") {
        Some(m) => message = m.to_string(),
        None => {}
    };

    let uri = match encode_bitcoin_uri(
        &address,
        &amount,
        &message,
        &label,
        &payment.network,
    ) {
        Ok(uri) => uri,
        Err(status) => return grpc_to_http_status(status).into_response(),
    };

    let mut url = uri.to_string();
    encode_text_to_string(uri.as_str(), &mut url);

    let qr_code = match svg_qr_code(uri.as_str(), state.config.static_dir) {
        Ok(qr_code) => qr_code,
        Err(status) => return grpc_to_http_status(status).into_response(),
    };

    let page = format!(
        "
        <!DOCTYPE html>
            <html>
                <head>
                    <title>BitcoinPay Server</title>
                    <link rel=\"icon\" type=\"image/svg+xml\" href=\"/static/icons/bitcoin.png\" />
                    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />
                    <style>
                        #address-container {{
                            background:#F6F1EB;
                            margin-left: 20px;
                            margin-right: 20px;
                            margin-top: 10px;
                            margin-bottom: 10px;
                            height: fit-content;
                            padding: 10px;
                            border-radius: 10px;
                        }}
                        #address-container:active {{
                            background: rgba(255, 233, 158, 0.45);
                        }}
                        #to-wallet {{
                            background:#FBD37B;
                            margin:10px;
                            margin-bottom:20px;
                            height:48px;
                            border-radius:10px;
                            border: none;
                            padding:5px;
                            font-size: 16px;
                            padding: 10px;
                        }}
                        #to-wallet:active {{
                            background: rgba(255, 233, 158, 0.45);
                        }}
                        #payment-info-container {{
                            border-radius: 16px;
                        }}
                        #payment-status-container {{
                            animation: pending-color 2s infinite alternate;
                            align-items: center;
                            padding: 4px;
                            margin: auto;
                            margin-bottom: 20px;
                        }}
                        @keyframes pending-color {{
                            0% {{ background: #879085; }}
                            100% {{ background: #F6F1EB; }}
                        }}
                        @keyframes awaiting-confirmation-color {{
                            0% {{ background: #B3AC4F; }}
                            100% {{ background: #F6F1EB; }}
                        }}
                    </style>
                </head>
                <body style=\"padding:0; margin:0; background:#FDF6F0; font-family:sans-serif;\">
                    <div style=\"display:flex; justify-content:center; height: 100vh; padding:0;\">
                        <div style=\"background:#F6F1EB; width:26em; height:fit-content; margin:auto; text-align:center; border-radius: 16px; border: 1px solid #74706A;\">
                            <h1>BitcoinPay</h1>
                            <div id=\"payment-status-container\">
                                <p id=\"payment-status\" style=\"width:fit-content; font-size:1.3rem; font-size:12px; margin: 4px; margin-right:auto; margin-left:auto;\"></p>
                            </div>
                            <div id=\"payment-info-container\">
                                <ul style=\"display:flex; flex-direction: row; align-items: center; width:fit-content; gap:120px; padding:0; margin:auto; margin-bottom:20px;\">
                                    <li style=\"display:inline-block; \"><p style=\"width:fit-content; font-size:1.2rem;\">To pay: ₿{amount} (XBT)</p></li>
                                </ul>
                                <div style=\"border-radius:5px;\">{qr_code}</div>
                                <div  id=\"address-container\">
                                    <p style=\"overflow-wrap:break-word; height: fit-content; border-radius:8px; font-size:12px;\">{address}</p>
                                    <img src=\"/static/icons/copy.svg\"  style = \"color: #F9FAEF; background: #F6F1EB; padding: 8px; width: 16px; height: 16px; border-radius: 16px;\"/>
                                </div>
                                <button id=\"to-wallet\">Pay from Wallet</button>
                            </div>
                        </div>
                    </div>
                    <script type=\"text/javascript\">
                        const addressContainer = document.getElementById(\"address-container\");
                        addressContainer.addEventListener(
                            'click',
                            () => {{
                                const address = addressContainer.querySelector('p').textContent
                                navigator.clipboard.writeText(address)
                            }}
                        );

                        const walletButton = document.getElementById(\"to-wallet\");
                        walletButton.addEventListener(
                            'click',
                            () => {{
                                window.location.href = \"{uri}\";
                            }}
                        );

                        const eventSource = new EventSource(\"/payment/status?id={payment_id}\");

                        eventSource.onmessage = (event) => {{
                            const data = JSON.parse(event.data);

                            if (data.timeout <= 0 || data.status === \"Confirmed\") {{
                                eventSource.close();
                            }}

                            if (data.status === \"Pending\") {{
                                const mins = Math.trunc(data.timeout / 60);
                                const secs = data.timeout % 60
                                const timeout = mins > 0 ? `${{mins}}m ${{secs}}s` : `${{secs}}s`
                                const paymentStatusContainer = document.getElementById(\"payment-status-container\");
                                paymentStatusContainer.style.animation = 'pending-color 1s infinite alternate';

                                document.getElementById(\"payment-status\").textContent = data.timeout > 0 ? `${{data.status}} (${{timeout}})` : `${{data.status}}`;
                            }}

                            if (data.status === \"Confirmed\") {{
                                document.getElementById(\"payment-status\").textContent = `${{data.status}}`;
                                const paymentStatusContainer = document.getElementById(\"payment-status-container\");
                                paymentStatusContainer.style.animation = 'none';
                                paymentStatusContainer.style.background = '#3BB152';
                            }}

                            if (data.status === \"AwaitingConfirmation\") {{
                                document.getElementById(\"payment-status\").textContent = `Awaiting Confirmation`;
                                const paymentStatusContainer = document.getElementById(\"payment-status-container\");
                                paymentStatusContainer.style.animation = 'awaiting-confirmation-color 1s infinite alternate';
                            }}

                            if ((data.timeout <= 0 && data.status === \"Pending\") || data.status === \"Expired\") {{
                                document.getElementById(\"payment-status\").textContent = `${{data.status}}`;
                                const paymentInfoContainer = document.getElementById(\"payment-info-container\");
                                paymentInfoContainer.style.filter = 'blur(8px)';
                                const paymentStatusContainer = document.getElementById(\"payment-status-container\");
                                paymentStatusContainer.style.animation = 'none';
                            }}
                        }};
                    </script>
                </body>
            </html>
    "
    );

    Html(page).into_response()
}

async fn receive_payment_status(
    State(state): State<AppState>,
    Query(params): Query<PaymentInfoQuery>,
    Json(data): Json<Utxo>
) -> impl IntoResponse {
    let payment_status = payment_status_from_confirmations(
        data.confirmations,
        state.config.min_confirmations,
        data.tx_id.to_string(),
        data.v_out,
        data.amount
    );

    let mut payment_service_client = state.payment_service_client;
    let payment_status_request = Request::new(PaymentStatusRequest {
        payment_id: params.id,
        tx_id: data.tx_id,
        v_out: data.v_out as u32,
        status: payment_status.to_string(),
    });
    payment_service_client.update_payment_status(payment_status_request).await.unwrap();
}

async fn subscribe_to_payment_status(
    State(state): State<AppState>,
    Query(params): Query<PaymentInfoQuery>
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let payment_id = Uuid::from_str(params.id.as_str()).unwrap();

    let interval = interval(Duration::from_secs(1));
    let timeout = 15 * 60;

    let stream = unfold(
        (interval, timeout, payment_id, state.payment_service_client),
        async move | state| {
            let mut interval = state.0;
            let mut timeout = state.1;
            let payment_id = state.2;
            let mut payment_service_client = state.3;

            interval.tick().await;

            let payment_request = Request::new(GetPaymentRequest { id: payment_id.to_string() });
            let response = payment_service_client.get_payment(payment_request).await.unwrap().into_inner();

            let mut json_payload = format!(
                r#"{{"timeout": {}, "status": "{}" }}"#,
                if response.payment_status == PaymentStatus::Expired.to_string() { 0 } else { timeout },
                response.payment_status
            );

            let mut state = (interval, timeout, payment_id, payment_service_client.clone());
            if timeout <= 0 && response.payment_status == PaymentStatus::Pending.to_string() {
                let status = PaymentStatus::Expired.to_string();
                json_payload = format!(
                    r#"{{"timeout": {}, "status": "{}" }}"#,
                    timeout,
                    PaymentStatus::Expired.to_string()
                );
                let payment_status_request = Request::new(PaymentStatusRequest {
                    payment_id: payment_id.to_string(),
                    tx_id: "".to_string(),
                    v_out: 0,
                    status,
                });
                payment_service_client.update_payment_status(payment_status_request).await.unwrap();

                return Some((Ok(Event::default().data(json_payload)), state));
            }

            timeout -= 1;

            state.1 = timeout;

            Some((Ok(Event::default().data(json_payload)), state))
        });

    Sse::new(stream)
}