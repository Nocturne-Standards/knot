// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {DuskBls} from "./DuskBls.sol";

interface ICrossDomainMessenger {
    function xDomainMessageSender() external view returns (address);
}

/// Knot members roots on an EVM chain, one per knot-registry account, kept current by each account's quorum.
/// No keys are stored.
///
/// Per account: members root, threshold, rotation nonce, consumed action marks.
///
/// Members root: leaf_i = keccak256(pk_i || pkEip_i), root = keccak256(leaf_0 || ... || leaf_{n-1}), where pk_i is
/// the 96-byte compressed Dusk G2 key and pkEip_i its 256-byte EIP-2537 form. The contract only installs roots it
/// computed itself from compressed keys it decoded (flags, on-curve, subgroup), so pkEip_i is bound to pk_i.
///
/// Bootstrap (D9): the first member set of an account arrives once, through the L2 CrossDomainMessenger, from the
/// knot-registry contract on DuskDS (pinned: native sender = keccak256(registryId)[12:]). The registry checks the
/// account's quorum before it sends. Same receiver checks as nocturne-lending NavReceiver. After bootstrap only
/// rotate changes membership.
///
/// Quorum check (Dusk abi::verify_bls_multisig, weighted): the caller supplies a Quorum: all leaves, a signer bitmap
/// over leaf indices, and each signer's (pk, pkEip). h1 is derived here from the compressed pk; nothing is pre-weighted.
///
/// Replay policy: rotations carry rotationNonce + 1 and are applied at most once. Action signatures bind the current
/// rotationNonce and the consuming caller; checkAction consumes each action digest at most once per rotation epoch.
/// A rotation invalidates every earlier action signature; marks from older epochs are ignored and their slot reused.
/// Message format: spec knot-rotation-message.
contract KnotEvmRoot {
    bytes32 public constant ROTATE_TAG = "KNOT_EVM_ROOT_ROTATE_V1";
    bytes32 public constant ACTION_TAG = "KNOT_EVM_ROOT_ACTION_V1";
    uint256 public constant MAX_MEMBERS = 16;

    /// knot-registry ContractId on DuskDS
    bytes32 public immutable registryId;
    uint64 public immutable duskChainId;
    /// L2 CrossDomainMessenger (DuskEVM: 0x4200000000000000000000000000000000000007)
    ICrossDomainMessenger public immutable messenger;
    /// xDomainMessageSender() of messages sent by knot-registry: keccak256(registryId)[12:]
    address public immutable trustedNativeSender;

    struct Account {
        bytes32 membersRoot;
        uint32 threshold;
        uint64 rotationNonce;
    }

    /// Signatures of a quorum of the stored root. leaves: all member leaves in order; signerBitmap: signers by leaf
    /// index; signerPks / signerPksEip: each signer's 96-byte compressed key / 256-byte EIP-2537 key, in bitmap
    /// order; sig: 48-byte compressed aggregate signature.
    struct Quorum {
        bytes32[] leaves;
        uint256 signerBitmap;
        bytes signerPks;
        bytes signerPksEip;
        bytes sig;
    }

    mapping(uint64 => Account) internal accounts;
    /// account => digest => rotationNonce + 1 of the epoch that consumed it (0: never)
    mapping(uint64 => mapping(bytes32 => uint64)) internal consumedEpoch;

    event Rotated(uint64 indexed accountId, uint64 indexed nonce, bytes32 membersRoot, uint32 threshold, bytes pks);
    event ActionConsumed(
        uint64 indexed accountId, uint64 indexed nonce, address indexed consumer, bytes32 actionDigest
    );

    error ZeroAddress();
    error NotCrossDomainMessenger(address caller, address expected);
    error UntrustedNativeSender(address sender, address expected);
    error AlreadyBootstrapped();
    error WrongDuskChain(uint64 got, uint64 expected);
    error NotBootstrapped();
    error BadMembers();
    error BadThreshold();
    error BadNonce();
    error RootMismatch();
    error BadBitmap();
    error BelowThreshold();
    error NotMember(uint256 index);
    error BadSignature();
    error ActionReplayed();

    constructor(bytes32 registryId_, uint64 duskChainId_, address messenger_) {
        if (messenger_ == address(0)) revert ZeroAddress();
        registryId = registryId_;
        duskChainId = duskChainId_;
        messenger = ICrossDomainMessenger(messenger_);
        trustedNativeSender = address(uint160(uint256(keccak256(abi.encodePacked(registryId_)))));
    }

    // ------------------------------------------------------------------ external

    /// One-shot install of an account's first member set. Only from the messenger, only from knot-registry.
    /// duskChainId_ must equal the immutable chain id baked into this root.
    /// pks: concatenated 96-byte compressed keys, no duplicates, at most MAX_MEMBERS.
    function bootstrap(uint64 accountId, uint64 duskChainId_, bytes calldata pks, uint32 threshold_) external {
        if (msg.sender != address(messenger)) revert NotCrossDomainMessenger(msg.sender, address(messenger));
        address native = messenger.xDomainMessageSender();
        if (native != trustedNativeSender) revert UntrustedNativeSender(native, trustedNativeSender);
        if (duskChainId_ != duskChainId) revert WrongDuskChain(duskChainId_, duskChainId);
        Account storage a = accounts[accountId];
        if (a.membersRoot != bytes32(0)) revert AlreadyBootstrapped();
        (bytes32 root, uint256 n) = _rootOf(pks);
        if (threshold_ == 0 || threshold_ > n) revert BadThreshold();
        a.membersRoot = root;
        a.threshold = threshold_;
        emit Rotated(accountId, 0, root, threshold_, pks);
    }

    /// Replace the member set. Signed by the current quorum over rotationMessage(accountId, newRoot, newThreshold,
    /// nonce). newPks: concatenated 96-byte compressed keys of the new set, no duplicates, at most MAX_MEMBERS.
    function rotate(uint64 accountId, bytes calldata newPks, uint32 newThreshold, uint64 nonce, Quorum calldata q)
        external
    {
        Account storage a = _account(accountId);
        if (nonce != a.rotationNonce + 1) revert BadNonce();
        (bytes32 newRoot, uint256 n) = _rootOf(newPks);
        if (newThreshold == 0 || newThreshold > n) revert BadThreshold();
        bytes memory m = rotationMessage(accountId, newRoot, newThreshold, nonce);
        if (!_verifyQuorum(accountId, m, q)) revert BadSignature();
        a.membersRoot = newRoot;
        a.threshold = newThreshold;
        a.rotationNonce = nonce;
        emit Rotated(accountId, nonce, newRoot, newThreshold, newPks);
    }

    /// Consume an action: a quorum of the account's stored root signed actionMessage(accountId, msg.sender,
    /// actionDigest). Each digest is accepted at most once per rotation epoch. Reverts on any failure; returning
    /// means authorized and consumed.
    function checkAction(uint64 accountId, bytes32 actionDigest, Quorum calldata q) external {
        uint64 nonce = _account(accountId).rotationNonce;
        if (consumedEpoch[accountId][actionDigest] == nonce + 1) revert ActionReplayed();
        bytes memory m = actionMessage(accountId, msg.sender, actionDigest);
        if (!_verifyQuorum(accountId, m, q)) revert BadSignature();
        consumedEpoch[accountId][actionDigest] = nonce + 1;
        emit ActionConsumed(accountId, nonce, msg.sender, actionDigest);
    }

    function isBootstrapped(uint64 accountId) external view returns (bool) {
        return accounts[accountId].membersRoot != bytes32(0);
    }

    function membersRoot(uint64 accountId) external view returns (bytes32) {
        return accounts[accountId].membersRoot;
    }

    function threshold(uint64 accountId) external view returns (uint32) {
        return accounts[accountId].threshold;
    }

    function rotationNonce(uint64 accountId) external view returns (uint64) {
        return accounts[accountId].rotationNonce;
    }

    /// True iff actionDigest was consumed in the account's current rotation epoch.
    function isConsumed(uint64 accountId, bytes32 actionDigest) external view returns (bool) {
        return consumedEpoch[accountId][actionDigest] == accounts[accountId].rotationNonce + 1;
    }

    function rotationMessage(uint64 accountId, bytes32 newRoot, uint32 newThreshold, uint64 nonce)
        public
        view
        returns (bytes memory)
    {
        return abi.encode(
            ROTATE_TAG,
            uint256(duskChainId),
            block.chainid,
            address(this),
            registryId,
            uint256(accountId),
            newRoot,
            uint256(newThreshold),
            uint256(nonce)
        );
    }

    function actionMessage(uint64 accountId, address consumer, bytes32 actionDigest)
        public
        view
        returns (bytes memory)
    {
        return abi.encode(
            ACTION_TAG,
            uint256(duskChainId),
            block.chainid,
            address(this),
            registryId,
            uint256(accountId),
            uint256(accounts[accountId].rotationNonce),
            consumer,
            actionDigest
        );
    }

    // ------------------------------------------------------------------ internal

    function _account(uint64 accountId) internal view returns (Account storage a) {
        a = accounts[accountId];
        if (a.membersRoot == bytes32(0)) revert NotBootstrapped();
    }

    /// Decode every key (subgroup-checked), reject identity and duplicates, return the root over (pk, pkEip) leaves.
    function _rootOf(bytes memory pks) internal view returns (bytes32 root, uint256 n) {
        n = pks.length / 96;
        if (n == 0 || n > MAX_MEMBERS || pks.length != n * 96) revert BadMembers();
        bytes32[] memory leaves = new bytes32[](n);
        for (uint256 i = 0; i < n; i++) {
            bytes memory pk = new bytes(96);
            assembly {
                let s := add(add(pks, 32), mul(i, 96))
                let d := add(pk, 32)
                mstore(d, mload(s))
                mstore(add(d, 32), mload(add(s, 32)))
                mstore(add(d, 64), mload(add(s, 64)))
            }
            (bool ok, bool inf, bytes memory eip) = DuskBls.decompressG2(pk);
            if (!ok || inf) revert BadMembers();
            bytes32 leaf = keccak256(abi.encodePacked(pk, eip));
            for (uint256 j = 0; j < i; j++) {
                if (leaves[j] == leaf) revert BadMembers();
            }
            leaves[i] = leaf;
        }
        root = keccak256(abi.encodePacked(leaves));
    }

    /// Weighted Dusk multisig over m by the signers in signerBitmap, against the account's stored root and threshold.
    /// Reverts on structural errors (root, bitmap, membership, threshold); returns false on a bad signature.
    function _verifyQuorum(uint64 accountId, bytes memory m, Quorum calldata q) internal view returns (bool) {
        bytes32[] calldata leaves = q.leaves;
        uint256 signerBitmap = q.signerBitmap;
        bytes calldata signerPks = q.signerPks;
        bytes calldata signerPksEip = q.signerPksEip;
        Account storage a = _account(accountId);
        if (keccak256(abi.encodePacked(leaves)) != a.membersRoot) revert RootMismatch();
        uint256 n = leaves.length;
        uint256 s = signerPks.length / 96;
        if (signerPks.length != s * 96 || signerPksEip.length != s * 256) revert BadBitmap();
        if (signerBitmap == 0 || (n < 256 && signerBitmap >> n != 0)) revert BadBitmap();
        if (s < a.threshold) revert BelowThreshold();

        uint256[] memory scalars = new uint256[](s);
        uint256 k = 0;
        for (uint256 i = 0; i < n; i++) {
            if ((signerBitmap >> i) & 1 == 0) continue;
            if (k == s) revert BadBitmap();
            bytes memory pk = signerPks[k * 96:(k + 1) * 96];
            if (keccak256(abi.encodePacked(pk, signerPksEip[k * 256:(k + 1) * 256])) != leaves[i]) revert NotMember(i);
            scalars[k] = DuskBls.h1(pk);
            k++;
        }
        if (k != s) revert BadBitmap();

        (bool okS, bool infS, bytes memory sigEip) = DuskBls.decompressG1(q.sig);
        if (!okS || infS) return false;
        (bool okA, bytes memory apk) = DuskBls.weightedSumG2(signerPksEip, scalars);
        if (!okA) return false;
        (bool okH, bytes memory h) = DuskBls.hashToG1(m);
        if (!okH) return false;
        return DuskBls.pairingCheck(sigEip, h, apk);
    }
}
