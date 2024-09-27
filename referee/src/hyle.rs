use std::env;
use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use hyle_contract::HyleInput;
use risc0_zkvm::Receipt;

#[derive(Clone, Copy)]
pub enum HyleNetwork {
    Localhost,
    Devnet,
}

pub struct Hyle {
    network: HyleNetwork,
    hyled_path: PathBuf,
}

impl Hyle {
    pub fn new(network: HyleNetwork, hyled_path: &Path) -> Self {
        Self {
            network,
            hyled_path: hyled_path.to_path_buf(),
        }
    }

    pub fn publish_payload(
        &self,
        identity: &str,
        contract_name: &str,
        payload: &str,
        initial_state: &[u8],
        program_inputs: &str,
    ) -> Result<HyleInput<String>, Box<dyn Error>> {
        let args = vec!["tx", "zktx", "publish", "", contract_name, payload];

        println!(
            "Executing command: {} {}",
            self.hyled_path.clone().display(),
            args.join(" ")
        );

        let cur_dir = env::current_dir().unwrap();
        env::set_current_dir(self.hyled_path.clone());

        let mut hyled = Command::new(self.hyled_path.clone())
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn hyled process");

        let mut stdin = hyled.stdin.take().expect("Failed to open stdin");
        std::thread::spawn(move || {
            stdin
                .write_all("y".as_bytes())
                .expect("Failed to write to stdin");
        });

        let output = hyled.wait_with_output().expect("Failed to read stdout");

        env::set_current_dir(cur_dir);
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", stdout);
            // Extract txhash from the output
            if let Some(txhash_line) = stdout.lines().find(|line| line.starts_with("txhash: ")) {
                if let Some(txhash) = txhash_line.strip_prefix("txhash: ") {
                    Ok(HyleInput {
                        initial_state: initial_state.to_vec(),
                        identity: identity.to_string(),
                        tx_hash: txhash.into(),
                        program_inputs: program_inputs.to_string(),
                    })
                } else {
                    Err("Failed to extract txhash from the line".into())
                }
            } else {
                Err("Txhash not found in the output".into())
            }
        } else {
            panic!("Could not publish payload");
        }
    }

    pub fn broadcast_proof(
        &self,
        txhash: &str,
        contract_name: &str,
        payload_index: &str,
        path: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let path_str = path.as_os_str().to_str().unwrap();

        let args = vec![
            "tx",
            "zktx",
            "prove",
            txhash,
            payload_index,
            contract_name,
            path_str,
        ];

        println!(
            "Executing command: {} {}",
            self.hyled_path.clone().display(),
            args.join(" ")
        );

        let mut hyled = Command::new(self.hyled_path.clone())
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn hyled process");

        let mut stdin = hyled.stdin.take().expect("Failed to open stdin");
        std::thread::spawn(move || {
            stdin
                .write_all("y".as_bytes())
                .expect("Failed to write to stdin");
        });

        let output = hyled.wait_with_output().expect("Failed to read stdout");

        if output.status.success() {
            Ok(())
        } else {
            panic!("Could not broadcast proof");
        }
    }

    // no creation of smart contract in the lib, i feel like it's better to do it manually once
    // than accidentally creating 1000s
}
