mod db;
mod engine;

// These constants represent the RISC-V ELF and the image ID generated by risc0-build.
// The ELF is used for proving and the ID is used for verification.
use engine::ChessEngine;
use hyle_contract::{HyleInput, HyleOutput};
use referee::{
    hyle::HyleNetwork,
    server::{EmailServer, ServerConfig},
};

// TODO: abstract this better
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain: String = std::env::var("REFEREE_IMAP_DOMAIN").unwrap();
    let username: String = std::env::var("REFEREE_IMAP_USERNAME").unwrap();
    let password: String = std::env::var("REFEREE_IMAP_PASSWORD").unwrap();
    let port: u16 = std::env::var("REFEREE_IMAP_PORT").unwrap().parse().unwrap();
    let mut engine =
        ChessEngine::new(HyleNetwork::Devnet, username.as_ref(), password.as_ref()).await?;
    let mut server = EmailServer::new(
        &mut engine,
        "CheckmateVerifierV2",
        &domain,
        port,
        &username,
        &password,
    );
    server.run().await;

    Ok(())
}
