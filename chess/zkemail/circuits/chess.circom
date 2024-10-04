pragma circom 2.1.5;

include "@zk-email/circuits/email-verifier.circom";
include "@zk-email/zk-regex-circom/circuits/common/from_addr_regex.circom";
include "@zk-email/circuits/utils/regex.circom";


template ChessVerifier (maxHeadersLength, maxBodyLength, n, k, maxFenLength, maxMoveLength) {
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
    signal input expectedMove[maxMoveLength];
    signal input expectedMoveLength;
    signal input expectedFen[maxFenLength];
    signal input expectedFenLength;

    signal output pubkeyHash;

    // INPUT VALIDATION
    signal validFenLength;
    validFenLength <-- (expectedFenLength > 0) && (expectedFenLength <= maxFenLength);
    validFenLength === 1;

    signal validMoveLength;
    validMoveLength <-- (expectedMoveLength > 0) && (expectedMoveLength <= maxMoveLength);
    validMoveLength === 1;


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


    // FROM HEADER REGEX
    signal input fromEmailIndex;

    // Assert fromEmailIndex < emailHeaderLength
    signal isFromIndexValid <== LessThan(log2Ceil(maxHeadersLength))([fromEmailIndex, emailHeaderLength]);
    isFromIndexValid === 1;

    signal (fromEmailFound, fromEmailReveal[maxHeadersLength]) <== FromAddrRegex(maxHeadersLength)(emailHeader);
    fromEmailFound === 1;

    var maxEmailLength = 255;

    signal output fromEmailAddrPacks[9] <== PackRegexReveal(maxHeadersLength, maxEmailLength)(fromEmailReveal, fromEmailIndex);

    signal inRange[maxFenLength];
    signal bodyChar[maxFenLength];
    signal charEqual[maxFenLength];

    for (var i = 0; i < maxFenLength; i++) {
        inRange[i] <-- i < expectedFenLength ? 1 : 0;
        bodyChar[i] <-- emailBody[fenIndex + i];
        charEqual[i] <-- (bodyChar[i] - expectedFen[i]) * inRange[i];
        charEqual[i] === 0;
    }

    signal moveInRange[maxMoveLength];
    signal moveBodyChar[maxMoveLength];
    signal moveCharEqual[maxMoveLength];
    for (var i = 0; i < maxMoveLength; i++) {
        moveInRange[i] <-- i < expectedMoveLength ? 1 : 0;
        moveBodyChar[i] <-- emailBody[moveIndex + i];
        moveCharEqual[i] <-- (moveBodyChar[i] - expectedMove[i]) * moveInRange[i];
        moveCharEqual[i] === 0;
    }


}

component main { public [ fenIndex, moveIndex, expectedFen, expectedFenLength, expectedMove, expectedMoveLength ] } = ChessVerifier(1024, 1536, 121, 17, 90, 10);
