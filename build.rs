fn main() {
    let result = tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .build_transport(true)
        .compile_protos(
            &[
                "proto/bitcoinpayserver/v1/services/payment.proto",
                "proto/bitcoinpayserver/v1/services/wallet.proto",
            ],
            &["proto"],
        );
    match result {
        Ok(_) => {}
        Err(error) => panic!("Failed to compile protos {:?}", error),
    }
}
