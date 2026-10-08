// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test, console} from "forge-std/Test.sol";
import {KnotEvmRoot} from "../src/KnotEvmRoot.sol";
import {KnotVectors} from "./KnotVectors.sol";
import {GasHarness} from "../script/GasHarness.sol";
import {MockCrossDomainMessenger} from "./mocks/MockCrossDomainMessenger.sol";

/// Exposes the raw quorum check so the probe vectors (signed over a fixed 32-byte message) can be checked
/// against a stored root.
contract KnotEvmRootHarness is KnotEvmRoot {
    constructor(bytes32 registryId_, address messenger_) KnotEvmRoot(registryId_, 0, messenger_) {}

    function verifyQuorumMessage(uint64 accountId, bytes memory m, Quorum calldata q) external view returns (bool) {
        return _verifyQuorum(accountId, m, q);
    }
}

contract KnotEvmRootTest is Test, KnotVectors {
    KnotEvmRoot root;
    MockCrossDomainMessenger messenger;

    function setUp() public {
        _load();
        vm.chainId(EVM_CHAIN_ID);
        messenger = new MockCrossDomainMessenger(_nativeSender(REG));
        deployCodeTo("KnotEvmRoot.sol:KnotEvmRoot", abi.encode(REG, DUSK_CHAIN_ID, address(messenger)), ROOT_ADDR);
        root = KnotEvmRoot(ROOT_ADDR);
        _bootstrap(ACC, _pks(S2()), 2);
    }

    /// SDK rule (nocturne-lending docs/xdm-aliasing-goldens.md): keccak256(contractId)[-20:]
    function _nativeSender(bytes32 contractId) internal pure returns (address) {
        return address(uint160(uint256(keccak256(abi.encodePacked(contractId)))));
    }

    /// Fresh, unbootstrapped root at a new address (signed vectors are bound to ROOT_ADDR, set up in setUp).
    function _deployEmpty() internal returns (KnotEvmRoot) {
        return new KnotEvmRoot(REG, DUSK_CHAIN_ID, address(messenger));
    }

    function _bootstrap(uint64 accountId, bytes memory pks, uint32 t) internal {
        messenger.relay(address(root), abi.encodeCall(KnotEvmRoot.bootstrap, (accountId, DUSK_CHAIN_ID, pks, t)));
    }

    function _deploy(uint256[] memory set, uint32 t) internal returns (KnotEvmRoot r) {
        root = r = _deployEmpty();
        _bootstrap(ACC, _pks(set), t);
    }

    function _rotate(uint256[] memory set, uint256[] memory newSet, uint32 t, uint64 nonce, string memory name)
        internal
    {
        KnotEvmRoot.Quorum memory q = _quorum(set, name);
        root.rotate(ACC, _pks(newSet), t, nonce, q);
    }

    function _consume(uint256[] memory set, string memory name, bytes32 digest) internal {
        KnotEvmRoot.Quorum memory q = _quorum(set, name);
        vm.prank(CONSUMER);
        root.checkAction(ACC, digest, q);
    }

    // ------------------------------------------------------------------ bootstrap over the messenger (D9)

    function test_bootstrap_fromMessengerWithPinnedSenderSetsRoot() public {
        root = _deployEmpty();
        assertFalse(root.isBootstrapped(ACC));
        vm.expectEmit(true, true, false, true, address(root));
        emit KnotEvmRoot.Rotated(ACC, 0, vm.parseJsonBytes32(J, ".root_s2"), 2, _pks(S2()));
        _bootstrap(ACC, _pks(S2()), 2);
        assertTrue(root.isBootstrapped(ACC));
        assertEq(root.membersRoot(ACC), vm.parseJsonBytes32(J, ".root_s2"));
        assertEq(root.threshold(ACC), 2);
        assertEq(root.rotationNonce(ACC), 0);
    }

    function test_bootstrap_pinnedSenderIsKeccakOfRegistryId() public view {
        assertEq(root.trustedNativeSender(), _nativeSender(REG));
        assertEq(address(root.messenger()), address(messenger));
        // golden from nocturne-lending docs/latch-nav-knot-quorum.md (attested-nav ContractId -> EVM sender)
        assertEq(
            _nativeSender(0xd630444239019a57839a56d451dbf46de5f7734df21ae0715326946a4ed7b53a),
            0x738e5CF39b0b1C11C1CAc6E3AC347A2959E8665b
        );
    }

    function test_bootstrap_wrongMsgSenderRefused() public {
        root = _deployEmpty();
        vm.prank(address(0xBAD));
        vm.expectRevert(
            abi.encodeWithSelector(KnotEvmRoot.NotCrossDomainMessenger.selector, address(0xBAD), address(messenger))
        );
        root.bootstrap(ACC, DUSK_CHAIN_ID, _pks(S2()), 2);
        // even a call carrying the pinned sender's identity directly is refused
        vm.prank(_nativeSender(REG));
        vm.expectRevert(
            abi.encodeWithSelector(KnotEvmRoot.NotCrossDomainMessenger.selector, _nativeSender(REG), address(messenger))
        );
        root.bootstrap(ACC, DUSK_CHAIN_ID, _pks(S2()), 2);
        assertFalse(root.isBootstrapped(ACC));
    }

    function test_bootstrap_wrongXDomainSenderRefused() public {
        root = _deployEmpty();
        messenger.setXDomainMessageSender(address(0xE71C));
        vm.expectRevert(
            abi.encodeWithSelector(KnotEvmRoot.UntrustedNativeSender.selector, address(0xE71C), _nativeSender(REG))
        );
        _bootstrap(ACC, _pks(S2()), 2);
        assertFalse(root.isBootstrapped(ACC));
    }

    function test_bootstrap_wrongDuskChainRefused() public {
        root = _deployEmpty();
        vm.expectRevert(
            abi.encodeWithSelector(KnotEvmRoot.WrongDuskChain.selector, uint64(DUSK_CHAIN_ID + 1), uint64(DUSK_CHAIN_ID))
        );
        messenger.relay(
            address(root), abi.encodeCall(KnotEvmRoot.bootstrap, (ACC, uint64(DUSK_CHAIN_ID + 1), _pks(S2()), 2))
        );
        assertFalse(root.isBootstrapped(ACC));
    }

    function test_bootstrap_secondBootstrapRefused() public {
        vm.expectRevert(KnotEvmRoot.AlreadyBootstrapped.selector);
        _bootstrap(ACC, _pks(S8()), 6);
        assertEq(root.membersRoot(ACC), vm.parseJsonBytes32(J, ".root_s2"));
    }

    function test_bootstrap_secondBootstrapRefusedAfterRotation() public {
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
        vm.expectRevert(KnotEvmRoot.AlreadyBootstrapped.selector);
        _bootstrap(ACC, _pks(S2()), 2);
    }

    function test_bootstrap_otherAccountIndependent() public {
        _bootstrap(ACC + 1, _pks(S8()), 6);
        assertEq(root.membersRoot(ACC + 1), vm.parseJsonBytes32(J, ".root_s8"));
        assertEq(root.membersRoot(ACC), vm.parseJsonBytes32(J, ".root_s2"));
    }

    function test_bootstrap_badThresholdRefused() public {
        root = _deployEmpty();
        vm.expectRevert(KnotEvmRoot.BadThreshold.selector);
        _bootstrap(ACC, _pks(S2()), 3);
        vm.expectRevert(KnotEvmRoot.BadThreshold.selector);
        _bootstrap(ACC, _pks(S2()), 0);
    }

    function test_bootstrap_offSubgroupMemberRefused() public {
        root = _deployEmpty();
        bytes[] memory off = vm.parseJsonBytesArray(_probe(), ".offsub_g2");
        for (uint256 i = 0; i < off.length; i++) {
            vm.expectRevert(KnotEvmRoot.BadMembers.selector);
            _bootstrap(ACC, bytes.concat(PKC[0], off[i]), 1);
        }
    }

    function test_bootstrap_rootFromProbeDuskVector() public {
        root = _deployEmpty();
        string memory p = _probe();
        bytes memory pks = vm.parseJsonBytes(p, ".multi_n2_m32.pks_compressed");
        bytes memory eips = vm.parseJsonBytes(p, ".multi_n2_m32.pks_eip");
        _bootstrap(ACC, pks, 2);
        bytes32 l0 = keccak256(bytes.concat(_slice(pks, 0, 96), _slice(eips, 0, 256)));
        bytes32 l1 = keccak256(bytes.concat(_slice(pks, 96, 96), _slice(eips, 256, 256)));
        assertEq(root.membersRoot(ACC), keccak256(abi.encodePacked(l0, l1)));
    }

    function test_beforeBootstrap_rotateAndActionRefused() public {
        root = _deployEmpty();
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "rot_s2_to_s8");
        vm.expectRevert(KnotEvmRoot.NotBootstrapped.selector);
        root.rotate(ACC, _pks(S8()), 6, 1, q);
        q = _quorum(S2(), "act_s2");
        vm.prank(CONSUMER);
        vm.expectRevert(KnotEvmRoot.NotBootstrapped.selector);
        root.checkAction(ACC, DIGEST, q);
    }

    function test_afterBootstrap_rotateStillNeedsQuorum() public {
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8_outsider");
        vm.expectRevert(KnotEvmRoot.BelowThreshold.selector);
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8_one");
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
        assertEq(root.membersRoot(ACC), vm.parseJsonBytes32(J, ".root_s8"));
    }

    // ------------------------------------------------------------------ encoding agrees with the generator

    function test_bootstrapRootMatchesGenerator() public view {
        assertEq(root.membersRoot(ACC), vm.parseJsonBytes32(J, ".root_s2"));
        assertEq(root.threshold(ACC), 2);
        assertEq(root.rotationNonce(ACC), 0);
        assertEq(root.registryId(), REG);
        assertEq(root.duskChainId(), DUSK_CHAIN_ID);
    }

    function test_messagesMatchGenerator() public view {
        assertEq(root.actionMessage(ACC, CONSUMER, DIGEST), _bytes("act_s2", "msg"));
        assertEq(root.rotationMessage(ACC, _b32("rot_s2_to_s8", "new_root"), 6, 1), _bytes("rot_s2_to_s8", "msg"));
    }

    // ------------------------------------------------------------------ acceptance 1: rotation

    function test_rotate_validQuorumUpdatesRoot() public {
        vm.expectEmit(true, true, false, true, ROOT_ADDR);
        emit KnotEvmRoot.Rotated(ACC, 1, _b32("rot_s2_to_s8", "new_root"), 6, _pks(S8()));
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
        assertEq(root.membersRoot(ACC), _b32("rot_s2_to_s8", "new_root"));
        assertEq(root.membersRoot(ACC), vm.parseJsonBytes32(J, ".root_s8"));
        assertEq(root.threshold(ACC), 6);
        assertEq(root.rotationNonce(ACC), 1);
    }

    function test_rotate_chainSixOfEightNonContiguous() public {
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
        _rotate(S8(), S2b(), 2, 2, "rot_s8_to_s2b_n2");
        assertEq(root.membersRoot(ACC), _b32("rot_s8_to_s2b_n2", "new_root"));
        assertEq(root.threshold(ACC), 2);
        assertEq(root.rotationNonce(ACC), 2);
    }

    function test_rotate_replayedNonceRefused() public {
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
        vm.expectRevert(KnotEvmRoot.BadNonce.selector);
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
    }

    function test_rotate_skippedNonceRefused() public {
        vm.expectRevert(KnotEvmRoot.BadNonce.selector);
        _rotate(S2(), S8(), 6, 2, "rot_s2_to_s8");
    }

    function test_rotate_zeroNonceRefused() public {
        vm.expectRevert(KnotEvmRoot.BadNonce.selector);
        _rotate(S2(), S8(), 6, 0, "rot_s2_to_s8");
    }

    function test_rotate_belowThresholdRefused() public {
        // real 1-of-2 Dusk multisig signature over the right message; threshold is 2
        vm.expectRevert(KnotEvmRoot.BelowThreshold.selector);
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8_one");
    }

    function test_rotate_outsiderSignatureRefused() public {
        // aggregate by member 0 and a non-member, bitmap claims members 0 and 1
        assertFalse(vm.parseJsonBool(J, ".rot_s2_to_s8_outsider.rust_verify"));
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8_outsider");
    }

    function test_rotate_signatureForOtherMessageRefused() public {
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _rotate(S2(), S8(), 6, 1, "act_s2");
    }

    function test_rotate_tamperedThresholdRefused() public {
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _rotate(S2(), S8(), 5, 1, "rot_s2_to_s8");
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _rotate(S2(), S8(), 7, 1, "rot_s2_to_s8");
    }

    function test_rotate_thresholdOutOfRangeRefused() public {
        vm.expectRevert(KnotEvmRoot.BadThreshold.selector);
        _rotate(S2(), S8(), 0, 1, "rot_s2_to_s8");
        vm.expectRevert(KnotEvmRoot.BadThreshold.selector);
        _rotate(S2(), S8(), 9, 1, "rot_s2_to_s8");
    }

    function test_rotate_tamperedMembersRefused() public {
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _rotate(S2(), S8b(), 6, 1, "rot_s2_to_s8");
    }

    function test_rotate_duplicateMemberRefused() public {
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "rot_s2_to_s8");
        bytes memory dup = bytes.concat(PKC[2], PKC[3], PKC[2]);
        vm.expectRevert(KnotEvmRoot.BadMembers.selector);
        root.rotate(ACC, dup, 2, 1, q);
    }

    function test_rotate_offSubgroupMemberRefused() public {
        bytes[] memory off = vm.parseJsonBytesArray(_probe(), ".offsub_g2");
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "rot_s2_to_s8");
        for (uint256 i = 0; i < off.length; i++) {
            vm.expectRevert(KnotEvmRoot.BadMembers.selector);
            root.rotate(ACC, bytes.concat(PKC[2], off[i]), 1, 1, q);
        }
    }

    function test_rotate_oldQuorumAfterRotationRefused() public {
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
        vm.expectRevert(KnotEvmRoot.RootMismatch.selector);
        _rotate(S2(), S2b(), 2, 2, "g2_rot");
    }

    function test_rotate_wrongEipForLeafRefused() public {
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "rot_s2_to_s8");
        q.signerPksEip = bytes.concat(PKE[1], PKE[0]);
        vm.expectRevert(abi.encodeWithSelector(KnotEvmRoot.NotMember.selector, 0));
        root.rotate(ACC, _pks(S8()), 6, 1, q);
    }

    function test_rotate_bitmapOutOfRangeRefused() public {
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "rot_s2_to_s8");
        q.signerBitmap = 0x7;
        vm.expectRevert(KnotEvmRoot.BadBitmap.selector);
        root.rotate(ACC, _pks(S8()), 6, 1, q);
    }

    function test_cap_seventeenMembersRefused() public {
        bytes memory pks17 = _pks(_range(21, 38));
        vm.expectRevert(KnotEvmRoot.BadMembers.selector);
        _bootstrap(ACC + 1, pks17, 2);
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "rot_s2_to_s8");
        vm.expectRevert(KnotEvmRoot.BadMembers.selector);
        root.rotate(ACC, pks17, 6, 1, q);
    }

    // ------------------------------------------------------------------ acceptance 2: action check + consume (D4)

    function test_action_realDuskSignatureAcceptedAndConsumed() public {
        assertTrue(vm.parseJsonBool(J, ".act_s2.rust_verify"));
        assertFalse(root.isConsumed(ACC, DIGEST));
        vm.expectEmit(true, true, true, true, ROOT_ADDR);
        emit KnotEvmRoot.ActionConsumed(ACC, 0, CONSUMER, DIGEST);
        _consume(S2(), "act_s2", DIGEST);
        assertTrue(root.isConsumed(ACC, DIGEST));
    }

    function test_action_sameActionTwiceInEpochRefused() public {
        _consume(S2(), "act_s2", DIGEST);
        vm.expectRevert(KnotEvmRoot.ActionReplayed.selector);
        _consume(S2(), "act_s2", DIGEST);
    }

    function test_action_replayAcrossRotationRefused() public {
        _consume(S2(), "act_s2", DIGEST);
        _rotate(S2(), S8(), 6, 1, "rot_s2_to_s8");
        assertFalse(root.isConsumed(ACC, DIGEST)); // epoch-0 mark ignored
        // old quorum's signature: old root
        vm.expectRevert(KnotEvmRoot.RootMismatch.selector);
        _consume(S2(), "act_s2", DIGEST);
        // new quorum, signature bound to nonce 0: refused
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _consume(S8(), "g8_act", DIGEST);
        // new quorum at nonce 1: accepted once, then refused
        _consume(S8(), "act_s8_n1", DIGEST);
        assertTrue(root.isConsumed(ACC, DIGEST));
        vm.expectRevert(KnotEvmRoot.ActionReplayed.selector);
        _consume(S8(), "act_s8_n1", DIGEST);
    }

    function test_action_wrongCallerRefused() public {
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "act_s2");
        vm.prank(address(0xBEEF));
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        root.checkAction(ACC, DIGEST, q);
        assertFalse(root.isConsumed(ACC, DIGEST));
    }

    function test_action_wrongDigestRejected() public {
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        _consume(S2(), "act_s2", keccak256("other action"));
    }

    function test_action_wrongSignatureRejected() public {
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "act_s2");
        q.sig = _bytes("rot_s2_to_s8", "sig_c"); // valid point, signs another message
        vm.prank(CONSUMER);
        vm.expectRevert(KnotEvmRoot.BadSignature.selector);
        root.checkAction(ACC, DIGEST, q);
    }

    function test_action_offSubgroupSignatureRejected() public {
        bytes[] memory off = vm.parseJsonBytesArray(_probe(), ".offsub_g1");
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "act_s2");
        for (uint256 i = 0; i < off.length; i++) {
            vm.prank(CONSUMER);
            vm.expectRevert(KnotEvmRoot.BadSignature.selector);
            q.sig = off[i];
            root.checkAction(ACC, DIGEST, q);
        }
    }

    function test_action_belowThresholdReverts() public {
        KnotEvmRoot.Quorum memory q = _quorum(S2(), "act_s2");
        (q.signerBitmap, q.signerPks, q.signerPksEip) = (1, PKC[0], PKE[0]);
        vm.prank(CONSUMER);
        vm.expectRevert(KnotEvmRoot.BelowThreshold.selector);
        root.checkAction(ACC, DIGEST, q);
    }

    // ------------------------------------------------------------------ probe vectors (unchanged bytes)

    function _probe() internal view returns (string memory) {
        return vm.readFile(string.concat(vm.projectRoot(), "/test/vectors/dusk_probe_multi.json"));
    }

    function _probeCheck(string memory p, string memory name, uint32 t, bytes memory sigOverride)
        internal
        returns (bool)
    {
        string memory k = string.concat(".", name, ".");
        bytes memory pks = vm.parseJsonBytes(p, string.concat(k, "pks_compressed"));
        bytes memory eips = vm.parseJsonBytes(p, string.concat(k, "pks_eip"));
        uint256 n = vm.parseJsonUint(p, string.concat(k, "n"));
        bytes memory sig =
            sigOverride.length == 0 ? vm.parseJsonBytes(p, string.concat(k, "sig_compressed")) : sigOverride;
        KnotEvmRootHarness h = new KnotEvmRootHarness(REG, address(messenger));
        messenger.relay(address(h), abi.encodeCall(KnotEvmRoot.bootstrap, (ACC, uint64(0), pks, t)));
        bytes32[] memory leaves = new bytes32[](n);
        for (uint256 i = 0; i < n; i++) {
            leaves[i] = keccak256(bytes.concat(_slice(pks, i * 96, 96), _slice(eips, i * 256, 256)));
        }
        return h.verifyQuorumMessage(
            ACC, vm.parseJsonBytes(p, string.concat(k, "msg")), KnotEvmRoot.Quorum(leaves, (1 << n) - 1, pks, eips, sig)
        );
    }

    function _slice(bytes memory b, uint256 off, uint256 len) internal pure returns (bytes memory o) {
        o = new bytes(len);
        for (uint256 i = 0; i < len; i++) {
            o[i] = b[off + i];
        }
    }

    function test_probe_realDuskMultisigN2Accepted() public {
        string memory p = _probe();
        assertTrue(vm.parseJsonBool(p, ".multi_n2_m32.rust_verify"));
        assertTrue(_probeCheck(p, "multi_n2_m32", 2, ""));
    }

    function test_probe_realDuskMultisigN8Accepted() public {
        string memory p = _probe();
        assertTrue(_probeCheck(p, "multi_n8_m32", 8, ""));
    }

    function test_probe_wrongMessageRejected() public {
        string memory p = _probe();
        assertFalse(vm.parseJsonBool(p, ".multi_NEG_n2_wrong_msg.rust_verify"));
        assertFalse(_probeCheck(p, "multi_NEG_n2_wrong_msg", 2, ""));
    }

    function test_probe_wrongSignatureRejected() public {
        string memory p = _probe();
        bytes memory other = vm.parseJsonBytes(p, ".multi_n8_m32.sig_compressed");
        assertFalse(_probeCheck(p, "multi_n2_m32", 2, other));
    }
}

/// Account bootstrapped with 16 members at ROOT_ADDR (the g16 vectors are bound to that address).
contract KnotEvmRootCapTest is Test, KnotVectors {
    function test_cap_sixteenMembersAccepted() public {
        _load();
        vm.chainId(EVM_CHAIN_ID);
        MockCrossDomainMessenger messenger =
            new MockCrossDomainMessenger(address(uint160(uint256(keccak256(abi.encodePacked(REG))))));
        deployCodeTo("KnotEvmRoot.sol:KnotEvmRoot", abi.encode(REG, DUSK_CHAIN_ID, address(messenger)), ROOT_ADDR);
        KnotEvmRoot root = KnotEvmRoot(ROOT_ADDR);
        assertEq(root.MAX_MEMBERS(), 16);
        messenger.relay(ROOT_ADDR, abi.encodeCall(KnotEvmRoot.bootstrap, (ACC, DUSK_CHAIN_ID, _pks(S16()), 16)));
        assertEq(root.threshold(ACC), 16);
        KnotEvmRoot.Quorum memory q = _quorum(S16(), "g16_rot");
        root.rotate(ACC, _pks(S16b()), 16, 1, q);
        assertEq(root.membersRoot(ACC), _b32("g16_rot", "new_root"));
    }
}

/// Local reference run of the gas harness (revm). The DuskEVM figures come from `cast call --create`.
contract KnotEvmRootGasTest is Test, KnotVectors {
    function setUp() public {
        _load();
        vm.chainId(EVM_CHAIN_ID);
    }

    function _run(bytes memory args, string memory rotName, uint256 n) internal {
        bytes memory init = bytes.concat(type(GasHarness).creationCode, args);
        address from = vm.parseJsonAddress(J, ".from");
        assertEq(vm.parseJsonAddress(J, ".harness"), vm.computeCreateAddress(from, 0));
        vm.prank(from);
        address h;
        assembly {
            h := create(0, add(init, 32), mload(init))
        }
        require(h != address(0), "harness create failed");
        (address r, uint256 gDeploy, uint256 gBoot, uint256 gAct, uint256 gRot, bytes32 newRoot) =
            abi.decode(h.code, (address, uint256, uint256, uint256, uint256, bytes32));
        assertEq(r, ROOT_ADDR);
        assertEq(newRoot, _b32(rotName, "new_root"));
        console.log("N", n);
        console.log("  deploy", gDeploy);
        console.log("  bootstrap", gBoot);
        console.log("  checkAction", gAct);
        console.log("  rotate", gRot);
    }

    function test_gas_n2() public {
        _run(
            _harnessArgs(_pks(S2()), uint32(2), _actCall(S2(), "act_s2"), _rotCall(S2(), S2b(), 2, 1, "g2_rot")),
            "g2_rot",
            2
        );
    }

    function test_gas_n16() public {
        _run(
            _harnessArgs(
                _pks(S16()), uint32(16), _actCall(S16(), "g16_act"), _rotCall(S16(), S16b(), 16, 1, "g16_rot")
            ),
            "g16_rot",
            16
        );
    }

    function test_gas_n8() public {
        _run(
            _harnessArgs(_pks(S8()), uint32(8), _actCall(S8(), "g8_act"), _rotCall(S8(), S8b(), 8, 1, "g8_rot")),
            "g8_rot",
            8
        );
    }
}
