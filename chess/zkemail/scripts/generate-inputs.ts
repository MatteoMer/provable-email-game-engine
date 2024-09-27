import { bytesToBigInt, fromHex } from "@zk-email/helpers/dist/binary-format";
import { generateEmailVerifierInputs } from "@zk-email/helpers/dist/input-generators";
import { program } from "commander";
import fs from "fs";
import path from "path";
const snarkjs = require("snarkjs");

export const STRING_PRESELECTOR = "";

const MAX_BYTES_IN_FIELD = 31; // Adjust this if your Circom constant is different

function packBytes(bytes: number[]): bigint[] {
    const packed: bigint[] = [];
    for (let i = 0; i < bytes.length; i += MAX_BYTES_IN_FIELD) {
        let chunk = 0n;
        for (let j = 0; j < MAX_BYTES_IN_FIELD && i + j < bytes.length; j++) {
            chunk += BigInt(bytes[i + j]) << BigInt(8 * j);
        }
        packed.push(chunk);
    }
    return packed;
}

export type IChessCircuitInputs = {
    moveIndex: string;
    fenIndex: string;
    emailHeader: string[];
    emailHeaderLength: string;
    pubkey: string[];
    signature: string[];
    emailBody?: string[] | undefined;
    emailBodyLength?: string | undefined;
    precomputedSHA?: string[] | undefined;
    bodyHashIndex?: string | undefined;
    expectedMove: string[];
    expectedFen?: string[];
};

export async function generateChessVerifierCircuitInputs(
    email: string | Buffer,
): Promise<IChessCircuitInputs> {
    const emailVerifierInputs = await generateEmailVerifierInputs(email, {
        shaPrecomputeSelector: STRING_PRESELECTOR,
    });

    const bodyRemaining = emailVerifierInputs.emailBody!.map((c) => Number(c)); // Char array to Uint8Array
    const bodyBuffer = Buffer.from(bodyRemaining);

    const moveSelectorBuffer = Buffer.from("MOVE: ");
    const fenSelectorBuffer = Buffer.from("FEN: ");

    const moveIndex = bodyBuffer.indexOf(moveSelectorBuffer) + moveSelectorBuffer.length;
    const fenIndex = bodyBuffer.indexOf(fenSelectorBuffer) + fenSelectorBuffer.length;

    // Extract and pack move
    const moveEndIndex = bodyBuffer.indexOf('\n', moveIndex);
    const moveString = bodyBuffer.slice(moveIndex, moveEndIndex).toString().trim();
    const moveBytes = Array.from(Buffer.from(moveString, 'utf8'));
    const packedMove = packBytes(moveBytes);

    // Extract and pack FEN
    const fenEndIndex = bodyBuffer.indexOf('\n', fenIndex);
    const fenString = bodyBuffer.slice(fenIndex, fenEndIndex).toString().trim();
    const fenBytes = Array.from(Buffer.from(fenString, 'utf8'));
    const packedFen = packBytes(fenBytes);

    return {
        ...emailVerifierInputs,
        fenIndex: fenIndex.toString(),
        moveIndex: moveIndex.toString(),
        expectedMove: packedMove.map(bn => bn.toString()),
        expectedFen: packedFen.map(bn => bn.toString()),
    };
}


program
    .requiredOption("--email-file <string>", "Path to email file")
    .option("--silent", "No console logs");

program.parse();
const args = program.opts();

const CIRCUIT_NAME = "chess";
const BUILD_DIR = path.join(__dirname, "../build");
const OUTPUT_DIR = path.join(__dirname, "../proofs");

function log(...message: any) {
    if (!args.silent) {
        console.log(...message);
    }
}
const logger = { log, error: log, warn: log, debug: log };

async function generate() {
    if (!fs.existsSync(OUTPUT_DIR)) {
        fs.mkdirSync(OUTPUT_DIR);
    }

    if (!fs.existsSync(args.emailFile)) {
        throw new Error("--input file path arg must end with .json");
    }

    log("Generating input and proof for:", args.emailFile);

    const rawEmail = Buffer.from(fs.readFileSync(args.emailFile, "utf8"));
    const circuitInputs = await generateChessVerifierCircuitInputs(rawEmail);

    log("\n\nGenerated Inputs:", circuitInputs, "\n\n");

    fs.writeFileSync(
        path.join(OUTPUT_DIR, "input.json"),
        JSON.stringify(circuitInputs, null, 2)
    );
    log("Inputs written to", path.join(OUTPUT_DIR, "input.json"));

    // Generate witness
    const wasm = fs.readFileSync(
        path.join(BUILD_DIR, `${CIRCUIT_NAME}_js/${CIRCUIT_NAME}.wasm`)
    );
    const wc = require(path.join(
        BUILD_DIR,
        `${CIRCUIT_NAME}_js/witness_calculator.js`
    ));
    const witnessCalculator = await wc(wasm);
    const buff = await witnessCalculator.calculateWTNSBin(circuitInputs, 0);
    fs.writeFileSync(path.join(OUTPUT_DIR, `input.wtns`), buff);

    // Generate proof
    const { proof, publicSignals } = await snarkjs.groth16.prove(
        path.join(BUILD_DIR, `${CIRCUIT_NAME}.zkey`),
        path.join(OUTPUT_DIR, `input.wtns`),
        logger
    );

    fs.writeFileSync(
        path.join(OUTPUT_DIR, "proof.json"),
        JSON.stringify(proof, null, 2)
    );
    log("Proof written to", path.join(OUTPUT_DIR, "proof.json"));

    fs.writeFileSync(
        path.join(OUTPUT_DIR, "public.json"),
        JSON.stringify(publicSignals, null, 2)
    );
    log("Public Inputs written to", path.join(OUTPUT_DIR, "public.json"));

    const vkey = JSON.parse(fs.readFileSync(path.join(BUILD_DIR, `/artifacts/circuit_vk.json`)).toString());
    const proofVerified = await snarkjs.groth16.verify(
        vkey,
        publicSignals,
        proof
    );
    if (proofVerified) {
        console.log("Proof Verified");
    } else {
        throw new Error("Proof Verification Failed");
    }

    process.exit(0);
}

generate().catch((err) => {
    console.error("Error generating proof", err);
    process.exit(1);
});
