use hyle_contract::{HyleInput, HyleOutput};
use risc0_zkvm::guest::env;
use shakmaty::{fen::Fen, san::San, CastlingMode, Chess, EnPassantMode, Move, Position};

fn main() {
    let input: HyleInput<String> = env::read();
    let serialized_inputs: String = input.program_inputs;

    // claimed_move: move claimed to be mate
    // claimed_board: board on which the mate happens
    // prev_move: move that lead to the actual board
    // prev_fen: board on which the prev_move has been played
    let (claimed_move, claimed_board, prev_move, prev_board): (&str, &str, &str, &str) =
        serde_json::from_str(serialized_inputs.as_ref()).unwrap();

    println!("[RISC0] Proving with inputs: claimed_move: {}, claimed_board:{}, prev_move: {}, prev_board: {}", claimed_move, claimed_board, prev_move, prev_board);
    let null_state = 0u32.to_be_bytes().to_vec();

    // Check if prev_board.play(prev_move) == claimed_board
    let prev_fen = Fen::from_ascii(prev_board.as_bytes()).unwrap();
    let prev_position = prev_fen
        .into_position::<Chess>(CastlingMode::Standard)
        .unwrap();
    let prev_move: Move = prev_move
        .parse::<San>()
        .unwrap()
        .to_move(&prev_position)
        .unwrap();
    let expected_pos = prev_position.play(&prev_move).unwrap();
    let expected_fen = Fen::from_position(expected_pos, EnPassantMode::Legal);

    let fen = Fen::from_ascii(claimed_board.as_bytes()).unwrap();

    if fen != expected_fen {
        panic!("fen are not matching");
    }
    println!("[RISC0] claimed_fen == expected_fen");

    // Check if claimed_board.play(claimed_move).is_checkmate() == true
    let position = fen.into_position::<Chess>(CastlingMode::Standard).unwrap();
    let chess_move: Move = claimed_move
        .parse::<San>()
        .unwrap()
        .to_move(&position)
        .unwrap();

    let pos = position.play(&chess_move).unwrap();
    let is_checkmate: bool = pos.is_checkmate();

    if !is_checkmate {
        panic!("move is not mate");
    }
    println!("[RISC0] Success");

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
