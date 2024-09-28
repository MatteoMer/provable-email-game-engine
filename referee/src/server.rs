use crate::hyle::{Hyle, HyleNetwork};
use base64::prelude::*;
use chrono::{DateTime, NaiveDateTime, Utc};
use hyle_contract::HyleInput;
use risc0_zkvm::{sha::Digestible, Receipt};
use std::{env, str};

use anyhow::Context;
use async_imap::{types::Fetch, Client};
use async_native_tls::TlsConnector;
use async_std::net::TcpStream;
use async_std::task;
use futures::TryStreamExt;
use tokio::time::{self, Duration};

pub trait ServerConfig {
    fn process_email(&self, content: &Fetch) -> Option<(Vec<u8>, String, String)>;
    fn prove(&self, hyle_input: &HyleInput<String>) -> Receipt;
}

#[derive(Clone)]
pub struct EmailServer<T: ServerConfig> {
    domain: String,
    port: u16,
    username: String,
    password: String,
    contract_name: String,
    config: T,
    last_checked: Option<DateTime<Utc>>,
}

impl<T: ServerConfig + Clone> EmailServer<T> {
    pub fn new(
        config: &T,
        contract_name: &str,
        domain: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> Self {
        Self {
            domain: domain.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            port,
            config: config.clone(),
            contract_name: contract_name.to_string(),
            last_checked: None,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.last_checked = Some(Utc::now());
        loop {
            if let Err(e) = self.process_emails().await {
                eprintln!("Error processing emails: {:?}", e);
            }

            task::sleep(Duration::from_secs(3)).await;
        }
    }

    async fn process_emails(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let imap_addr = (self.domain.as_str(), self.port);
        let tcp_stream = TcpStream::connect(imap_addr).await?;
        let tls = TlsConnector::new();
        let tls_stream = tls.connect(self.domain.as_str(), tcp_stream).await?;

        let client = Client::new(tls_stream);
        println!("-- connected to {}:{}", self.domain, self.port);

        let mut imap_session = client
            .login(&self.username, &self.password)
            .await
            .map_err(|e| e.0)?;
        println!("-- logged in as {}", self.username);

        let inbox = imap_session.select("INBOX").await?;
        println!("-- INBOX selected. Message count: {}", inbox.exists);

        if inbox.exists == 0 {
            println!("-- No messages in the inbox.");
            return Ok(());
        }

        // Fetch all messages
        let fetch_sequence = format!("1:{}", inbox.exists);
        println!("-- Fetching messages with sequence: {}", fetch_sequence);

        let messages_stream = imap_session
            .fetch(&fetch_sequence, "(RFC822 INTERNALDATE)")
            .await?;

        let messages: Vec<_> = messages_stream.try_collect().await?;

        println!("-- Fetched {} messages", messages.len());

        for message in messages.into_iter() {
            let internal_date = message.internal_date().expect("no internal date found");
            if internal_date > self.last_checked.expect("no last checked") {
                println!(
                    "internal_date: {}, last_checked: {}",
                    internal_date,
                    self.last_checked.unwrap()
                );
                let processed_message = self.config.process_email(&message);
                // if it's some, then we need to process the risc0 proof
                if let Some((initial_state, identity, program_inputs)) = processed_message {
                    let _ = self
                        .compute_and_publish_risc0_proof(&initial_state, &identity, &program_inputs)
                        .await;
                }
                self.last_checked = Some(internal_date.into());
            }
        }

        // Update the last checked time

        imap_session.logout().await?;

        Ok(())
    }

    // note: program_inputs is a serde serialized string
    async fn compute_and_publish_risc0_proof(
        &self,
        initial_state: &[u8],
        identity: &str,
        program_inputs: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Publishing hyle payload and getting the inputs

        let mut home_dir = std::env::home_dir().unwrap();
        // TODO: change to configurable path
        home_dir.push("projects/hyle-cosmos/hyled");

        let hyle = Hyle::new(HyleNetwork::Devnet, &home_dir);

        let hyle_input = hyle
            .publish_payload(
                identity,
                &self.contract_name,
                &BASE64_STANDARD.encode(program_inputs),
                initial_state,
                program_inputs,
            )
            .unwrap();

        // Proving that inputs are valid w/ risc0
        let receipt = self.config.prove(&hyle_input);
        let receipt_json = serde_json::to_string(&receipt)?;
        std::fs::write("proofs/proof.json", receipt_json)?;
        let result: bool = receipt.journal.decode::<bool>()?;

        // TODO remove maybe?
        receipt.verify(receipt.inner.claim()?.value()?.pre.digest());

        // Posting the proof on hyle to settle

        let mut proof_dir = env::current_dir()?;
        proof_dir.push("proofs/proof.json");
        hyle.broadcast_proof(
            str::from_utf8(&hyle_input.tx_hash)?,
            &self.contract_name,
            "0".as_ref(),
            &proof_dir,
        )
    }
}
