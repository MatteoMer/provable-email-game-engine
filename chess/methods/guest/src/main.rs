// Attribution:
// LastPosition impl for Visitor is taken from the pgn_reader docs
// https://docs.rs/pgn-reader/latest/pgn_reader/

use hyle_contract::{HyleInput, HyleOutput};
use pgn_reader::{BufferedReader, RawHeader, SanPlus, Skip, Visitor};
use risc0_zkvm::guest::env;
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Position};

struct LastPosition {
    pos: Chess,
}

impl LastPosition {
    fn new() -> LastPosition {
        LastPosition {
            pos: Chess::default(),
        }
    }
}

impl Visitor for LastPosition {
    type Result = Chess;

    fn header(&mut self, key: &[u8], value: RawHeader<'_>) {
        // Support games from a non-standard starting position.
        if key == b"FEN" {
            let pos = Fen::from_ascii(value.as_bytes())
                .ok()
                .and_then(|f| f.into_position(CastlingMode::Standard).ok());

            if let Some(pos) = pos {
                self.pos = pos;
            }
        }
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true) // stay in the mainline
    }

    fn san(&mut self, san_plus: SanPlus) {
        if let Ok(m) = san_plus.san.to_move(&self.pos) {
            self.pos.play_unchecked(&m);
        }
    }

    fn end_game(&mut self) -> Self::Result {
        ::std::mem::replace(&mut self.pos, Chess::default())
    }
}

// is it bad if i dont commit the hyle struct in the guest?
fn main() {
    // read the pgn
    let input: HyleInput<String> = env::read();
    let pgn: String = input.program_inputs;

    let mut reader = BufferedReader::new_cursor(&pgn[..]);

    let mut visitor = LastPosition::new();
    let pos = reader
        .read_game(&mut visitor)
        .unwrap()
        .expect("invalid game");

    let is_checkmate: bool = pos.is_checkmate();

    let null_state = 0u32.to_be_bytes().to_vec();

    env::commit(&HyleOutput {
        version: 1,
        index: 0,
        identity: "".to_string(),
        tx_hash: input.tx_hash,
        program_outputs: is_checkmate,
        payloads: pgn.clone().as_bytes().to_vec(),
        success: is_checkmate,
        initial_state: input.initial_state,
        next_state: null_state.clone(),
    });
}
