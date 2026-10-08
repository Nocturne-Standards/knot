// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// Dusk BLS12-381 (bls12_381-bls 0.6.0, V2) on EIP-2537 precompiles.
///
/// Encodings: Dusk uses ZCash compressed big-endian (G1 48 bytes, G2 96 bytes as x_c1 || x_c0; flags in the
/// top 3 bits of byte 0: 0x80 compressed, 0x40 infinity, 0x20 "largest y"). EIP-2537 wants uncompressed
/// points: G1 128 bytes (x, y), G2 256 bytes (x_c0, x_c1, y_c0, y_c1), 64-byte big-endian field elements.
///
/// Decoding checks the flags, x < p, on-curve, and subgroup membership (k=1 MSM with scalar r; EIP-2537 MSM
/// rejects off-subgroup input, and the zero-result test covers a client that would not). Fp arithmetic uses
/// the modexp precompile only. p = 3 (mod 4), so sqrt_Fp(a) = a^((p+1)/4), checked by squaring.
///
/// Multisig (abi::verify_bls_multisig): apk = sum h1(pk_i) * pk_i, sig = sum h1(pk_i) * sk_i * H(m),
/// h1(pk) = blake2b-512(H1_DST || compressed pk) as a little-endian 512-bit integer, mod r.
/// Verify: e(sig, -g2) * e(H(m), apk) == 1, H = hash_to_curve G1, expand_message_xmd SHA-256, DST below.
struct Fp {
    uint256 h; // bits 256..380
    uint256 l; // bits 0..255
}

struct Fp2 {
    Fp c0;
    Fp c1;
}

library DuskBls {
    bytes internal constant DST = "BLS_SIG_BLS12381G1_XMD:SHA-256_DUSK_V2"; // 38 bytes
    bytes internal constant H1_DST = "BLS_SIG_BLS12381_SCALAR_SHA256_DUSK_H1_V2"; // 41 bytes

    // p = 0x1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaab
    uint256 internal constant PH = 0x1a0111ea397fe69a4b1ba7b6434bacd7;
    uint256 internal constant PL = 0x64774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaab;
    // (p+1)/4
    uint256 internal constant ESQ_H = 0x680447a8e5ff9a692c6e9ed90d2eb35;
    uint256 internal constant ESQ_L = 0xd91dd2e13ce144afd9cc34a83dac3d8907aaffffac54ffffee7fbfffffffeaab;
    // p-2
    uint256 internal constant EINV_H = 0x1a0111ea397fe69a4b1ba7b6434bacd7;
    uint256 internal constant EINV_L = 0x64774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaa9;
    // (p-1)/2
    uint256 internal constant HALF_H = 0x0d0088f51cbff34d258dd3db21a5d66b;
    uint256 internal constant HALF_L = 0xb23ba5c279c2895fb39869507b587b120f55ffff58a9ffffdcff7fffffffd555;
    uint256 internal constant FLAG_CLEAR = (uint256(1) << 125) - 1;

    // scalar field order r
    uint256 internal constant R = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001;
    uint256 internal constant G1MSM_K1_GAS = 12000;
    uint256 internal constant G2MSM_K1_GAS = 22500;
    /// EIP-2537 PAIRING_CHECK price for k = 2 (32600 * 2 + 37700). Caps the burn on bad input.
    uint256 internal constant PAIRING_K2_GAS = 102900;

    // -G2 generator, EIP-2537 layout: x_c0, x_c1, y_c0, y_c1
    uint256 internal constant NG2_X0_HI = 0x024aa2b2f08f0a91260805272dc51051;
    uint256 internal constant NG2_X0_LO = 0xc6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8;
    uint256 internal constant NG2_X1_HI = 0x13e02b6052719f607dacd3a088274f65;
    uint256 internal constant NG2_X1_LO = 0x596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e;
    uint256 internal constant NG2_Y0_HI = 0x0d1b3cc2c7027888be51d9ef691d77bc;
    uint256 internal constant NG2_Y0_LO = 0xb679afda66c73f17f9ee3837a55024f78c71363275a75d75d86bab79f74782aa;
    uint256 internal constant NG2_Y1_HI = 0x13fa4d4a0ad8b1ce186ed5061789213d;
    uint256 internal constant NG2_Y1_LO = 0x993923066dddaf1040bc3ff59f825c78df74f2d75467e25e0f55f8a00fa030ed;

    // blake2b-512 initial state h0..h7 (IV, h0 ^= 0x01010040), 8-byte little-endian words
    bytes32 internal constant B2_H_A = 0x48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5;
    bytes32 internal constant B2_H_B = 0xd182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b;

    // ------------------------------------------------------------------ decoding

    /// ok: flags valid, x < p, on curve, in the subgroup (or the canonical infinity encoding).
    /// inf: point at infinity (out is all zero, the EIP-2537 infinity encoding).
    /// out: EIP-2537 G1 encoding, 128 bytes.
    function decompressG1(bytes memory c) internal view returns (bool ok, bool inf, bytes memory out) {
        out = new bytes(128);
        if (c.length != 48) return (false, false, out);
        uint8 f = uint8(c[0]);
        if (f & 0x80 == 0) return (false, false, out);
        bool sortF = (f & 0x20) != 0;
        Fp memory x = _load(c, 0);
        x.h &= FLAG_CLEAR;
        if (f & 0x40 != 0) {
            bool z = !sortF && x.h == 0 && x.l == 0;
            return (z, z, out);
        }
        if (!_lt(x, Fp(PH, PL))) return (false, false, out);
        Fp memory rhs = _add(_mexp(x, 1, 3, 0), Fp(0, 4)); // x^3 + 4
        Fp memory y = _mexp(rhs, 48, ESQ_H, ESQ_L);
        if (!_eq(_sqr(y), rhs)) return (false, false, out);
        if (_gtHalf(y) != sortF) y = _neg(y);
        _store(out, 0, x);
        _store(out, 64, y);
        if (!inSubgroupG1(out)) return (false, false, new bytes(128));
        return (true, false, out);
    }

    /// out: EIP-2537 G2 encoding, 256 bytes: x_c0, x_c1, y_c0, y_c1. Same checks as decompressG1.
    function decompressG2(bytes memory c) internal view returns (bool ok, bool inf, bytes memory out) {
        out = new bytes(256);
        if (c.length != 96) return (false, false, out);
        uint8 f = uint8(c[0]);
        if (f & 0x80 == 0) return (false, false, out);
        bool sortF = (f & 0x20) != 0;
        Fp2 memory x;
        x.c1 = _load(c, 0);
        x.c1.h &= FLAG_CLEAR;
        x.c0 = _load(c, 48);
        if (f & 0x40 != 0) {
            bool z = !sortF && x.c1.h == 0 && x.c1.l == 0 && x.c0.h == 0 && x.c0.l == 0;
            return (z, z, out);
        }
        if (!_lt(x.c1, Fp(PH, PL)) || !_lt(x.c0, Fp(PH, PL))) return (false, false, out);
        Fp2 memory rhs = _f2add(_f2mul(_f2sqr(x), x), Fp2(Fp(0, 4), Fp(0, 4))); // x^3 + 4(1+u)
        (bool sok, Fp2 memory y) = _f2sqrt(rhs);
        if (!sok) return (false, false, out);
        bool largest = (y.c1.h != 0 || y.c1.l != 0) ? _gtHalf(y.c1) : _gtHalf(y.c0);
        if (largest != sortF) y = Fp2(_neg(y.c0), _neg(y.c1));
        _store(out, 0, x.c0);
        _store(out, 64, x.c1);
        _store(out, 128, y.c0);
        _store(out, 192, y.c1);
        if (!inSubgroupG2(out)) return (false, false, new bytes(256));
        return (true, false, out);
    }

    /// pt: 128-byte EIP-2537 G1 point, on curve, not infinity. true iff r * pt == infinity.
    function inSubgroupG1(bytes memory pt) internal view returns (bool ok) {
        // 63/64 forwarding: require enough gas so a failed call means "off subgroup", not "out of gas"
        require(gasleft() > G1MSM_K1_GAS + G1MSM_K1_GAS / 63 + 200, "gas");
        assembly {
            let m := mload(0x40)
            let s := add(pt, 32)
            mstore(m, mload(s))
            mstore(add(m, 32), mload(add(s, 32)))
            mstore(add(m, 64), mload(add(s, 64)))
            mstore(add(m, 96), mload(add(s, 96)))
            mstore(add(m, 128), R)
            ok := staticcall(G1MSM_K1_GAS, 0x0c, m, 160, m, 128)
            if ok { ok := iszero(or(or(mload(m), mload(add(m, 32))), or(mload(add(m, 64)), mload(add(m, 96))))) }
        }
    }

    /// pt: 256-byte EIP-2537 G2 point, on curve, not infinity. true iff r * pt == infinity.
    function inSubgroupG2(bytes memory pt) internal view returns (bool ok) {
        require(gasleft() > G2MSM_K1_GAS + G2MSM_K1_GAS / 63 + 200, "gas");
        assembly {
            let m := mload(0x40)
            let s := add(pt, 32)
            for { let j := 0 } lt(j, 256) { j := add(j, 32) } { mstore(add(m, j), mload(add(s, j))) }
            mstore(add(m, 256), R)
            ok := staticcall(G2MSM_K1_GAS, 0x0e, m, 288, m, 256)
            if ok {
                let acc := 0
                for { let j := 0 } lt(j, 256) { j := add(j, 32) } { acc := or(acc, mload(add(m, j))) }
                ok := iszero(acc)
            }
        }
    }

    // ------------------------------------------------------------------ multisig

    /// h1(pk) = blake2b-512(H1_DST || pk) read little-endian, mod r. pk: 96-byte compressed G2.
    function h1(bytes memory pk) internal view returns (uint256 s) {
        require(pk.length == 96, "pk len");
        bytes memory M = abi.encodePacked(H1_DST, pk); // 137 bytes: one full block + 9 bytes
        bytes32 w0;
        bytes32 w1;
        bytes32 w2;
        bytes32 w3;
        bytes32 w4;
        assembly {
            let mp := add(M, 32)
            w0 := mload(mp)
            w1 := mload(add(mp, 32))
            w2 := mload(add(mp, 64))
            w3 := mload(add(mp, 96))
            w4 := and(mload(add(mp, 128)), shl(184, 0xffffffffffffffffff))
        }
        bytes memory out = new bytes(64);
        bytes memory inp = abi.encodePacked(
            uint32(12), B2_H_A, B2_H_B, w0, w1, w2, w3, bytes8(0x8000000000000000), bytes8(0), uint8(0)
        );
        bool ok;
        assembly { ok := staticcall(gas(), 0x09, add(inp, 32), 213, add(out, 32), 64) }
        require(ok, "blake2f");
        bytes32 ha;
        bytes32 hb;
        assembly {
            ha := mload(add(out, 32))
            hb := mload(add(out, 64))
        }
        inp = abi.encodePacked(
            uint32(12), ha, hb, w4, bytes32(0), bytes32(0), bytes32(0), bytes8(0x8900000000000000), bytes8(0), uint8(1)
        );
        assembly { ok := staticcall(gas(), 0x09, add(inp, 32), 213, add(out, 32), 64) }
        require(ok, "blake2f");
        // digest d[0..64) is a little-endian integer: big-endian bytes = reverse(d)
        uint256 be0;
        uint256 be1;
        assembly {
            be0 := mload(add(out, 64))
            be1 := mload(add(out, 32))
        }
        be0 = _bswap(be0);
        be1 = _bswap(be1);
        assembly {
            let p := mload(0x40)
            mstore(p, 64)
            mstore(add(p, 32), 1)
            mstore(add(p, 64), 32)
            mstore(add(p, 96), be0)
            mstore(add(p, 128), be1)
            mstore8(add(p, 160), 1)
            mstore(add(p, 161), R)
            ok := staticcall(gas(), 0x05, p, 193, p, 32)
            s := mload(p)
        }
        require(ok, "modexp");
    }

    /// apk = sum s_i * pk_i via one G2MSM. pksEip: n * 256 bytes, scalars: n entries.
    function weightedSumG2(bytes memory pksEip, uint256[] memory scalars)
        internal
        view
        returns (bool ok, bytes memory apk)
    {
        uint256 n = scalars.length;
        require(n > 0 && pksEip.length == n * 256, "msm shape");
        bytes memory msm = new bytes(n * 288);
        assembly {
            let src := add(pksEip, 32)
            let dst := add(msm, 32)
            let sc := add(scalars, 32)
            for { let i := 0 } lt(i, n) { i := add(i, 1) } {
                let d := add(dst, mul(i, 288))
                let s := add(src, mul(i, 256))
                for { let j := 0 } lt(j, 256) { j := add(j, 32) } { mstore(add(d, j), mload(add(s, j))) }
                mstore(add(d, 256), mload(add(sc, mul(i, 32))))
            }
        }
        apk = new bytes(256);
        assembly { ok := staticcall(gas(), 0x0e, add(msm, 32), mul(n, 288), add(apk, 32), 256) }
    }

    // ------------------------------------------------------------------ hash to curve + pairing

    function hashToG1(bytes memory m) internal view returns (bool ok, bytes memory h) {
        (bytes32 b1, bytes32 b2, bytes32 b3, bytes32 b4) = _expand(m);
        bytes memory u0 = _modP(b1, b2);
        bytes memory u1 = _modP(b3, b4);
        bytes memory maps = new bytes(256);
        h = new bytes(128);
        bool s0;
        bool s1;
        bool s2;
        assembly {
            let mp := add(maps, 32)
            s0 := staticcall(gas(), 0x10, add(u0, 32), 64, mp, 128)
            s1 := staticcall(gas(), 0x10, add(u1, 32), 64, add(mp, 128), 128)
            s2 := staticcall(gas(), 0x0b, mp, 256, add(h, 32), 128)
        }
        ok = s0 && s1 && s2;
    }

    /// e(sig, -g2) * e(h, apk) == 1. Pairing call capped at PAIRING_K2_GAS.
    function pairingCheck(bytes memory sig, bytes memory h, bytes memory apk) internal view returns (bool verified) {
        require(gasleft() > PAIRING_K2_GAS + PAIRING_K2_GAS / 63 + 500, "gas");
        bytes memory pin = new bytes(768);
        assembly {
            let p := add(pin, 32)
            let s := add(sig, 32)
            mstore(p, mload(s))
            mstore(add(p, 32), mload(add(s, 32)))
            mstore(add(p, 64), mload(add(s, 64)))
            mstore(add(p, 96), mload(add(s, 96)))
            mstore(add(p, 128), NG2_X0_HI)
            mstore(add(p, 160), NG2_X0_LO)
            mstore(add(p, 192), NG2_X1_HI)
            mstore(add(p, 224), NG2_X1_LO)
            mstore(add(p, 256), NG2_Y0_HI)
            mstore(add(p, 288), NG2_Y0_LO)
            mstore(add(p, 320), NG2_Y1_HI)
            mstore(add(p, 352), NG2_Y1_LO)
            let hh := add(h, 32)
            mstore(add(p, 384), mload(hh))
            mstore(add(p, 416), mload(add(hh, 32)))
            mstore(add(p, 448), mload(add(hh, 64)))
            mstore(add(p, 480), mload(add(hh, 96)))
            let a := add(apk, 32)
            for { let j := 0 } lt(j, 256) { j := add(j, 32) } { mstore(add(add(p, 512), j), mload(add(a, j))) }
            let sp := staticcall(PAIRING_K2_GAS, 0x0f, p, 768, p, 32)
            if sp { verified := eq(mload(p), 1) }
        }
    }

    /// expand_message_xmd(msg, DST, 128) with SHA-256
    function _expand(bytes memory m) private pure returns (bytes32 b1, bytes32 b2, bytes32 b3, bytes32 b4) {
        bytes32 b0 = sha256(abi.encodePacked(new bytes(64), m, uint16(128), uint8(0), DST, uint8(38)));
        b1 = sha256(abi.encodePacked(b0, uint8(1), DST, uint8(38)));
        b2 = sha256(abi.encodePacked(b0 ^ b1, uint8(2), DST, uint8(38)));
        b3 = sha256(abi.encodePacked(b0 ^ b2, uint8(3), DST, uint8(38)));
        b4 = sha256(abi.encodePacked(b0 ^ b3, uint8(4), DST, uint8(38)));
    }

    /// (hi||lo as 512-bit big-endian) mod p, as 64-byte big-endian (EIP-2537 field element)
    function _modP(bytes32 hi, bytes32 lo) private view returns (bytes memory out) {
        bytes memory inp = new bytes(225);
        out = new bytes(64);
        bool s;
        assembly {
            let p := add(inp, 32)
            mstore(p, 64)
            mstore(add(p, 32), 1)
            mstore(add(p, 64), 64)
            mstore(add(p, 96), hi)
            mstore(add(p, 128), lo)
            mstore8(add(p, 160), 1)
            mstore(add(p, 161), PH)
            mstore(add(p, 193), PL)
            s := staticcall(gas(), 0x05, p, 225, add(out, 32), 64)
        }
        require(s, "modexp");
    }

    function _bswap(uint256 x) private pure returns (uint256 r) {
        r = x;
        r = ((r & 0xFF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00) >> 8)
            | ((r & 0x00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF00FF) << 8);
        r = ((r & 0xFFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000) >> 16)
            | ((r & 0x0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF0000FFFF) << 16);
        r = ((r & 0xFFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000) >> 32)
            | ((r & 0x00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF00000000FFFFFFFF) << 32);
        r = ((r & 0xFFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF0000000000000000) >> 64)
            | ((r & 0x0000000000000000FFFFFFFFFFFFFFFF0000000000000000FFFFFFFFFFFFFFFF) << 64);
        r = (r >> 128) | (r << 128);
    }

    // ------------------------------------------------------------------ Fp2

    function _f2add(Fp2 memory a, Fp2 memory b) private pure returns (Fp2 memory) {
        return Fp2(_add(a.c0, b.c0), _add(a.c1, b.c1));
    }

    function _f2mul(Fp2 memory a, Fp2 memory b) private view returns (Fp2 memory) {
        Fp memory t0 = _mul(a.c0, b.c0);
        Fp memory t1 = _mul(a.c1, b.c1);
        Fp memory s = _mul(_add(a.c0, a.c1), _add(b.c0, b.c1));
        return Fp2(_sub(t0, t1), _sub(_sub(s, t0), t1));
    }

    function _f2sqr(Fp2 memory a) private view returns (Fp2 memory) {
        Fp memory c0 = _mul(_add(a.c0, a.c1), _sub(a.c0, a.c1));
        Fp memory t = _mul(a.c0, a.c1);
        return Fp2(c0, _add(t, t));
    }

    function _f2eq(Fp2 memory a, Fp2 memory b) private pure returns (bool) {
        return _eq(a.c0, b.c0) && _eq(a.c1, b.c1);
    }

    function _fpSqrt(Fp memory a) private view returns (bool ok, Fp memory r) {
        r = _mexp(a, 48, ESQ_H, ESQ_L);
        ok = _eq(_sqr(r), a);
    }

    /// complex method: alpha = sqrt(a0^2 + a1^2); x0 = sqrt((a0 +- alpha)/2); x1 = a1 / (2 x0); checked by squaring
    function _f2sqrt(Fp2 memory a) private view returns (bool ok, Fp2 memory r) {
        if (a.c1.h == 0 && a.c1.l == 0) {
            Fp memory s;
            (ok, s) = _fpSqrt(a.c0);
            if (ok) return (true, Fp2(s, Fp(0, 0)));
            (ok, s) = _fpSqrt(_neg(a.c0)); // (s*u)^2 = -s^2 = a0
            if (ok) return (true, Fp2(Fp(0, 0), s));
            return (false, r);
        }
        Fp memory alpha;
        (ok, alpha) = _fpSqrt(_add(_sqr(a.c0), _sqr(a.c1)));
        if (!ok) return (false, r);
        Fp memory x0;
        (ok, x0) = _fpSqrt(_halve(_add(a.c0, alpha)));
        if (!ok) {
            (ok, x0) = _fpSqrt(_halve(_sub(a.c0, alpha)));
            if (!ok) return (false, r);
        }
        if (x0.h == 0 && x0.l == 0) return (false, r);
        Fp memory inv = _mexp(_add(x0, x0), 48, EINV_H, EINV_L);
        r = Fp2(x0, _mul(a.c1, inv));
        ok = _f2eq(_f2sqr(r), a);
    }

    // ------------------------------------------------------------------ Fp

    function _lt(Fp memory a, Fp memory b) private pure returns (bool) {
        return a.h < b.h || (a.h == b.h && a.l < b.l);
    }

    function _eq(Fp memory a, Fp memory b) private pure returns (bool) {
        return a.h == b.h && a.l == b.l;
    }

    function _gtHalf(Fp memory a) private pure returns (bool) {
        return a.h > HALF_H || (a.h == HALF_H && a.l > HALF_L);
    }

    function _add(Fp memory a, Fp memory b) private pure returns (Fp memory r) {
        unchecked {
            uint256 l = a.l + b.l;
            uint256 h = a.h + b.h + (l < a.l ? 1 : 0);
            if (h > PH || (h == PH && l >= PL)) {
                uint256 bw = l < PL ? 1 : 0;
                l = l - PL;
                h = h - PH - bw;
            }
            r = Fp(h, l);
        }
    }

    function _sub(Fp memory a, Fp memory b) private pure returns (Fp memory r) {
        unchecked {
            uint256 l = a.l - b.l;
            uint256 h = a.h - b.h - (a.l < b.l ? 1 : 0);
            if (_lt(a, b)) {
                uint256 l2 = l + PL;
                h = h + PH + (l2 < l ? 1 : 0);
                l = l2;
            }
            r = Fp(h, l);
        }
    }

    function _neg(Fp memory a) private pure returns (Fp memory) {
        if (a.h == 0 && a.l == 0) return Fp(0, 0);
        return _sub(Fp(0, 0), a);
    }

    function _halve(Fp memory a) private pure returns (Fp memory r) {
        unchecked {
            uint256 h = a.h;
            uint256 l = a.l;
            if (l & 1 == 1) {
                uint256 nl = l + PL;
                h = h + PH + (nl < l ? 1 : 0);
                l = nl;
            }
            r = Fp(h >> 1, (l >> 1) | (h << 255));
        }
    }

    function _sqr(Fp memory a) private view returns (Fp memory) {
        return _mexp(a, 1, 2, 0);
    }

    /// ab = ((a+b)^2 - (a-b)^2) / 4
    function _mul(Fp memory a, Fp memory b) private view returns (Fp memory) {
        Fp memory d = _sub(_sqr(_add(a, b)), _sqr(_sub(a, b)));
        return _halve(_halve(d));
    }

    /// base^e mod p through 0x05. esz == 1: exponent is the byte `eh`. esz == 48: exponent is (eh << 256 | el).
    function _mexp(Fp memory a, uint256 esz, uint256 eh, uint256 el) private view returns (Fp memory r) {
        r = Fp(0, 0);
        bool ok;
        assembly {
            let m := mload(0x40)
            mstore(m, 48)
            mstore(add(m, 32), esz)
            mstore(add(m, 64), 48)
            let b := add(m, 96)
            mstore(b, shl(128, mload(a)))
            mstore(add(b, 16), mload(add(a, 32)))
            let e := add(b, 48)
            switch esz
            case 1 { mstore8(e, eh) }
            default {
                mstore(e, shl(128, eh))
                mstore(add(e, 16), el)
            }
            let mo := add(e, esz)
            mstore(mo, shl(128, PH))
            mstore(add(mo, 16), PL)
            let out := add(mo, 48)
            ok := staticcall(gas(), 0x05, m, add(192, esz), out, 48)
            mstore(r, shr(128, mload(out)))
            mstore(add(r, 32), mload(add(out, 16)))
        }
        require(ok, "modexp");
    }

    /// big-endian 48 bytes at c[off..off+48)
    function _load(bytes memory c, uint256 off) private pure returns (Fp memory r) {
        r = Fp(0, 0);
        assembly {
            let p := add(add(c, 32), off)
            mstore(r, shr(128, mload(p)))
            mstore(add(r, 32), mload(add(p, 16)))
        }
    }

    /// EIP-2537 field element: 64 bytes big-endian
    function _store(bytes memory out, uint256 off, Fp memory a) private pure {
        assembly {
            let p := add(add(out, 32), off)
            mstore(p, mload(a))
            mstore(add(p, 32), mload(add(a, 32)))
        }
    }
}
