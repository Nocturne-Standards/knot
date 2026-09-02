// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Leon Frenzel
//
// In-browser port of knot-tool `mock_ledger.rs` + Lab `/api/*` mock router.
// Used when the page has no live tool token (static / Cloudflare Pages Lab).
// Crypto digests are deterministic UI stand-ins - not production BIP39 fingerprints.

(function (global) {
  "use strict";

  const MOCK_CHAIN_ID = 2;

  /** Tiny deterministic 32-byte digest (not SHA-256 / not production). */
  function mockDigest32(parts) {
    const s = parts.join("|");
    const out = new Uint8Array(32);
    let h1 = 0x811c9dc5 >>> 0;
    let h2 = 0x01000193 >>> 0;
    for (let i = 0; i < s.length; i++) {
      const c = s.charCodeAt(i);
      h1 ^= c;
      h1 = Math.imul(h1, 0x01000193) >>> 0;
      h2 ^= c + i;
      h2 = Math.imul(h2 ^ (h1 >>> 7), 0x85ebca6b) >>> 0;
      out[i % 32] ^= (h1 >>> (i % 24)) & 0xff;
      out[(i * 7) % 32] ^= (h2 >>> (i % 16)) & 0xff;
    }
    for (let i = 0; i < 32; i++) {
      out[i] ^= (h1 >>> (i % 8)) & 0xff;
      h1 = Math.imul(h1 ^ out[i], 0x27d4eb2d) >>> 0;
    }
    return out;
  }

  function bytesToHex(bytes) {
    return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  }

  function hexToBytes32(hex) {
    const h = String(hex || "").replace(/^0x/i, "");
    if (h.length !== 64 || /[^0-9a-f]/i.test(h)) {
      throw new Error("target must be 32-byte hex");
    }
    const out = new Uint8Array(32);
    for (let i = 0; i < 32; i++) out[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16);
    return out;
  }

  function hexToBytes(hex) {
    const h = String(hex || "").replace(/^0x/i, "");
    if (!h) return new Uint8Array(0);
    if (h.length % 2 !== 0 || /[^0-9a-f]/i.test(h)) throw new Error("invalid hex");
    const out = new Uint8Array(h.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16);
    return out;
  }

  // Compact alphabet for display-only "base58" PKs (not full Bitcoin base58).
  const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  function bytesToB58ish(bytes) {
    let out = "";
    for (let i = 0; i < bytes.length; i++) {
      out += B58[bytes[i] % 58];
    }
    return out;
  }

  /** Stable fake 96-byte BLS pk material from a name (Lab-only). */
  function pkBytesForName(name) {
    const d = mockDigest32(["pk", String(name)]);
    const out = new Uint8Array(96);
    for (let i = 0; i < 96; i++) out[i] = d[i % 32] ^ ((i * 17 + name.length) & 0xff);
    return out;
  }

  function pkKey(bytes) {
    return bytesToHex(bytes);
  }

  function samePk(a, b) {
    if (!a || !b || a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
    return true;
  }

  const MNEMONIC_WORDS = [
    "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract",
    "absurd", "abuse", "access", "accident", "account", "accuse", "achieve", "acid",
    "acoustic", "acquire", "across", "act", "action", "actor", "actress", "actual",
    "adapt", "add", "addict", "address", "adjust", "admit", "adult", "advance",
    "advice", "aerobic", "affair", "afford", "afraid", "again", "age", "agent",
    "agree", "ahead", "aim", "air", "airport", "aisle", "alarm", "album",
    "alcohol", "alert", "alien", "all", "alley", "allow", "almost", "alone",
    "alpha", "already", "also", "alter", "always", "amateur", "amazing", "among",
    "amount", "amused", "analyst", "anchor", "ancient", "anger", "angle", "angry",
    "animal", "ankle", "announce", "annual", "another", "answer", "antenna", "antique",
    "anxiety", "any", "apart", "apology", "appear", "apple", "approve", "april",
    "area", "arena", "argue", "arm", "armed", "armor", "army", "around",
    "arrange", "arrest", "arrive", "arrow", "art", "artefact", "artist", "artwork",
    "ask", "aspect", "assault", "asset", "assist", "assume", "asthma", "athlete",
    "atom", "attack", "attend", "attitude", "attract", "auction", "audit", "august",
    "aunt", "author", "auto", "autumn", "average", "avocado", "avoid", "awake",
    "aware", "away", "awesome", "awful", "awkward", "axis", "baby", "bachelor",
    "bacon", "badge", "bag", "balance", "balcony", "ball", "bamboo", "banana",
    "banner", "bar", "barely", "bargain", "barrel", "base", "basic", "basket",
    "battle", "beach", "bean", "beauty", "because", "become", "beef", "before",
    "begin", "behave", "behind", "believe", "below", "belt", "bench", "benefit",
    "best", "betray", "better", "between", "beyond", "bicycle", "bid", "bike",
    "bind", "biology", "bird", "birth", "bitter", "black", "blade", "blame",
    "blanket", "blast", "bleak", "bless", "blind", "blood", "blossom", "blouse",
    "blue", "blur", "blush", "board", "boat", "body", "boil", "bomb",
    "bone", "bonus", "book", "boost", "border", "boring", "borrow", "boss",
    "bottom", "bounce", "box", "boy", "bracket", "brain", "brand", "brass",
    "brave", "bread", "breeze", "brick", "bridge", "brief", "bright", "bring",
    "brisk", "broccoli", "broken", "bronze", "broom", "brother", "brown", "brush",
    "bubble", "buddy", "budget", "buffalo", "build", "bulb", "bulk", "bullet",
    "bundle", "bunker", "burden", "burger", "burst", "bus", "business", "busy",
  ];

  function digestMnemonic(digest) {
    const words = [];
    for (let i = 0; i < 24; i++) {
      const idx = digest[i % 32] ^ digest[(i * 3) % 32];
      words.push(MNEMONIC_WORDS[idx % MNEMONIC_WORDS.length]);
    }
    return words.join(" ");
  }

  function digestSafetyNumber(digest) {
    const parts = [];
    for (let i = 0; i < 32; i += 2) {
      const n = (digest[i] << 8) | digest[i + 1];
      parts.push(String(n).padStart(5, "0"));
    }
    // Group like production display (pairs with spaces).
    return parts.join(" ").replace(/(\d{5} \d{5}) /g, "$1  ");
  }

  function intentDigest(intent) {
    return mockDigest32([
      "intent",
      String(intent.chain_id),
      String(intent.committee_id),
      String(intent.nonce),
      bytesToHex(intent.target),
      intent.function_name,
      bytesToHex(intent.call_args),
      String(intent.deadline),
    ]);
  }

  /** Fingerprint of an arbitrary message — same output shape as the Rust
   * tool's quorum-preview response (digest_hex/mnemonic/safety_number), not
   * its bytes. UI stand-in only, see file header. */
  function messageFingerprintOut(msg) {
    const digest = mockDigest32(["msg", bytesToHex(msg)]);
    return {
      msg_hex: `0x${bytesToHex(msg)}`,
      digest_hex: `0x${bytesToHex(digest)}`,
      digest_mnemonic: digestMnemonic(digest),
      digest_safety_number: digestSafetyNumber(digest),
    };
  }

  /** Digest over a proposed change_account call — same output shape as the
   * Rust tool's change-account-preview response, not its bytes. UI stand-in
   * only, see file header. */
  function changeAccountDigest({ accountId, nonce, newMembers, newThreshold }) {
    return mockDigest32([
      "change_account",
      String(accountId),
      String(nonce),
      newMembers.map(bytesToHex).join(","),
      String(newThreshold),
    ]);
  }

  function multiSignerServeNote(signers) {
    return signers.length > 1
      ? "Prefer one signer identity per serve process — run separate serve instances per member when possible."
      : null;
  }

  class MockLedger {
    constructor() {
      this.accounts = new Map();
      this.proposals = new Map();
      this.nextAccountId = 0;
      this.nextProposalId = 0;
    }

    next_account_id() {
      return this.nextAccountId;
    }

    next_proposal_id() {
      return this.nextProposalId;
    }

    create_account(members, threshold) {
      if (!members || !members.length) throw new Error("members must be non-empty");
      // Match Rust u32 + 1..=members.len() (reject NaN / non-integers / negatives).
      if (
        typeof threshold !== "number" ||
        !Number.isInteger(threshold) ||
        threshold < 1 ||
        threshold > members.length
      ) {
        throw new Error("threshold must be between 1 and committee size");
      }
      for (let i = 0; i < members.length; i++) {
        for (let j = i + 1; j < members.length; j++) {
          if (samePk(members[i], members[j])) {
            throw new Error("duplicate member public key");
          }
        }
      }
      const id = this.nextAccountId++;
      this.accounts.set(id, {
        threshold,
        nonce: 0,
        members: members.map((m) => new Uint8Array(m)),
        timelock_blocks: 0,
        pending_execute_at: 0,
        pending_timelock: null,
      });
      return id;
    }

    account(id) {
      const a = this.accounts.get(Number(id));
      return a ? { ...a, members: a.members.map((m) => new Uint8Array(m)) } : null;
    }

    account_meta(id) {
      const a = this.accounts.get(Number(id));
      if (!a) return null;
      return {
        nonce: a.nonce,
        threshold: a.threshold,
        member_count: a.members.length,
        timelock_blocks: a.timelock_blocks || 0,
        pending_execute_at: a.pending_execute_at || 0,
      };
    }

    create_proposal(registryAccountId, target, functionName, callArgs, deadline, chainId) {
      const account = this.accounts.get(Number(registryAccountId));
      if (!account) throw new Error(`unknown registry account ${registryAccountId}`);
      const nonce = account.nonce;
      const digest = intentDigest({
        chain_id: chainId,
        committee_id: Number(registryAccountId),
        nonce,
        target,
        function_name: functionName,
        call_args: callArgs,
        deadline,
      });
      const id = this.nextProposalId++;
      this.proposals.set(id, {
        id,
        status: "open",
        registry_account_id: Number(registryAccountId),
        chain_id: chainId,
        nonce,
        target: new Uint8Array(target),
        function_name: functionName,
        call_args: new Uint8Array(callArgs),
        deadline,
        digest,
        approvals: [],
        execute_at: 0,
      });
      return id;
    }

    proposal(id) {
      const p = this.proposals.get(Number(id));
      if (!p) return null;
      return {
        ...p,
        target: new Uint8Array(p.target),
        call_args: new Uint8Array(p.call_args),
        digest: new Uint8Array(p.digest),
        approvals: p.approvals.map((a) => new Uint8Array(a)),
      };
    }

    approve(id, memberPk) {
      const proposal = this.proposals.get(Number(id));
      if (!proposal) throw new Error(`no such proposal ${id}`);
      if (proposal.status !== "open") throw new Error("proposal is not open");
      const account = this.accounts.get(proposal.registry_account_id);
      if (!account) throw new Error("unknown registry account for proposal");
      if (!account.members.some((m) => samePk(m, memberPk))) {
        throw new Error("signer is not a member of the proposal's registry account");
      }
      if (proposal.approvals.some((a) => samePk(a, memberPk))) {
        throw new Error("signer has already approved this proposal");
      }
      proposal.approvals.push(new Uint8Array(memberPk));
    }

    finalize(id) {
      const proposal = this.proposals.get(Number(id));
      if (!proposal) throw new Error(`no such proposal ${id}`);
      if (proposal.status !== "open") throw new Error("proposal is not open");
      const account = this.accounts.get(proposal.registry_account_id);
      if (!account) throw new Error("unknown registry account for proposal");
      if (proposal.nonce !== account.nonce) throw new Error("proposal nonce is stale");
      if (proposal.approvals.length < account.threshold) {
        throw new Error(
          `finalize: quorum not met (approvals=${proposal.approvals.length}, threshold=${account.threshold})`
        );
      }
      if (account.nonce >= Number.MAX_SAFE_INTEGER) {
        throw new Error("account nonce overflow");
      }
      account.nonce += 1;
      const delay = account.timelock_blocks || 0;
      if (delay === 0) {
        proposal.status = "finalized";
      } else {
        proposal.status = "queued";
        proposal.execute_at = delay;
      }
    }

    execute(id) {
      const proposal = this.proposals.get(Number(id));
      if (!proposal) throw new Error(`no such proposal ${id}`);
      if (proposal.status !== "queued") throw new Error("proposal is not queued");
      proposal.status = "finalized";
    }

    cancel_proposal(id) {
      const proposal = this.proposals.get(Number(id));
      if (!proposal) throw new Error(`no such proposal ${id}`);
      if (proposal.status !== "queued") throw new Error("proposal is not queued");
      proposal.status = "cancelled";
    }

    set_timelock(id, blocks) {
      const account = this.accounts.get(Number(id));
      if (!account) throw new Error(`unknown registry account ${id}`);
      if (!account.timelock_blocks) {
        account.timelock_blocks = Number(blocks);
        account.nonce += 1;
        return;
      }
      account.pending_timelock = Number(blocks);
      account.pending_execute_at = account.timelock_blocks;
      account.nonce += 1;
    }

    execute_pending(id) {
      const account = this.accounts.get(Number(id));
      if (!account) throw new Error(`unknown registry account ${id}`);
      if (!account.pending_execute_at) throw new Error("no pending change");
      if (account.pending_timelock != null) {
        account.timelock_blocks = account.pending_timelock;
        account.pending_timelock = null;
      }
      account.pending_execute_at = 0;
    }

    cancel_pending(id) {
      const account = this.accounts.get(Number(id));
      if (!account) throw new Error(`unknown registry account ${id}`);
      if (!account.pending_execute_at) throw new Error("no pending change");
      account.pending_timelock = null;
      account.pending_execute_at = 0;
    }
  }

  class MockLabStore {
    constructor() {
      this.ledger = new MockLedger();
      this.identities = []; // { name, pk_bytes, pk_base58, pk_only }
    }

    listIdentities() {
      return this.identities.map((i) => ({
        name: i.name,
        pk_base58: i.pk_base58,
        pk_only: !!i.pk_only,
      }));
    }

    createIdentity(name) {
      const n = String(name || "").trim();
      if (!n) throw new Error("name required");
      if (this.identities.some((i) => i.name === n)) {
        throw new Error(`identity '${n}' already exists`);
      }
      const pk_bytes = pkBytesForName(n);
      const row = {
        name: n,
        pk_bytes,
        pk_base58: bytesToB58ish(pk_bytes),
        pk_only: false,
      };
      this.identities.push(row);
      return { name: row.name, pk_base58: row.pk_base58, pk_only: false };
    }

    findIdentity(name) {
      return this.identities.find((i) => i.name === name) || null;
    }

    okSubmit(log, txHash) {
      return {
        log,
        outcome: "ok",
        tx_status: "confirmed",
        tx_hash: txHash,
        panic_line: null,
        note: "DEMO_MODE=mock - no chain submit (frontend MockLedger)",
      };
    }
  }

  const store = new MockLabStore();

  function httpError(status, message) {
    const err = new Error(`${status}: ${message}`);
    err.status = status;
    err.body = message;
    throw err;
  }

  function resolveSignerPks(names) {
    return names.map((name) => {
      const id = store.findIdentity(name);
      if (!id) httpError(400, `no identity named '${name}'`);
      return id.pk_bytes;
    });
  }

  function ensureSignersAreMembers(accountId, signerPks) {
    const account = store.ledger.account(accountId);
    if (!account) httpError(400, `unknown registry account ${accountId}`);
    for (const pk of signerPks) {
      if (!account.members.some((m) => samePk(m, pk))) {
        httpError(403, "Signer is not a committee member.");
      }
    }
    return account;
  }

  function parseBody(opts) {
    if (!opts || opts.body == null || opts.body === "") return {};
    if (typeof opts.body === "string") return JSON.parse(opts.body);
    return opts.body;
  }

  /**
   * Lab `/api/*` router mirroring mock DEMO_MODE responses.
   * @returns {Promise<any>}
   */
  async function mockApi(path, opts = {}) {
    const method = (opts.method || "GET").toUpperCase();
    const url = String(path || "");
    const q = url.split("?")[0];

    // Tiny async yield so callers keep await shape.
    await Promise.resolve();

    if (q === "/api/setup/status" && method === "GET") {
      return {
        store_path: "(frontend-mock)",
        identities_count: store.identities.length,
        collector_configured: false,
        collector_user_configured: false,
        demo_mode: "mock",
      };
    }

    if (q === "/api/identities" && method === "GET") {
      return store.listIdentities();
    }

    if (q === "/api/identities" && method === "POST") {
      try {
        return store.createIdentity(parseBody(opts).name);
      } catch (e) {
        httpError(400, e.message);
      }
    }

    if (q === "/api/account/create" && method === "POST") {
      const body = parseBody(opts);
      const names = body.members || [];
      const thresholdRaw = body.threshold;
      const threshold =
        typeof thresholdRaw === "number"
          ? thresholdRaw
          : typeof thresholdRaw === "string" && thresholdRaw.trim() !== ""
            ? Number(thresholdRaw)
            : NaN;
      const memberBytes = [];
      for (const name of names) {
        const id = store.findIdentity(name);
        if (!id) httpError(400, `no identity named '${name}'`);
        memberBytes.push(id.pk_bytes);
      }
      try {
        const id = store.ledger.create_account(memberBytes, threshold);
        return store.okSubmit(
          `mock: create_account id=${id} threshold=${threshold}`,
          `mock-create-account-${id}`
        );
      } catch (e) {
        httpError(400, e.message);
      }
    }

    if (q === "/api/account/next-id" && method === "GET") {
      return store.ledger.next_account_id();
    }

    let m = q.match(/^\/api\/account\/(\d+)$/);
    if (m && method === "GET") {
      const acct = store.ledger.account(Number(m[1]));
      if (!acct) return null;
      return {
        threshold: acct.threshold,
        nonce: acct.nonce,
        timelock_blocks: acct.timelock_blocks || 0,
        pending_execute_at: acct.pending_execute_at || 0,
        members: acct.members.map((pk) => bytesToB58ish(pk)),
      };
    }

    m = q.match(/^\/api\/account\/(\d+)\/meta$/);
    if (m && method === "GET") {
      const meta = store.ledger.account_meta(Number(m[1]));
      if (!meta) return null;
      return {
        threshold: meta.threshold,
        nonce: meta.nonce,
        members_len: meta.member_count,
        timelock_blocks: meta.timelock_blocks || 0,
        pending_execute_at: meta.pending_execute_at || 0,
      };
    }

    if (q === "/api/proposal/create" && method === "POST") {
      const body = parseBody(opts);
      try {
        const target = hexToBytes32(body.target);
        const callArgs = hexToBytes(body.args_hex || "");
        const before = store.ledger.next_proposal_id();
        const id = store.ledger.create_proposal(
          Number(body.account),
          target,
          String(body.function || "noop"),
          callArgs,
          Number(body.deadline) || 0,
          MOCK_CHAIN_ID
        );
        return {
          ...store.okSubmit(
            `mock: propose id=${id} account=${body.account}`,
            `mock-propose-${id}`
          ),
          allocated_id_hint: before,
        };
      } catch (e) {
        httpError(400, e.message);
      }
    }

    if (q === "/api/proposal/next-id" && method === "GET") {
      return store.ledger.next_proposal_id();
    }

    m = q.match(/^\/api\/proposal\/(\d+)\/preview$/);
    if (m && method === "GET") {
      const p = store.ledger.proposal(Number(m[1]));
      if (!p) httpError(400, `proposal ${m[1]} not found`);
      if (p.status !== "open") httpError(400, "proposal is not Open");
      return {
        digest_hex: `0x${bytesToHex(p.digest)}`,
        digest_mnemonic: digestMnemonic(p.digest),
        digest_safety_number: digestSafetyNumber(p.digest),
        chain_id: p.chain_id,
        committee_id: p.registry_account_id,
        nonce: p.nonce,
        target_hex: `0x${bytesToHex(p.target)}`,
        function_name: p.function_name,
        call_args_hex: `0x${bytesToHex(p.call_args)}`,
        deadline: p.deadline,
      };
    }

    m = q.match(/^\/api\/proposal\/(\d+)\/approve$/);
    if (m && method === "POST") {
      const body = parseBody(opts);
      if (!body.confirm) {
        httpError(400, "confirm required - call /preview first, then POST with confirm:true");
      }
      const id = Number(m[1]);
      const p = store.ledger.proposal(id);
      if (!p) httpError(400, `proposal ${id} not found`);
      if (p.status !== "open") httpError(400, `proposal ${id} is not Open`);
      const ident = store.findIdentity(body.signer);
      if (!ident) httpError(400, `no identity named '${body.signer}'`);
      if (ident.pk_only) httpError(400, `identity '${body.signer}' is pk-only`);
      try {
        store.ledger.approve(id, ident.pk_bytes);
      } catch (e) {
        httpError(400, e.message);
      }
      return {
        ...store.okSubmit(
          `mock: approve proposal ${id} by ${body.signer}`,
          `mock-approve-${id}`
        ),
        intent: {
          chain_id: p.chain_id,
          committee_id: p.registry_account_id,
          nonce: p.nonce,
          target: `0x${bytesToHex(p.target)}`,
          function: p.function_name,
          call_args_hex: `0x${bytesToHex(p.call_args)}`,
          deadline: p.deadline,
          digest_hex: `0x${bytesToHex(p.digest)}`,
          digest_mnemonic: digestMnemonic(p.digest),
          digest_safety_number: digestSafetyNumber(p.digest),
        },
      };
    }

    m = q.match(/^\/api\/proposal\/(\d+)\/finalize$/);
    if (m && method === "POST") {
      const id = Number(m[1]);
      try {
        store.ledger.finalize(id);
      } catch (e) {
        httpError(400, e.message);
      }
      return store.okSubmit(`mock: finalize proposal ${id}`, `mock-finalize-${id}`);
    }

    m = q.match(/^\/api\/proposal\/(\d+)\/execute$/);
    if (m && method === "POST") {
      const id = Number(m[1]);
      try {
        store.ledger.execute(id);
      } catch (e) {
        httpError(400, e.message);
      }
      return store.okSubmit(`mock: execute proposal ${id}`, `mock-execute-${id}`);
    }

    m = q.match(/^\/api\/proposal\/(\d+)\/cancel$/);
    if (m && method === "POST") {
      const id = Number(m[1]);
      try {
        store.ledger.cancel_proposal(id);
      } catch (e) {
        httpError(400, e.message);
      }
      return store.okSubmit(`mock: cancel proposal ${id}`, `mock-cancel-${id}`);
    }

    m = q.match(/^\/api\/account\/(\d+)\/set-timelock$/);
    if (m && method === "POST") {
      const id = Number(m[1]);
      const body = parseBody(opts);
      try {
        store.ledger.set_timelock(id, body.blocks);
      } catch (e) {
        httpError(400, e.message);
      }
      return store.okSubmit(`mock: set_timelock account ${id}`, `mock-set-timelock-${id}`);
    }

    m = q.match(/^\/api\/account\/(\d+)\/execute-pending$/);
    if (m && method === "POST") {
      const id = Number(m[1]);
      try {
        store.ledger.execute_pending(id);
      } catch (e) {
        httpError(400, e.message);
      }
      return store.okSubmit(`mock: execute_pending account ${id}`, `mock-execute-pending-${id}`);
    }

    m = q.match(/^\/api\/account\/(\d+)\/cancel-pending$/);
    if (m && method === "POST") {
      const id = Number(m[1]);
      try {
        store.ledger.cancel_pending(id);
      } catch (e) {
        httpError(400, e.message);
      }
      return store.okSubmit(`mock: cancel_pending account ${id}`, `mock-cancel-pending-${id}`);
    }

    m = q.match(/^\/api\/proposal\/(\d+)$/);
    if (m && method === "GET") {
      const id = Number(m[1]);
      const p = store.ledger.proposal(id);
      if (!p) return null;
      return {
        id,
        status:
          p.status === "finalized"
            ? "Executed"
            : p.status === "queued"
              ? "Queued"
              : p.status === "cancelled"
                ? "Cancelled"
                : "Open",
        registry_account_id: p.registry_account_id,
        chain_id: p.chain_id,
        nonce: p.nonce,
        target: `0x${bytesToHex(p.target)}`,
        function: p.function_name,
        call_args_hex: `0x${bytesToHex(p.call_args)}`,
        deadline: p.deadline,
        execute_at: p.execute_at || 0,
        digest_hex: `0x${bytesToHex(p.digest)}`,
        approvals_len: p.approvals.length,
        approvals: p.approvals.map((a) => bytesToB58ish(a)),
      };
    }

    if (q === "/api/quorum/preview" && method === "POST") {
      const body = parseBody(opts);
      const accountId = Number(body.account);
      const signers = body.signers || [];
      const signerPks = resolveSignerPks(signers);
      ensureSignersAreMembers(accountId, signerPks);
      const msg = new TextEncoder().encode(String(body.msg || ""));
      return {
        account_id: accountId,
        signers,
        note: multiSignerServeNote(signers),
        ...messageFingerprintOut(msg),
      };
    }

    if (q === "/api/quorum/submit" && method === "POST") {
      const body = parseBody(opts);
      if (!body.confirm) {
        httpError(400, "Confirm required — call preview first, then POST with confirm:true.");
      }
      // Matches the real DEMO_MODE=mock server: verify_quorum submits a real
      // chain call, so it is rejected in mock mode too (nothing to port here).
      httpError(501, "This action requires live testnet mode (DEMO_MODE=testnet).");
    }

    if (q === "/api/change-account/preview" && method === "POST") {
      const body = parseBody(opts);
      const accountId = Number(body.account);
      const account = store.ledger.account(accountId);
      if (!account) httpError(400, `unknown registry account ${accountId}`);
      const newMemberNames = body.new_members || [];
      const newMembers = resolveSignerPks(newMemberNames);
      const newThreshold = Number(body.new_threshold);
      const signers = body.signers || [];
      const digest = changeAccountDigest({
        accountId,
        nonce: account.nonce,
        newMembers,
        newThreshold,
      });
      return {
        account_id: accountId,
        nonce: account.nonce,
        new_members: newMemberNames,
        new_threshold: newThreshold,
        signers,
        digest_hex: `0x${bytesToHex(digest)}`,
        digest_mnemonic: digestMnemonic(digest),
        digest_safety_number: digestSafetyNumber(digest),
        note: multiSignerServeNote(signers),
      };
    }

    if (q === "/api/change-account/submit" && method === "POST") {
      const body = parseBody(opts);
      if (!body.confirm) {
        httpError(400, "Confirm required — call preview first, then POST with confirm:true.");
      }
      // Matches the real DEMO_MODE=mock server: change_account submits a real
      // chain call, so it is rejected in mock mode too (nothing to port here).
      httpError(501, "This action requires live testnet mode (DEMO_MODE=testnet).");
    }

    httpError(404, `frontend mock: unsupported ${method} ${q}`);
  }

  /** Lightweight self-check of the quorum/change-account router paths.
   * Swaps store.identities/ledger out for isolated fixtures and restores
   * them in `finally` — this runs during live app boot, so it must never
   * touch the real demo's identities/councils, even on failure. */
  async function selfTestRouter() {
    const savedIdentities = store.identities;
    const savedLedger = store.ledger;
    store.identities = [];
    store.ledger = new MockLedger();
    try {
      return await selfTestRouterBody();
    } finally {
      store.identities = savedIdentities;
      store.ledger = savedLedger;
    }
  }

  async function selfTestRouterBody() {
    store.createIdentity("selftest-a");
    store.createIdentity("selftest-b");
    const acct = await mockApi("/api/account/create", {
      method: "POST",
      body: JSON.stringify({ members: ["selftest-a", "selftest-b"], threshold: 2 }),
    });
    const accountId = Number(acct.log.match(/id=(\d+)/)[1]);

    const qp = await mockApi("/api/quorum/preview", {
      method: "POST",
      body: JSON.stringify({ account: accountId, msg: "hello", signers: ["selftest-a"] }),
    });
    if (!qp.digest_hex || !qp.digest_mnemonic) throw new Error("quorum preview shape");
    try {
      await mockApi("/api/quorum/submit", {
        method: "POST",
        body: JSON.stringify({ account: accountId, msg: "hello", signers: ["selftest-a"], confirm: true }),
      });
      throw new Error("quorum submit should be rejected in mock mode");
    } catch (e) {
      if (!/live testnet mode/i.test(e.message)) throw e;
    }

    const cap = await mockApi("/api/change-account/preview", {
      method: "POST",
      body: JSON.stringify({
        account: accountId,
        new_members: ["selftest-a"],
        new_threshold: 1,
        signers: ["selftest-a", "selftest-b"],
      }),
    });
    if (!cap.digest_hex || !cap.digest_mnemonic) throw new Error("change-account preview shape");
    if (!cap.note) throw new Error("change-account preview multi-signer note");
    try {
      await mockApi("/api/change-account/submit", {
        method: "POST",
        body: JSON.stringify({
          account: accountId,
          new_members: ["selftest-a"],
          new_threshold: 1,
          signers: ["selftest-a"],
          confirm: true,
        }),
      });
      throw new Error("change-account submit should be rejected in mock mode");
    } catch (e) {
      if (!/live testnet mode/i.test(e.message)) throw e;
    }

    const tl = await mockApi(`/api/account/${accountId}/set-timelock`, {
      method: "POST",
      body: JSON.stringify({ blocks: 5 }),
    });
    if (tl.outcome === "panic") throw new Error("set-timelock should succeed in mock");
    const afterTl = await mockApi(`/api/account/${accountId}`);
    if (!afterTl || afterTl.timelock_blocks !== 5) throw new Error("account delay after set-timelock");

    const created = await mockApi("/api/proposal/create", {
      method: "POST",
      body: JSON.stringify({
        account: accountId,
        target: "0000000000000000000000000000000000000000000000000000000000000001",
        function: "noop",
        args_hex: "",
        deadline: 999999999,
      }),
    });
    const proposalId = created.allocated_id_hint;
    await mockApi(`/api/proposal/${proposalId}/approve`, {
      method: "POST",
      body: JSON.stringify({ signer: "selftest-a", confirm: true }),
    });
    await mockApi(`/api/proposal/${proposalId}/approve`, {
      method: "POST",
      body: JSON.stringify({ signer: "selftest-b", confirm: true }),
    });
    await mockApi(`/api/proposal/${proposalId}/finalize`, { method: "POST", body: "{}" });
    const queued = await mockApi(`/api/proposal/${proposalId}`);
    if (!queued || queued.status !== "Queued") throw new Error("finalize with delay should queue");
    await mockApi(`/api/proposal/${proposalId}/execute`, { method: "POST", body: "{}" });
    const executed = await mockApi(`/api/proposal/${proposalId}`);
    if (!executed || executed.status !== "Executed") throw new Error("execute should mark Executed");

    const created2 = await mockApi("/api/proposal/create", {
      method: "POST",
      body: JSON.stringify({
        account: accountId,
        target: "0000000000000000000000000000000000000000000000000000000000000001",
        function: "noop",
        args_hex: "",
        deadline: 999999999,
      }),
    });
    const cancelId = created2.allocated_id_hint;
    await mockApi(`/api/proposal/${cancelId}/approve`, {
      method: "POST",
      body: JSON.stringify({ signer: "selftest-a", confirm: true }),
    });
    await mockApi(`/api/proposal/${cancelId}/approve`, {
      method: "POST",
      body: JSON.stringify({ signer: "selftest-b", confirm: true }),
    });
    await mockApi(`/api/proposal/${cancelId}/finalize`, { method: "POST", body: "{}" });
    await mockApi(`/api/proposal/${cancelId}/cancel`, { method: "POST", body: "{}" });
    const cancelled = await mockApi(`/api/proposal/${cancelId}`);
    if (!cancelled || cancelled.status !== "Cancelled") throw new Error("cancel should mark Cancelled");

    return true;
  }

  /** Lightweight self-check of ledger rules, same cases as the Rust unit tests. */
  function selfTest() {
    const led = new MockLedger();
    const m = (b) => {
      const a = new Uint8Array(96);
      a.fill(b);
      return a;
    };
    const id = led.create_account([m(1), m(2), m(3)], 2);
    if (id !== 0) throw new Error("account id");
    const pid = led.create_proposal(id, new Uint8Array(32), "noop", new Uint8Array(0), 0, 2);
    if (pid !== 0) throw new Error("proposal id");
    try {
      led.approve(pid, m(9));
      throw new Error("non-member should fail");
    } catch (e) {
      if (!/member/i.test(e.message)) throw e;
    }
    led.approve(pid, m(1));
    try {
      led.finalize(pid);
      throw new Error("below threshold should fail");
    } catch (e) {
      if (!/quorum|threshold|approval/i.test(e.message)) throw e;
    }
    led.approve(pid, m(2));
    led.finalize(pid);
    if (led.proposal(pid).status !== "finalized") throw new Error("not finalized");
    if (led.account(id).nonce !== 1) throw new Error("nonce bump");

    led.set_timelock(id, 5);
    if (led.account(id).timelock_blocks !== 5) throw new Error("set_timelock delay");
    const pidQ = led.create_proposal(id, new Uint8Array(32), "noop", new Uint8Array(0), 0, 2);
    led.approve(pidQ, m(1));
    led.approve(pidQ, m(2));
    led.finalize(pidQ);
    if (led.proposal(pidQ).status !== "queued") throw new Error("delay should queue");
    if (!led.proposal(pidQ).execute_at) throw new Error("queued execute_at");
    led.execute(pidQ);
    if (led.proposal(pidQ).status !== "finalized") throw new Error("execute should finalize");

    const pidC = led.create_proposal(id, new Uint8Array(32), "noop", new Uint8Array(0), 0, 2);
    led.approve(pidC, m(1));
    led.approve(pidC, m(2));
    led.finalize(pidC);
    led.cancel_proposal(pidC);
    if (led.proposal(pidC).status !== "cancelled") throw new Error("cancel should cancel");
    return true;
  }

  global.MockLab = {
    MockLedger,
    store,
    mockApi,
    selfTest,
    selfTestRouter,
    MOCK_CHAIN_ID,
    pkBytesForName,
  };
})(typeof window !== "undefined" ? window : globalThis);
