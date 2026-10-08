// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {KnotEvmRoot} from "../src/KnotEvmRoot.sol";
import {MockCrossDomainMessenger} from "../test/mocks/MockCrossDomainMessenger.sol";

/// Measurement only. Run with `cast call --create` (eth_call): never deployed, no transaction.
/// Creates a messenger stand-in (at create(this, 1)) and a KnotEvmRoot (at create(this, 2)), bootstraps the account
/// through the stand-in, then measures one checkAction (consume) and one rotate call. Also checks that a second
/// identical checkAction is refused.
/// Returns abi.encode(root, gasDeploy, gasBootstrap, gasAction, gasRotate, newRoot).
contract GasHarness {
    constructor(
        bytes32 registryId,
        uint64 duskChainId,
        uint64 accountId,
        bytes memory initPks,
        uint32 threshold,
        bytes memory actCall,
        bytes memory rotCall
    ) {
        MockCrossDomainMessenger messenger = new MockCrossDomainMessenger(
            address(uint160(uint256(keccak256(abi.encodePacked(registryId)))))
        );

        uint256 g = gasleft();
        KnotEvmRoot root = new KnotEvmRoot(registryId, duskChainId, address(messenger));
        uint256 gasDeploy = g - gasleft();

        g = gasleft();
        messenger.relay(address(root), abi.encodeCall(KnotEvmRoot.bootstrap, (accountId, duskChainId, initPks, threshold)));
        uint256 gasBootstrap = g - gasleft();

        g = gasleft();
        (bool ok,) = address(root).call(actCall);
        uint256 gasAction = g - gasleft();
        require(ok && root.isConsumed(accountId, bytes32(_word(actCall, 36))), "action");

        g = gasleft();
        (ok,) = address(root).call(rotCall);
        uint256 gasRotate = g - gasleft();
        require(ok, "rotate");

        (ok,) = address(root).call(actCall);
        require(!ok, "replay accepted");

        bytes memory out =
            abi.encode(address(root), gasDeploy, gasBootstrap, gasAction, gasRotate, root.membersRoot(accountId));
        assembly {
            return(add(out, 32), mload(out))
        }
    }

    function _word(bytes memory b, uint256 off) private pure returns (uint256 w) {
        assembly {
            w := mload(add(add(b, 32), off))
        }
    }
}
