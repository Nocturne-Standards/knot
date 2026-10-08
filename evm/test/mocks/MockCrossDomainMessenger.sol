// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// Stand-in for the L2 CrossDomainMessenger (pattern: nocturne-lending `src/mocks/MockCrossDomainMessenger.sol`).
/// `relay` calls `target` with `data` while `xDomainMessageSender()` returns the configured native sender.
contract MockCrossDomainMessenger {
    address public xDomainMessageSender;

    constructor(address sender) {
        xDomainMessageSender = sender;
    }

    function setXDomainMessageSender(address sender) external {
        xDomainMessageSender = sender;
    }

    function relay(address target, bytes calldata data) external {
        (bool ok, bytes memory ret) = target.call(data);
        if (!ok) {
            assembly {
                revert(add(ret, 32), mload(ret))
            }
        }
    }
}
