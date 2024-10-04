import { bytesToBigInt, fromHex } from "@zk-email/helpers/dist/binary-format";
import { generateEmailVerifierInputs } from "@zk-email/helpers/dist/input-generators";
import { program } from "commander";
import fs from "fs";
import path from "path";
const snarkjs = require("snarkjs");

export const STRING_PRESELECTOR = "";
const MAX_FEN_LENGTH = 90; // This should match the maxFenLength in your Circom circuit
const MAX_MOVE_LENGTH = 10;
const MAX_BYTES_IN_FIELD = 31; // Adjust this if your Circom constant is different

function padAndConvertToDecimal(bytes: number[], maxLength: number): string[] {
    const padded = bytes.slice(0, maxLength);
    while (padded.length < maxLength) {
        padded.push(0);
    }
    return padded.map(byte => byte.toString());
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
    expectedMoveLength: string;
    expectedFen?: string[];
    expectedFenLength: string;

};

export async function generateChessVerifierCircuitInputs(
    email: string | Buffer,
): Promise<IChessCircuitInputs> {
    const emailVerifierInputs = await generateEmailVerifierInputs(email, {
        shaPrecomputeSelector: STRING_PRESELECTOR,
    });

    const bodyRemaining = emailVerifierInputs.emailBody!.map((c) => Number(c));
    const bodyBuffer = Buffer.from(bodyRemaining);

    const moveSelectorBuffer = Buffer.from("MOVE: ");
    const fenSelectorBuffer = Buffer.from("FEN: ");

    const moveIndex = bodyBuffer.indexOf(moveSelectorBuffer) + moveSelectorBuffer.length;
    const fenIndex = bodyBuffer.indexOf(fenSelectorBuffer) + fenSelectorBuffer.length;

    // Extract move
    const moveEndIndex = bodyBuffer.indexOf('\n', moveIndex);
    const moveString = bodyBuffer.slice(moveIndex, moveEndIndex).toString().trim();
    const moveBytes = Array.from(Buffer.from(moveString, 'utf8'));
    const paddedMove = padAndConvertToDecimal(moveBytes, MAX_MOVE_LENGTH);

    // Extract FEN
    const fenEndIndex = bodyBuffer.indexOf('\n', fenIndex);
    const fenString = bodyBuffer.slice(fenIndex, fenEndIndex).toString().trim();
    const fenBytes = Array.from(Buffer.from(fenString, 'utf8'));
    const paddedFen = padAndConvertToDecimal(fenBytes, MAX_FEN_LENGTH);

    return {
        ...emailVerifierInputs,
        fenIndex: fenIndex.toString(),
        moveIndex: moveIndex.toString(),
        expectedMove: paddedMove,
        expectedMoveLength: moveBytes.length.toString(),
        expectedFen: paddedFen,
        expectedFenLength: fenBytes.length.toString(),
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
        path.join(BUILD_DIR, `${CIRCUIT_NAME}.vkey`),
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
    /*
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
        */

    process.exit(0);
}

generate().catch((err) => {
    console.error("Error generating proof", err);
    process.exit(1);
});
