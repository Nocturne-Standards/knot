// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Script} from "forge-std/Script.sol";
import {KnotVectors} from "../test/KnotVectors.sol";
import {GasHarness} from "./GasHarness.sol";

/// Writes gas/n2.initcode.hex and gas/n8.initcode.hex for `cast call --create`. Sends nothing.
contract GasInitcode is Script, KnotVectors {
    function run() external {
        _load();
        vm.writeFile(string.concat(vm.projectRoot(), "/gas/n2.initcode.hex"), vm.toString(initcode(2)));
        vm.writeFile(string.concat(vm.projectRoot(), "/gas/n8.initcode.hex"), vm.toString(initcode(8)));
        vm.writeFile(string.concat(vm.projectRoot(), "/gas/n16.initcode.hex"), vm.toString(initcode(16)));
    }

    function initcode(uint256 n) public view returns (bytes memory) {
        return bytes.concat(type(GasHarness).creationCode, harnessArgs(n));
    }

    function harnessArgs(uint256 n) public view returns (bytes memory) {
        if (n == 2) {
            return _harnessArgs(_pks(S2()), uint32(2), _actCall(S2(), "act_s2"), _rotCall(S2(), S2b(), 2, 1, "g2_rot"));
        }
        if (n == 8) {
            return _harnessArgs(_pks(S8()), uint32(8), _actCall(S8(), "g8_act"), _rotCall(S8(), S8b(), 8, 1, "g8_rot"));
        }
        require(n == 16, "n");
        return
            _harnessArgs(_pks(S16()), uint32(16), _actCall(S16(), "g16_act"), _rotCall(S16(), S16b(), 16, 1, "g16_rot"));
    }
}
