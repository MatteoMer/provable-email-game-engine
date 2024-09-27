use async_imap::types::Fetch;
use hyle_contract::{HyleInput, HyleOutput};
use methods::{CHESS_GUEST_ELF, CHESS_GUEST_ID};
use referee::hyle::HyleNetwork;
use referee::server::ServerConfig;
use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};

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
    fn process_email(&self, input: &Fetch) -> Option<(Vec<u8>, String, String)> {
        //TODO: process email instead of hardcoding PGN
        let pgn: String = "1. d4 e6 2. e3 d5 3. a3 c5 4. c4 cxd4 5. Qxd4 Nc6 6. Qc3 Nf6 7. Nd2 Bd6 8. f4 O-O 9. cxd5 Nxd5 10. Qc2 Nxe3 11. Qd3 Bxf4 12. Qxd8 Rxd8 13. Ngf3 Nxf1 14. Rxf1 Bxd2+ 15. Bxd2 b6 16. Rd1 Bb7 17. Ng5 f6 18. Nxe6 Re8 19. Kf2 Rxe6 20. Bc3 Ne5 21. Bxe5 Rxe5 22. Rd7 Bd5 23. Rc1 Rae8 24. Rcc7 Re2+ 25. Kg3 R8e3+ 26. Kf4 Re4+ 27. Kf5 Be6# 0-1".to_string();

        let null_state = 0u32.to_be_bytes().to_vec();
        let identiy = "".to_string();
        Some((null_state, identiy, pgn.clone()))
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
