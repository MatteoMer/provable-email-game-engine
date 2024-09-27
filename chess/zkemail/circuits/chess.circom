pragma circom 2.1.5;

include "@zk-email/zk-regex-circom/circuits/common/from_addr_regex.circom";
include "@zk-email/circuits/email-verifier.circom";
include "@zk-email/circuits/utils/regex.circom";
include "chess-fen-regex.circom";
// include "https://github.com/0xPARC/circom-secp256k1/blob/master/circuits/bigint.circom";


template ChessVerifier (maxHeadersLength, maxBodyLength, n, k) {
    signal input emailHeader[maxHeadersLength];
    signal input emailHeaderLength;
    signal input pubkey[k];
    signal input signature[k];
    signal input emailBody[maxBodyLength];
    signal input emailBodyLength;
    signal input bodyHashIndex;
    signal input precomputedSHA[32];
    signal input fenIndex;
    signal input moveIndex;
    signal input expectedMove;
    signal input expectedFen[3];

    signal output pubkeyHash;

    component EV = EmailVerifier(maxHeadersLength, maxBodyLength, n, k, 0,0,0,0);

    EV.emailHeader <== emailHeader;
    EV.pubkey <== pubkey;
    EV.signature <== signature;
    EV.emailHeaderLength <== emailHeaderLength;
    EV.bodyHashIndex <== bodyHashIndex;
    EV.precomputedSHA <== precomputedSHA;
    EV.emailBody <== emailBody;
    EV.emailBodyLength <== emailBodyLength;

    pubkeyHash <== EV.pubkeyHash;

    // // FROM HEADER REGEX
    // signal input fromEmailIndex;

    // // Assert fromEmailIndex < emailHeaderLength
    // signal isFromIndexValid <== LessThan(log2Ceil(maxHeadersLength))([fromEmailIndex, emailHeaderLength]);
    // isFromIndexValid === 1;

    // signal (fromEmailFound, fromEmailReveal[maxHeadersLength]) <== FromAddrRegex(maxHeadersLength)(emailHeader);
    // fromEmailFound === 1;

    // var maxEmailLength = 255;

    // signal output fromEmailAddrPacks[9] <== PackRegexReveal(maxHeadersLength, maxEmailLength)(fromEmailReveal, fromEmailIndex);

    // CHESS FEN AND MOVE REGEX
    signal (found, revealMove[maxBodyLength], revealFen[maxBodyLength]) <== ChessFenRegex(maxBodyLength)(emailBody);
    found === 1;

    var maxMoveLen = 10; // super rare but i think it could be 10
    signal movePacks[1] <== PackRegexReveal(maxBodyLength, maxMoveLen)(revealMove, moveIndex);

    var maxFenLen = 90; 
    signal fenPacks[3] <== PackRegexReveal(maxBodyLength, maxFenLen)(revealFen, fenIndex);

    expectedMove === movePacks[0];
    expectedFen[0] === fenPacks[0];
    expectedFen[1] === fenPacks[1];
    expectedFen[2] === fenPacks[2];


}

component main { public [ fenIndex, moveIndex ] } = ChessVerifier(1024, 1536, 121, 17);
