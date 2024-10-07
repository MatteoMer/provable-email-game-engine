use std::{path::Path, str::FromStr};

use async_imap::types::Fetch;
use hyle_contract::{HyleInput, HyleOutput};
use lettre::{
    message::header::{self, ContentType},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
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

use crate::db::{DbManager, Game};

// TODO: move email stuff to ref
pub struct ChessEngine {
    network: HyleNetwork,
    ref_mail: String,
    mailer: SmtpTransport,
    db: DbManager,
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

        let db_manager = DbManager::new("chess_positions.db")?;

        Ok(Self {
            network,
            ref_mail: username.to_string(),
            mailer,
            db: db_manager,
        })
    }

    fn send_move_by_mail(
        &self,
        from: &str,
        to_mail: &str,
        to_name: &str,
        opponent_mail: &str,
        opponent_name: &str,
        fen: &str,
        chess_move: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = Message::builder()
            .from(format!("Referee <{}>", from).parse()?)
            .to(format!("{} <{}>", to_name, to_mail).parse()?)
            .cc(format!("{} <{}>", opponent_name, opponent_mail).parse()?)
            .subject(format!("New valid move from {}", to_mail))
            .header(ContentType::TEXT_HTML)
            .body(String::from(format!("<body><p>{} played <b>{}</p><p><img src=\"https://fen2image.chessvision.ai/{}.png\"></p><p>Current FEN (to pass in next mail): {}</p></body>", to_mail, chess_move, encode(fen), fen)))?;

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

    fn extract_to_addr(&self, mail: &ParsedMail) -> (String, String) {
        // Parse the address
        match &addrparse_header(mail.headers.get_first_header("To").unwrap()).unwrap()[0] {
            MailAddr::Single(info) => {
                return (info.display_name.clone().unwrap(), info.addr.clone())
            }
            _ => panic!(),
        }
    }

    fn extract_cc_addr(&self, mail: &ParsedMail) -> (String, String) {
        // Parse the address
        match &addrparse_header(mail.headers.get_first_header("Cc").unwrap()).unwrap()[0] {
            MailAddr::Single(info) => {
                return (info.display_name.clone().unwrap(), info.addr.clone())
            }
            _ => panic!(),
        }
    }

    fn parse_mail_body(&self, body_content: &str) -> (String, Fen, bool) {
        // Extract MOVE
        let move_regex = Regex::new(r"MOVE:\s*(.+)").unwrap();
        let chess_move_string = move_regex
            .captures(&body_content)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim())
            .unwrap_or("Move not found");

        // Check for NEW GAME
        let new_game_regex = Regex::new(r"NEW GAME:\s*(\S+@\S+)").unwrap();
        let (is_new_game, _) = new_game_regex
            .captures(&body_content)
            .map(|cap| (true, cap.get(1).map(|m| m.as_str().to_string())))
            .unwrap_or((false, None));

        // Extract FEN if not a new game
        // TODO: not making the server crash when someone sends an illegal FEN
        let fen: Fen = if !is_new_game {
            let fen_regex = Regex::new(r"FEN:\s*(.+)").unwrap();
            let fen = fen_regex
                .captures(&body_content)
                .and_then(|cap| cap.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or("FEN not found");
            Fen::from_str(&fen).expect("invalid FEN")
        } else {
            // It's a new game
            let fen_string =
                String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
            Fen::from_str(&fen_string).expect("invalid FEN")
        };

        (chess_move_string.to_string(), fen, is_new_game)
    }

    fn get_risc0_inputs(
        &self,
        game_id: &str,
        actual_move: &str,
        actual_fen: &str,
    ) -> Option<String> {
        let prev_game_email = self.db.get_email(game_id).unwrap()?;
        let parsed_mail = parse_mail(&prev_game_email).expect("could not parse prev email");
        let body_part = parsed_mail.subparts.first().unwrap();
        let body_content = body_part.get_body().unwrap();
        let (prev_move, prev_fen, _) = self.parse_mail_body(&body_content);

        let inputs = (actual_move, actual_fen, &prev_move, &prev_fen.to_string());

        Some(serde_json::to_string(&inputs).unwrap())
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
            let (opponent_id, opponent_addr) = self.extract_cc_addr(&parsed_email);

            let game_id = self
                .db
                .get_game_id(&from_addr, &opponent_addr)
                .expect("couldnt get game id");

            let (chess_move_string, fen, is_new_game) = self.parse_mail_body(&body_content);

            let mut game: Game = if (is_new_game) {
                let game = Game::new(&from_addr, &opponent_addr);
                // Use the starting position FEN for a new game
                self.db.store_game(&game_id, &game);
                println!(
                    "new game FEN: {}",
                    Fen::from_position(game.position.chess.clone(), EnPassantMode::Legal)
                );

                game
            } else {
                self.db.get_game(&game_id).unwrap()?
            };

            let fen_string = fen.to_string();
            println!("MOVE: {}", chess_move_string);
            println!("FEN: {}", fen_string);

            // TODO: not making the server crash when someone sends an illegal move
            let position = fen.into_position::<Chess>(CastlingMode::Standard).unwrap();
            let chess_move: Move = chess_move_string
                .parse::<San>()
                .unwrap()
                .to_move(&position)
                .unwrap();

            let position = position.play(&chess_move).unwrap();
            game.update(&position);
            self.db.store_game(&game_id, &game);

            // only prove the move when it's mate
            if position.is_checkmate() {
                let serialized_args =
                    self.get_risc0_inputs(&game_id, &chess_move_string, &fen_string)?;

                self.db.delete_game(&game_id);
                if let Some(email_data) = self.db.get_email(&game_id).ok()? {
                    // Do something with the email data, like saving it to a file
                    std::fs::write("game123.eml", email_data).ok()?;
                }
                Some((null_state, identiy, serialized_args))
            } else {
                let setup: Setup = position.into_setup(EnPassantMode::Legal);
                let fen: Fen = setup.into();
                self.send_move_by_mail(
                    &self.ref_mail,
                    &from_addr,
                    &from_id,
                    &opponent_addr,
                    &opponent_id,
                    &fen.to_string(),
                    &chess_move_string,
                )
                .expect(format!("could not send mail to {}", from_addr).as_str());
                self.db.store_email(&game_id, body);

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
