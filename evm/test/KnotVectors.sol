// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {CommonBase} from "forge-std/Base.sol";
import {KnotEvmRoot} from "../src/KnotEvmRoot.sol";

/// Loads test/vectors/knot_root.json (written by vectors-gen) and builds KnotEvmRoot call arguments.
abstract contract KnotVectors is CommonBase {
    string internal J;
    bytes[] internal PKC;
    bytes[] internal PKE;
    address internal ROOT_ADDR;
    /// caller bound into action messages (the gas harness address)
    address internal CONSUMER;
    bytes32 internal REG;
    uint64 internal ACC;
    bytes32 internal DIGEST;
    uint64 internal constant DUSK_CHAIN_ID = 2;
    uint256 internal constant EVM_CHAIN_ID = 745;

    function _load() internal {
        J = vm.readFile(string.concat(vm.projectRoot(), "/test/vectors/knot_root.json"));
        PKC = vm.parseJsonBytesArray(J, ".pk_c");
        PKE = vm.parseJsonBytesArray(J, ".pk_eip");
        ROOT_ADDR = vm.parseJsonAddress(J, ".root_addr");
        CONSUMER = vm.parseJsonAddress(J, ".consumer");
        REG = vm.parseJsonBytes32(J, ".registry_id");
        ACC = uint64(vm.parseJsonUint(J, ".account_id"));
        DIGEST = vm.parseJsonBytes32(J, ".digest");
        require(vm.parseJsonUint(J, ".dusk_chain_id") == DUSK_CHAIN_ID, "dusk chain id");
        require(vm.parseJsonUint(J, ".evm_chain_id") == EVM_CHAIN_ID, "evm chain id");
    }

    function _range(uint256 from, uint256 to) internal pure returns (uint256[] memory s) {
        s = new uint256[](to - from);
        for (uint256 i = 0; i < s.length; i++) {
            s[i] = from + i;
        }
    }

    function S2() internal pure returns (uint256[] memory) {
        return _range(0, 2);
    }

    function S8() internal pure returns (uint256[] memory) {
        return _range(2, 10);
    }

    function S2b() internal pure returns (uint256[] memory) {
        return _range(10, 12);
    }

    function S8b() internal pure returns (uint256[] memory) {
        return _range(12, 20);
    }

    function S16() internal pure returns (uint256[] memory) {
        return _range(21, 37);
    }

    function S16b() internal pure returns (uint256[] memory) {
        return _range(37, 53);
    }

    function _pks(uint256[] memory set) internal view returns (bytes memory out) {
        for (uint256 i = 0; i < set.length; i++) {
            out = bytes.concat(out, PKC[set[i]]);
        }
    }

    function _leaves(uint256[] memory set) internal view returns (bytes32[] memory l) {
        l = new bytes32[](set.length);
        for (uint256 i = 0; i < set.length; i++) {
            l[i] = keccak256(abi.encodePacked(PKC[set[i]], PKE[set[i]]));
        }
    }

    function _str(string memory name, string memory field) internal pure returns (string memory) {
        return string.concat(".", name, ".", field);
    }

    function _bytes(string memory name, string memory field) internal view returns (bytes memory) {
        return vm.parseJsonBytes(J, _str(name, field));
    }

    function _b32(string memory name, string memory field) internal view returns (bytes32) {
        return vm.parseJsonBytes32(J, _str(name, field));
    }

    /// Quorum arguments for vector `name` against member set `set` (bitmap from the vector).
    function _quorum(uint256[] memory set, string memory name) internal view returns (KnotEvmRoot.Quorum memory q) {
        q.leaves = _leaves(set);
        q.signerBitmap = vm.parseJsonUint(J, _str(name, "bitmap"));
        q.sig = _bytes(name, "sig_c");
        for (uint256 i = 0; i < set.length; i++) {
            if ((q.signerBitmap >> i) & 1 == 1) {
                q.signerPks = bytes.concat(q.signerPks, PKC[set[i]]);
                q.signerPksEip = bytes.concat(q.signerPksEip, PKE[set[i]]);
            }
        }
    }

    function _actCall(uint256[] memory set, string memory name) internal view returns (bytes memory) {
        KnotEvmRoot.Quorum memory q = _quorum(set, name);
        return abi.encodeCall(KnotEvmRoot.checkAction, (ACC, DIGEST, q));
    }

    /// GasHarness constructor arguments for the vector deployment.
    function _harnessArgs(bytes memory initPks, uint32 t, bytes memory actCall, bytes memory rotCall)
        internal
        view
        returns (bytes memory)
    {
        return abi.encode(REG, DUSK_CHAIN_ID, ACC, initPks, t, actCall, rotCall);
    }

    function _rotCall(uint256[] memory set, uint256[] memory newSet, uint32 t, uint64 nonce, string memory name)
        internal
        view
        returns (bytes memory)
    {
        KnotEvmRoot.Quorum memory q = _quorum(set, name);
        return abi.encodeCall(KnotEvmRoot.rotate, (ACC, _pks(newSet), t, nonce, q));
    }
}
