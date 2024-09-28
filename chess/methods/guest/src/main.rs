use hyle_contract::{HyleInput, HyleOutput};
use risc0_zkvm::guest::env;
use shakmaty::{fen::Fen, san::San, CastlingMode, Chess, Move, Position};

fn main() {
    let input: HyleInput<String> = env::read();
    let serialized_inputs: String = input.program_inputs;

    let (chess_move_string, fen_string): (&str, &str) =
        serde_json::from_str(serialized_inputs.as_ref()).unwrap();
    let null_state = 0u32.to_be_bytes().to_vec();

    let fen = Fen::from_ascii(fen_string.as_bytes());
    // invalid fen in the mail
    // TODO: send mail about the invalid move to the player
    let fen = fen.unwrap();

    // TODO: not making the server crash when someone sends an illegal move
    let position = fen.into_position::<Chess>(CastlingMode::Standard).unwrap();
    let chess_move: Move = chess_move_string
        .parse::<San>()
        .unwrap()
        .to_move(&position)
        .unwrap();

    let pos = position.play(&chess_move).unwrap();
    let is_checkmate: bool = pos.is_checkmate();

    env::commit(&HyleOutput {
        version: 1,
        index: 0,
        identity: "".to_string(),
        tx_hash: input.tx_hash,
        program_outputs: is_checkmate,
        payloads: serialized_inputs.clone().as_bytes().to_vec(),
        success: is_checkmate,
        initial_state: input.initial_state,
        next_state: null_state.clone(),
    });
}
