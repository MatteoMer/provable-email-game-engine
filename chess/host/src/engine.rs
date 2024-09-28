use std::str::FromStr;

use async_imap::types::Fetch;
use hyle_contract::{HyleInput, HyleOutput};
use mailparse::*;
use methods::{CHESS_GUEST_ELF, CHESS_GUEST_ID};
use referee::hyle::HyleNetwork;
use referee::server::ServerConfig;
use regex::Regex;
use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};

use shakmaty::{fen::Fen, san::San, Board, CastlingMode, Chess, Move, Position};

#[derive(Clone, Copy)]
pub struct ChessEngine {
    network: HyleNetwork,
}

impl ChessEngine {
    pub fn new(network: HyleNetwork) -> Self {
        Self { network }
    }
}

impl ServerConfig for ChessEngine {
    // maybe program_inputs
    // not with a string?
    fn process_email(&self, message: &Fetch) -> Option<(Vec<u8>, String, String)> {
        //TODO: process email instead of hardcoding PGN

        let body = message.body().expect("message did not have a body!");

        let parsed_email = parse_mail(body).unwrap();
        let null_state = 0u32.to_be_bytes().to_vec();
        let identiy = "".to_string();

        if let Some(body_part) = parsed_email.subparts.first() {
            let body_content = body_part.get_body().ok()?;

            // Extract MOVE
            let move_regex = Regex::new(r"MOVE:\s*(.+)").unwrap();
            let chess_move_string = move_regex
                .captures(&body_content)
                .and_then(|cap| cap.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or("Move not found");

            // Extract FEN
            let fen_regex = Regex::new(r"FEN:\s*(.+)").unwrap();
            let fen_string = fen_regex
                .captures(&body_content)
                .and_then(|cap| cap.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or("FEN not found");

            println!("MOVE: {}", chess_move_string);
            println!("FEN: {}", fen_string);

            let fen = Fen::from_ascii(fen_string.as_bytes());

            // invalid fen in the mail
            // TODO: send mail about the invalid move to the player
            if fen.is_err() {
                return None;
            }
            let fen = fen.unwrap();

            // TODO: not making the server crash when someone sends an illegal move
            let position = fen.into_position::<Chess>(CastlingMode::Standard).unwrap();
            let chess_move: Move = chess_move_string
                .parse::<San>()
                .unwrap()
                .to_move(&position)
                .unwrap();

            let position = position.play(&chess_move).unwrap();
            // only prove the move when it's mate
            if position.is_checkmate() {
                let inputs = (chess_move_string, fen_string);
                let serialized_args = serde_json::to_string(&inputs).unwrap();
                Some((null_state, identiy, serialized_args))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn prove(&self, input: &HyleInput<String>) -> Receipt {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
            .init();

        /* RISC0 Proving */
        let null_state = 0u32.to_be_bytes().to_vec();
        let env = ExecutorEnv::builder()
            .write(&input)
            .unwrap()
            .build()
            .unwrap();

        let prover = default_prover();
        let proof_info = prover.prove(env, CHESS_GUEST_ELF).unwrap();

        proof_info.receipt
    }
}
