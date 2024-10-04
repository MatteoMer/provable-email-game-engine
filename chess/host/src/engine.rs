use std::{path::Path, str::FromStr};

use async_imap::types::Fetch;
use hyle_contract::{HyleInput, HyleOutput};
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, Message,
    SmtpTransport, Transport,
};
use mailparse::*;
use methods::{CHESS_GUEST_ELF, CHESS_GUEST_ID};
use referee::hyle::HyleNetwork;
use referee::server::ServerConfig;
use regex::Regex;
use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};
use shakmaty::{
    fen::Fen, san::San, Board, CastlingMode, Chess, EnPassantMode, Move, Position, Setup,
};
use tracing_subscriber::fmt::format;
use urlencoding::encode;

// TODO: move email stuff to ref
#[derive(Clone)]
pub struct ChessEngine {
    network: HyleNetwork,
    ref_mail: String,
    mailer: SmtpTransport,
}

impl ChessEngine {
    pub async fn new(
        network: HyleNetwork,
        username: &str,
        password: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: make it in the ref and customizable
        let server = "smtp.gmail.com";
        let port = 465;
        let mailer = SmtpTransport::relay(server)?
            .credentials(Credentials::new(username.to_string(), password.to_string()))
            .port(port)
            .build();
        Ok(Self {
            network,
            ref_mail: username.to_string(),
            mailer,
        })
    }

    fn send_move_by_mail(
        &self,
        from: &str,
        to_mail: &str,
        to_name: &str,
        fen: &str,
        chess_move: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = Message::builder()
            .from(format!("Referee <{}>", from).parse()?)
            .to(format!("{} <{}>", to_name, to_mail).parse()?)
            .subject("New move from your opponent")
            .header(ContentType::TEXT_HTML)
            .body(String::from(format!("<body><p>Your opponent played <b>{}</p><p><img src=\"https://fen2image.chessvision.ai/{}.png\"></p></body>", chess_move, encode(fen))))?;

        self.mailer.send(&message)?;
        Ok(())
    }

    fn extract_from_addr(&self, mail: &ParsedMail) -> (String, String) {
        // Parse the address
        match &addrparse_header(mail.headers.get_first_header("From").unwrap()).unwrap()[0] {
            MailAddr::Single(info) => {
                return (info.display_name.clone().unwrap(), info.addr.clone())
            }
            _ => panic!(),
        }
    }
}

impl ServerConfig for ChessEngine {
    fn process_email(&mut self, message: &Fetch) -> Option<(Vec<u8>, String, String)> {
        let body = message.body().expect("message did not have a body!");

        let parsed_email = parse_mail(body).unwrap();
        let null_state = 0u32.to_be_bytes().to_vec();
        let identiy = "".to_string();

        if let Some(body_part) = parsed_email.subparts.first() {
            let body_content = body_part.get_body().ok()?;

            let (from_id, from_addr) = self.extract_from_addr(&parsed_email);

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
                let setup: Setup = position.into_setup(EnPassantMode::Legal);
                let fen: Fen = setup.into();
                self.send_move_by_mail(
                    &self.ref_mail,
                    &from_addr,
                    &from_id,
                    &fen.to_string(),
                    chess_move_string,
                )
                .expect(format!("could not send mail to {}", from_addr).as_str());
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
