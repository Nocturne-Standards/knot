// Plain JS, no build step. Token from index.html; signing is server-side.

const TOKEN = window.MULTISIG_TOOL_TOKEN;
let focusedNameTarget = null;

const STORY = {
  cast: ["alice", "bob", "carol"],
  members: "alice,bob,carol",
  threshold: 2,
  message: "approve payout #42",
  signers: "alice,bob",
  newMembers: "alice,carol",
  newThreshold: 2,
  // Mock-safe noop target: 31 zero bytes + 0x01 (32-byte ContractId hex).
  propTarget: "0000000000000000000000000000000000000000000000000000000000000001",
  propFunction: "noop",
  propArgsHex: "",
  propDeadline: 999999999,
  propSigner: "alice",
};

/** Five-beat proposals walkthrough (replaces old 7-chapter CHAPTERS). */
const BEATS = [
  {
    tab: "cast",
    beat: 1,
    step: "Beat 1 of 5 — Meet the cast",
    text: "Alice, Bob, and Carol are the treasury signers. The walkthrough created them if they were missing. Confirm all three show as signing identities, then Next.",
  },
  {
    tab: "council",
    beat: 2,
    step: "Beat 2 of 5 — Form the treasury",
    text: "Create a 2-of-3 council with alice,bob,carol. Press create_account — the new account id fills later beats automatically. Next unlocks after success.",
  },
  {
    tab: "check",
    beat: 3,
    step: "Beat 3 of 5 — Look it up",
    text: "Free-read the council. Query should show three members and threshold 2. No gas — just confirmation. Next unlocks after a successful query.",
  },
  {
    tab: "proposal",
    beat: 4,
    beatPhase: null,
    step: "Beat 4 of 5 — Propose & first approve",
    text: "Propose a harmless noop (target 00…01, fn noop, empty args, far deadline). Preview the fingerprint, confirm, then approve as Alice. Next unlocks after Alice’s approval.",
  },
  {
    tab: "proposal",
    beat: 5,
    beatPhase: "finalize",
    step: "Beat 5 of 5 — Threshold & finalize",
    text: "Switch to Bob, preview again if you like, confirm, approve — then finalize when approvals ≥ threshold. Finish after finalize confirms.",
  },
];

const DRAWER_TABS = new Set([
  "setup",
  "aggregate",
  "rotate",
  "payout",
  "party",
  "pm-resolve",
]);

const BROWSE = {
  setup: {
    step: "Developer · Setup",
    text: "Confirm your keystore is unlocked, create a signing identity, and check the collector URL configured server-side.",
  },
  cast: {
    step: "Beat 1 — Meet the cast",
    text: "Create named BLS keys that stay in this process. Foreign members can be imported as pk-only.",
  },
  council: {
    step: "Beat 2 — Form the treasury",
    text: "Register an M-of-N member set. Naming keys grants the creator no special power.",
  },
  check: {
    step: "Beat 3 — Look it up",
    text: "Confirm members, threshold, and nonce after creates or rotations — free reads, no gas.",
  },
  proposal: {
    step: "Beats 4–5 — Propose & finalize",
    text: "Propose → each signer approves → finalize. Coordination is the chain, not a file handoff.",
  },
  payout: {
    step: "Developer · Unsafe UTF-8",
    text: "Per-signature quorum with a raw message. Needs testnet in mock mode (501).",
  },
  aggregate: {
    step: "Developer · Aggregate verify",
    text: "Same story as a payout, but aggregated multisig — one pairing check on-chain. Needs testnet when mock.",
  },
  rotate: {
    step: "Developer · Rotate",
    text: "Current members authorize a new set. Needs testnet when mock.",
  },
  "pm-resolve": {
    step: "Developer · PM resolve",
    text: "Init a council-resolve blob, collect secure BLS partials, submit resolve. Needs testnet when mock.",
  },
  party: {
    step: "Developer · Party finder",
    text: "Shared roster signup. Needs testnet when mock.",
  },
};

let walkActive = false;
let walkIndex = 0;
/** Parallel to BEATS — Next stays disabled until the beat’s primary action succeeds. */
let beatDone = [false, false, false, false, false];
/** Soft walk gate: preview must succeed before confirm enables approve. */
let propPreviewShown = false;
let storyAccountId = null;
let storyProposalId = null;
let demoMode = "mock";
let statusThreshold = null;
let statusApprovals = null;

async function api(path, opts = {}) {
  const res = await fetch(path, {
    ...opts,
    headers: {
      "Content-Type": "application/json",
      "X-Multisig-Tool-Token": TOKEN,
      ...(opts.headers || {}),
    },
  });
  const text = await res.text();
  let body;
  try { body = JSON.parse(text); } catch { body = text; }
  if (!res.ok) {
    const msg = typeof body === "string" ? body : (body.error || JSON.stringify(body));
    throw new Error(`${res.status}: ${msg}`);
  }
  return body;
}

function splitNames(s) {
  return s.split(",").map((x) => x.trim()).filter(Boolean);
}

function setLog(id, text, ok = true) {
  const el = document.getElementById(id);
  el.textContent = text;
  el.className = "log " + (ok ? "status-ok" : "status-err");
}

function ensureTxStatusRow(logId) {
  const logEl = document.getElementById(logId);
  let row = logEl.nextElementSibling;
  if (!row || !row.classList.contains("tx-status")) {
    row = document.createElement("div");
    row.className = "tx-status";
    row.innerHTML =
      `<span class="tx-pill"></span>` +
      `<span class="tx-hash-wrap"><code class="tx-hash"></code>` +
      `<button type="button" class="tiny tx-copy" title="Copy tx hash">Copy</button></span>` +
      `<span class="tx-missing" hidden></span>`;
    logEl.after(row);
    row.querySelector(".tx-copy").addEventListener("click", async () => {
      const hash = row.querySelector(".tx-hash").textContent.trim();
      if (!hash) return;
      try {
        await navigator.clipboard.writeText(hash);
      } catch {
        prompt("Tx hash", hash);
      }
    });
  }
  return row;
}

function setTxStatus(logId, { status, hash, show }) {
  const row = ensureTxStatusRow(logId);
  if (!show) {
    row.hidden = true;
    return;
  }
  row.hidden = false;
  const pill = row.querySelector(".tx-pill");
  const st = (status || "unknown").toLowerCase();
  pill.textContent = st === "n/a" ? "n/a" : st;
  pill.className = "tx-pill " + (st === "n/a" ? "na" : st.replace(/[^a-z]/g, ""));
  const hashEl = row.querySelector(".tx-hash");
  const copyBtn = row.querySelector(".tx-copy");
  const missing = row.querySelector(".tx-missing");
  if (hash) {
    hashEl.textContent = hash;
    hashEl.hidden = false;
    copyBtn.hidden = false;
    missing.hidden = true;
  } else {
    hashEl.textContent = "";
    hashEl.hidden = true;
    copyBtn.hidden = true;
    missing.hidden = false;
    missing.textContent = "No tx hash parsed from wallet output";
  }
}

function formatSubmit(out) {
  const lines = [];
  lines.push(`outcome: ${out.outcome}`);
  if (out.tx_status) lines.push(`tx_status: ${out.tx_status}`);
  if (out.tx_hash) lines.push(`tx_hash: ${out.tx_hash}`);
  if (out.panic_line) lines.push(`panic: ${out.panic_line}`);
  if (out.note) lines.push(`note: ${out.note}`);
  if (out.check !== undefined && out.check !== null) lines.push(`check: ${out.check}`);
  if (out.diagnose) {
    const d = out.diagnose;
    lines.push(
      `diagnose: exists=${d.exists} threshold=${d.threshold} members_len=${d.members_len} ` +
      `member_matches=${d.member_matches} sigs_ok=${d.sigs_ok}` +
      (d.free_read_untrusted ? " [FREE-READ UNTRUSTED]" : "")
    );
  }
  lines.push("--- wallet log ---");
  lines.push(out.log || "");
  const ok = out.outcome !== "panic" && out.tx_status !== "failed";
  return { text: lines.join("\n"), ok };
}

function showSubmit(logId, out) {
  const f = formatSubmit(out);
  setLog(logId, f.text, f.ok);
  setTxStatus(logId, {
    show: true,
    status: out.tx_status || (f.ok ? "unknown" : "failed"),
    hash: out.tx_hash || null,
  });
}

function showError(logId, message) {
  setLog(logId, message, false);
  const hashMatch = message.match(/\b([a-fA-F0-9]{64})\b/);
  setTxStatus(logId, {
    show: true,
    status: "failed",
    hash: hashMatch ? hashMatch[1] : null,
  });
}

function appendName(inputId, name) {
  const el = document.getElementById(inputId);
  if (!el) return;
  const cur = splitNames(el.value);
  if (!cur.includes(name)) cur.push(name);
  el.value = cur.join(",");
}

function wireNameTargets() {
  document.querySelectorAll(".name-target").forEach((el) => {
    el.addEventListener("focus", () => { focusedNameTarget = el; });
  });
  document.querySelectorAll("input, textarea").forEach((el) => {
    el.setAttribute("dir", "ltr");
    el.style.unicodeBidi = "isolate";
  });
}

function setGuide(step, text) {
  document.getElementById("guide-step").textContent = step;
  document.getElementById("guide-narrator").textContent = text;
}

function showToast(message) {
  let el = document.getElementById("lab-toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "lab-toast";
    el.setAttribute("role", "status");
    Object.assign(el.style, {
      position: "fixed",
      bottom: "1.25rem",
      left: "50%",
      transform: "translateX(-50%)",
      zIndex: "40",
      maxWidth: "min(32rem, calc(100vw - 2rem))",
      padding: "0.65rem 1rem",
      borderRadius: "8px",
      background: "#1c2a33",
      color: "#f4f7f9",
      fontSize: "0.85rem",
      boxShadow: "0 8px 24px rgba(0,0,0,0.18)",
      transition: "opacity 200ms ease",
    });
    document.body.appendChild(el);
  }
  el.textContent = message;
  el.style.opacity = "1";
  el.hidden = false;
  clearTimeout(showToast._t);
  showToast._t = setTimeout(() => {
    el.style.opacity = "0";
    setTimeout(() => {
      el.hidden = true;
    }, 220);
  }, 4800);
}

/** Drawer endpoints return 501 in mock with DEMO_MODE=testnet in the body. */
function toastIfNeedsTestnet(err) {
  const msg = String(err && err.message ? err.message : err);
  if (demoMode === "mock" && msg.includes("DEMO_MODE=testnet")) {
    showToast("Needs testnet — set DEMO_MODE=testnet and restart for this panel.");
    return true;
  }
  return false;
}

function applyDemoMode(mode) {
  demoMode = mode === "testnet" ? "testnet" : "mock";
  const banner = document.getElementById("mode-banner");
  const badge = document.getElementById("status-mode");
  if (banner) {
    banner.dataset.mode = demoMode;
    banner.textContent =
      demoMode === "testnet" ? "TESTNET · live chain" : "MOCK · local ledger";
  }
  if (badge) {
    badge.dataset.mode = demoMode;
    badge.textContent = demoMode === "testnet" ? "Testnet" : "Mock";
  }
}

function updateStatusStrip({ account, threshold, approvals, beatLabel } = {}) {
  if (account !== undefined && account !== null) {
    const el = document.getElementById("status-account");
    if (el) el.textContent = `Account ${account}`;
  }
  if (threshold !== undefined && threshold !== null) {
    statusThreshold = threshold;
    const el = document.getElementById("status-threshold");
    if (el) el.textContent = `Threshold ${threshold}`;
  }
  if (approvals !== undefined && approvals !== null) {
    statusApprovals = approvals;
    const el = document.getElementById("status-approvals");
    if (el) {
      const t = statusThreshold != null ? `/${statusThreshold}` : "";
      el.textContent = `Approvals ${approvals}${t}`;
    }
  }
  if (beatLabel) {
    const el = document.getElementById("status-beat-label");
    if (el) el.textContent = beatLabel;
  }
}

function openDevDrawer() {
  const d = document.getElementById("dev-drawer");
  if (d && !d.open) d.open = true;
}

function syncGuideForTab(tab) {
  if (walkActive) {
    const beat = BEATS[walkIndex];
    setGuide(beat.step, beat.text);
    return;
  }
  const b = BROWSE[tab];
  if (b) setGuide(b.step, b.text);
  else {
    setGuide(
      "Browse freely",
      "Step through the five-beat proposals walkthrough, or open the developer drawer for aggregate, rotate, unsafe UTF-8, party finder, and PM resolve."
    );
  }
}

function applyAccountIds(id) {
  if (id === null || id === undefined || Number.isNaN(id)) return;
  storyAccountId = id;
  const s = String(id);
  ["query-account-id", "quorum-account-id", "agg-account-id", "change-account-id", "prop-account-id", "pm-account"]
    .forEach((fid) => {
      const el = document.getElementById(fid);
      if (el) el.value = s;
    });
  updateStatusStrip({ account: id });
}

function markBeatDone(index) {
  if (index < 0 || index >= beatDone.length) return;
  beatDone[index] = true;
  if (walkActive) setWalkUi(true);
}

/** Highest beat index reachable while walking (incomplete current beat included). */
function maxWalkIndexAllowed() {
  let max = 0;
  for (let i = 0; i < beatDone.length; i++) {
    max = i;
    if (!beatDone[i]) break;
  }
  return max;
}

function submitOk(out) {
  return out && out.outcome !== "panic" && out.tx_status !== "failed";
}

function syncPropApproveGate() {
  const propConfirm = document.getElementById("prop-confirm");
  const propBtn = document.getElementById("prop-approve-btn");
  if (!propConfirm || !propBtn) return;
  if (walkActive && !propPreviewShown) {
    propBtn.disabled = true;
    return;
  }
  propBtn.disabled = !propConfirm.checked;
}

function prefillStoryFields() {
  document.getElementById("create-members").value = STORY.members;
  document.getElementById("create-threshold").value = String(STORY.threshold);
  document.getElementById("quorum-msg").value = STORY.message;
  document.getElementById("quorum-signers").value = STORY.signers;
  document.getElementById("agg-msg").value = STORY.message;
  document.getElementById("agg-signers").value = STORY.signers;
  document.getElementById("change-new-members").value = STORY.newMembers;
  document.getElementById("change-new-threshold").value = String(STORY.newThreshold);
  document.getElementById("change-signers").value = STORY.signers;
  document.getElementById("prop-target").value = STORY.propTarget;
  document.getElementById("prop-function").value = STORY.propFunction;
  document.getElementById("prop-args-hex").value = STORY.propArgsHex;
  document.getElementById("prop-deadline").value = String(STORY.propDeadline);
  document.getElementById("prop-signer").value = STORY.propSigner;
  if (storyAccountId !== null) applyAccountIds(storyAccountId);
}

async function ensureStoryCast() {
  const identities = await api("/api/identities");
  const have = new Set(identities.map((i) => i.name));
  for (const name of STORY.cast) {
    if (have.has(name)) continue;
    try {
      await api("/api/identities", { method: "POST", body: JSON.stringify({ name }) });
    } catch (e) {
      if (!String(e.message).includes("already exists")) throw e;
    }
  }
  await refreshIdentities();
}

function setWalkUi(active) {
  walkActive = active;
  document.getElementById("story-guide").classList.toggle("walking", active);
  document.getElementById("walk-nav").hidden = !active;
  document.getElementById("btn-walkthrough-exit").hidden = !active;
  document.getElementById("btn-walkthrough").hidden = active;
  const next = document.getElementById("btn-walk-next");
  const prev = document.getElementById("btn-walk-prev");
  if (active) {
    prev.disabled = walkIndex === 0;
    next.disabled = !beatDone[walkIndex];
    next.textContent = walkIndex >= BEATS.length - 1 ? "Finish" : "Next beat →";
  }
}

function prepareBeatEntry(index) {
  const beat = BEATS[index];
  if (!beat) return;
  if (beat.beat === 4) {
    document.getElementById("prop-target").value = STORY.propTarget;
    document.getElementById("prop-function").value = STORY.propFunction;
    document.getElementById("prop-args-hex").value = STORY.propArgsHex;
    document.getElementById("prop-deadline").value = String(STORY.propDeadline);
    document.getElementById("prop-signer").value = "alice";
    document.getElementById("prop-confirm").checked = false;
    propPreviewShown = false;
    document.getElementById("prop-approve-btn").disabled = true;
    if (storyAccountId !== null) applyAccountIds(storyAccountId);
  }
  if (beat.beat === 5) {
    document.getElementById("prop-signer").value = "bob";
    document.getElementById("prop-confirm").checked = false;
    propPreviewShown = false;
    document.getElementById("prop-approve-btn").disabled = true;
    if (storyProposalId !== null) {
      document.getElementById("prop-id").value = String(storyProposalId);
    }
  }
}

function goWalkChapter(i) {
  walkIndex = Math.max(0, Math.min(BEATS.length - 1, i));
  const beat = BEATS[walkIndex];
  prepareBeatEntry(walkIndex);
  activateTab(beat.tab, {
    fromWalk: true,
    beatPhase: beat.beatPhase === "finalize" ? "finalize" : undefined,
  });
  setWalkUi(true);
  syncGuideForTab(beat.tab);
}

async function startWalkthrough() {
  try {
    beatDone = [false, false, false, false, false];
    propPreviewShown = false;
    storyProposalId = null;
    statusApprovals = null;
    await ensureStoryCast();
    prefillStoryFields();
    // Beat 1 primary action: cast ready.
    beatDone[0] = true;
    goWalkChapter(0);
    updateStatusStrip({
      threshold: STORY.threshold,
      approvals: 0,
      beatLabel: "Beat 1 · Cast",
    });
  } catch (e) {
    const msg = e && e.message ? e.message : String(e);
    setGuide("Walkthrough could not start", msg);
    alert(msg);
  }
}

function exitWalkthrough() {
  setWalkUi(false);
  const active = document.querySelector(".beat-dots .tab.active, .drawer-tabs .tab.active");
  syncGuideForTab(active ? active.dataset.tab : "cast");
}

async function refreshIdentities() {
  const identities = await api("/api/identities");
  const list = document.getElementById("identities-list");
  list.innerHTML = "";
  for (const id of identities) {
    const row = document.createElement("div");
    row.className = "id-row";
    row.title = "Click to add name to focused field; copy button for pk";
    const kind = id.pk_only ? `<span class="pill">pk-only</span>` : `<span class="pill signing">signing</span>`;
    row.innerHTML =
      `<span class="id-name">${id.name} ${kind}</span>` +
      `<span class="pk">${id.pk_base58.slice(0, 20)}…</span>` +
      `<button type="button" class="secondary tiny" data-copy="${id.pk_base58}">copy pk</button>`;
    row.addEventListener("click", (ev) => {
      if (ev.target.closest("[data-copy]")) return;
      if (focusedNameTarget) {
        const cur = splitNames(focusedNameTarget.value);
        if (!cur.includes(id.name)) cur.push(id.name);
        focusedNameTarget.value = cur.join(",");
      } else {
        appendName("create-members", id.name);
      }
    });
    row.querySelector("[data-copy]").addEventListener("click", async (ev) => {
      ev.stopPropagation();
      try {
        await navigator.clipboard.writeText(id.pk_base58);
      } catch {
        prompt("Public key", id.pk_base58);
      }
    });
    list.appendChild(row);
  }
}

async function createIdentity() {
  const name = document.getElementById("new-identity-name").value.trim();
  if (!name) return;
  try {
    await api("/api/identities", { method: "POST", body: JSON.stringify({ name }) });
    document.getElementById("new-identity-name").value = "";
    await refreshIdentities();
  } catch (e) {
    alert(e.message);
  }
}

async function importPk() {
  const name = document.getElementById("import-pk-name").value.trim();
  const pk = document.getElementById("import-pk-value").value.trim();
  if (!name || !pk) return;
  try {
    await api("/api/identities/import-pk", { method: "POST", body: JSON.stringify({ name, pk }) });
    document.getElementById("import-pk-name").value = "";
    document.getElementById("import-pk-value").value = "";
    await refreshIdentities();
  } catch (e) {
    alert(e.message);
  }
}

async function refreshSetupStatus() {
  try {
    const status = await api("/api/setup/status");
    if (status.demo_mode) applyDemoMode(status.demo_mode);
    const count = status.identities_count;
    document.getElementById("setup-store-status").textContent =
      `unlocked · ${count} identit${count === 1 ? "y" : "ies"} · ${status.store_path}`;
    const collectorEl = document.getElementById("setup-collector-status");
    if (status.collector_configured) {
      const auth = status.collector_user_configured ? "Basic Auth configured" : "no auth";
      collectorEl.textContent = `configured: ${status.collector_url} (${auth})`;
    } else {
      collectorEl.textContent =
        "not configured — set MULTISIG_COLLECTOR_URL (and optionally _USER/_PASSWORD) in the tool's environment before serve, then restart";
    }
  } catch (e) {
    document.getElementById("setup-store-status").textContent = e.message;
  }
}

function showSetupPk(identity) {
  document.getElementById("setup-my-pk-value").textContent = identity.pk_base58;
  document.getElementById("setup-my-pk").hidden = false;
}

async function createSetupIdentity() {
  const name = document.getElementById("setup-identity-name").value.trim();
  if (!name) return;
  try {
    const identity = await api("/api/identities", { method: "POST", body: JSON.stringify({ name }) });
    document.getElementById("setup-identity-name").value = "";
    await refreshIdentities();
    await refreshSetupStatus();
    showSetupPk(identity);
  } catch (e) {
    alert(e.message);
  }
}

let partySelected = new Set();

async function refreshParty() {
  setLog("party-log", "loading roster...");
  try {
    const members = await api("/api/party");
    renderPartyList(members);
    setLog("party-log", `roster: ${members.length} member(s)`, true);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("party-log", e.message);
  }
}

function renderPartyList(members) {
  const list = document.getElementById("party-list");
  list.innerHTML = "";
  const pks = new Set(members.map((m) => m.pk));
  partySelected = new Set([...partySelected].filter((pk) => pks.has(pk)));
  for (const m of members) {
    const row = document.createElement("div");
    row.className = "id-row";
    row.dataset.name = m.name;
    row.dataset.pk = m.pk;
    const note = m.note ? ` · ${m.note}` : "";
    row.innerHTML =
      `<input type="checkbox" ${partySelected.has(m.pk) ? "checked" : ""} />` +
      `<span class="id-name">${m.name}${note}</span>` +
      `<span class="pk">${m.pk.slice(0, 22)}…</span>` +
      `<button type="button" class="secondary tiny" data-copy="${m.pk}">copy pk</button>`;
    row.querySelector('input[type="checkbox"]').addEventListener("change", (ev) => {
      if (ev.target.checked) partySelected.add(m.pk);
      else partySelected.delete(m.pk);
    });
    row.querySelector("[data-copy]").addEventListener("click", async (ev) => {
      ev.stopPropagation();
      try {
        await navigator.clipboard.writeText(m.pk);
      } catch {
        prompt("Public key", m.pk);
      }
    });
    list.appendChild(row);
  }
}

async function partySignup() {
  const name = document.getElementById("party-signup-name").value.trim();
  const note = document.getElementById("party-signup-note").value.trim();
  if (!name) return;
  setLog("party-log", "signing up...");
  try {
    await api("/api/party", {
      method: "POST",
      body: JSON.stringify({ name, note: note || undefined }),
    });
    await refreshParty();
    setLog("party-log", `signed up as ${name}`, true);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("party-log", e.message);
  }
}

// Ensures each selected roster member exists as a local pk-only identity
// (importing it if new — "already exists" is treated as already-imported,
// the idempotent common case), then prefills Form council's members field.
async function usePartyForCouncil() {
  if (partySelected.size === 0) {
    alert("Select at least one roster row first.");
    return;
  }
  const rows = [...document.querySelectorAll("#party-list .id-row")].filter((r) =>
    partySelected.has(r.dataset.pk)
  );
  const names = [];
  for (const row of rows) {
    const { name, pk } = row.dataset;
    try {
      await api("/api/identities/import-pk", {
        method: "POST",
        body: JSON.stringify({ name, pk }),
      });
    } catch (e) {
      if (!String(e.message).includes("already exists")) {
        setLog("party-log", `import failed for ${name}: ${e.message}`, false);
        continue;
      }
    }
    names.push(name);
  }
  await refreshIdentities();
  document.getElementById("create-members").value = names.join(",");
  activateTab("council");
  setLog("party-log", `prefilled Form council with: ${names.join(",")}`, true);
}

async function submitCreateAccount() {
  const members = splitNames(document.getElementById("create-members").value);
  const threshold = parseInt(document.getElementById("create-threshold").value, 10);
  setLog("create-log", "submitting...");
  try {
    const out = await api("/api/account/create", {
      method: "POST",
      body: JSON.stringify({ members, threshold }),
    });
    let next = "";
    try {
      const n = await api("/api/account/next-id");
      next = `\nnext_account_id => ${n} (created id is usually n-1)`;
      if (out.outcome !== "panic" && typeof n === "number") applyAccountIds(n - 1);
      else if (out.outcome !== "panic" && typeof n === "string" && /^\d+$/.test(n)) {
        applyAccountIds(parseInt(n, 10) - 1);
      }
    } catch (_) {}
    showSubmit("create-log", { ...out, log: (out.log || "") + next });
    if (submitOk(out)) {
      updateStatusStrip({ threshold, approvals: 0 });
      if (walkActive && walkIndex === 1) markBeatDone(1);
    }
  } catch (e) {
    showError("create-log", e.message);
  }
}

async function refreshNextId() {
  setLog("create-log", "querying next-id...");
  try {
    const n = await api("/api/account/next-id");
    setLog("create-log", `next_account_id => ${n}`, true);
    setTxStatus("create-log", { show: false });
  } catch (e) {
    showError("create-log", e.message);
  }
}

async function queryAccount() {
  const id = document.getElementById("query-account-id").value;
  setLog("query-log", "querying...");
  setTxStatus("query-log", { show: false });
  try {
    const out = await api(`/api/account/${id}`);
    setLog("query-log", out ? JSON.stringify(out, null, 2) : "not found", true);
    if (out) {
      updateStatusStrip({
        account: id,
        threshold: out.threshold,
        approvals: statusApprovals != null ? statusApprovals : 0,
      });
      try {
        const meta = await api(`/api/account/${id}/meta`);
        if (meta) {
          setLog(
            "query-log",
            JSON.stringify(out, null, 2) + "\n--- meta ---\n" + JSON.stringify(meta, null, 2),
            true
          );
        }
      } catch (_) {}
      if (walkActive && walkIndex === 2) markBeatDone(2);
    }
  } catch (e) {
    showError("query-log", e.message);
  }
}

async function queryMeta() {
  const id = document.getElementById("query-account-id").value;
  setLog("query-log", "meta...");
  setTxStatus("query-log", { show: false });
  try {
    const out = await api(`/api/account/${id}/meta`);
    setLog("query-log", out ? JSON.stringify(out, null, 2) : "not found", true);
  } catch (e) {
    showError("query-log", e.message);
  }
}

async function queryKeys() {
  const id = document.getElementById("query-account-id").value;
  setLog("query-log", "keys...");
  setTxStatus("query-log", { show: false });
  try {
    const out = await api(`/api/account/${id}/keys`);
    setLog("query-log", out ? JSON.stringify(out, null, 2) : "not found", true);
  } catch (e) {
    showError("query-log", e.message);
  }
}

function quorumBody() {
  return {
    account: parseInt(document.getElementById("quorum-account-id").value, 10),
    msg: document.getElementById("quorum-msg").value,
    hex: false,
    signers: splitNames(document.getElementById("quorum-signers").value),
  };
}

async function submitQuorum() {
  setLog("quorum-log", "submitting...");
  try {
    const out = await api("/api/quorum/submit", { method: "POST", body: JSON.stringify(quorumBody()) });
    showSubmit("quorum-log", out);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("quorum-log", e.message);
  }
}

async function checkQuorum() {
  setLog("quorum-log", "check/diagnose (free)...");
  try {
    const out = await api("/api/quorum/diagnose", { method: "POST", body: JSON.stringify(quorumBody()) });
    showSubmit("quorum-log", out);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("quorum-log", e.message);
  }
}

function aggBody() {
  return {
    account: parseInt(document.getElementById("agg-account-id").value, 10),
    msg: document.getElementById("agg-msg").value,
    hex: false,
    signers: splitNames(document.getElementById("agg-signers").value),
  };
}

async function submitQuorumAgg() {
  setLog("agg-log", "submitting...");
  try {
    const out = await api("/api/quorum-agg/submit", { method: "POST", body: JSON.stringify(aggBody()) });
    showSubmit("agg-log", out);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("agg-log", e.message);
  }
}

async function checkQuorumAgg() {
  setLog("agg-log", "check (free)...");
  try {
    const out = await api("/api/quorum-agg/check", { method: "POST", body: JSON.stringify(aggBody()) });
    showSubmit("agg-log", out);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("agg-log", e.message);
  }
}

async function submitChangeAccount() {
  const account = parseInt(document.getElementById("change-account-id").value, 10);
  const new_members = splitNames(document.getElementById("change-new-members").value);
  const new_threshold = parseInt(document.getElementById("change-new-threshold").value, 10);
  const signers = splitNames(document.getElementById("change-signers").value);
  setLog("change-log", "submitting...");
  try {
    const out = await api("/api/change-account/submit", {
      method: "POST",
      body: JSON.stringify({ account, new_members, new_threshold, signers }),
    });
    showSubmit("change-log", out);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("change-log", e.message);
  }
}

async function proposalCreate() {
  const account = parseInt(document.getElementById("prop-account-id").value, 10);
  const target = document.getElementById("prop-target").value.trim();
  const functionName = document.getElementById("prop-function").value.trim();
  const args_hex = document.getElementById("prop-args-hex").value.trim();
  const deadline = parseInt(document.getElementById("prop-deadline").value, 10) || 0;
  setLog("prop-log", "proposing...");
  try {
    const out = await api("/api/proposal/create", {
      method: "POST",
      body: JSON.stringify({
        account,
        target,
        function: functionName,
        args_hex,
        deadline,
      }),
    });
    const submit = out.submit || out;
    storyProposalId = out.allocated_id_hint;
    document.getElementById("prop-id").value = String(out.allocated_id_hint);
    showSubmit("prop-log", {
      ...submit,
      log: (submit.log || "") + `\nallocated_id_hint: ${out.allocated_id_hint}`,
    });
    updateStatusStrip({ approvals: 0 });
  } catch (e) {
    showError("prop-log", e.message);
  }
}

async function proposalNextId() {
  setLog("prop-log", "next-id...");
  try {
    const n = await api("/api/proposal/next-id");
    setLog("prop-log", `next_proposal_id => ${n}`, true);
  } catch (e) {
    setLog("prop-log", e.message, false);
  }
}

async function proposalStatus() {
  const id = document.getElementById("prop-id").value;
  setLog("prop-log", "status...");
  try {
    const out = await api(`/api/proposal/${id}`);
    setLog("prop-log", out ? JSON.stringify(out, null, 2) : "not found", true);
  } catch (e) {
    setLog("prop-log", e.message, false);
  }
}

async function proposalPreview() {
  const id = document.getElementById("prop-id").value;
  setLog("prop-log", "preview (no signing)...");
  try {
    const out = await api(`/api/proposal/${id}/preview`);
    const box = document.getElementById("prop-preview");
    box.hidden = false;
    box.innerHTML =
      `<strong>Fingerprint</strong><br>` +
      `digest: ${out.digest_hex}<br>` +
      `mnemonic: ${out.digest_mnemonic}<br>` +
      `safety: ${out.digest_safety_number}<br>` +
      `chain=${out.chain_id} committee=${out.committee_id} nonce=${out.nonce}<br>` +
      `target=${out.target_hex}<br>fn=${out.function_name}<br>` +
      `args=${out.call_args_hex}<br>deadline=${out.deadline}`;
    document.getElementById("prop-confirm").checked = false;
    propPreviewShown = true;
    syncPropApproveGate();
    setLog("prop-log", "preview ok — check fingerprint, then confirm + approve", true);
  } catch (e) {
    propPreviewShown = false;
    syncPropApproveGate();
    setLog("prop-log", e.message, false);
  }
}

async function proposalApprove() {
  const id = document.getElementById("prop-id").value;
  const signer = document.getElementById("prop-signer").value.trim();
  if (walkActive && !propPreviewShown) {
    setLog("prop-log", "preview the proposal first, then confirm + approve", false);
    return;
  }
  if (!document.getElementById("prop-confirm").checked) {
    setLog("prop-log", "check the confirm box after preview", false);
    return;
  }
  setLog("prop-log", "approving (confirm:true)...");
  try {
    const out = await api(`/api/proposal/${id}/approve`, {
      method: "POST",
      body: JSON.stringify({ signer, confirm: true }),
    });
    const submit = out.submit || out;
    const intent = out.intent;
    let extra = "";
    if (intent) {
      extra =
        "\n=== intent (canonical; never trust human_summary) ===\n" +
        JSON.stringify(intent, null, 2);
    }
    showSubmit("prop-log", {
      ...submit,
      log: (submit.log || "") + extra,
    });
    // Refresh approval count from status when possible.
    try {
      const st = await api(`/api/proposal/${id}`);
      if (st && st.approvals_len != null) {
        updateStatusStrip({ approvals: st.approvals_len });
      } else {
        updateStatusStrip({
          approvals: (statusApprovals != null ? statusApprovals : 0) + 1,
        });
      }
    } catch (_) {
      updateStatusStrip({
        approvals: (statusApprovals != null ? statusApprovals : 0) + 1,
      });
    }
    // Alice or Bob: only unlock walk beat on successful submit (same as create/finalize).
    if (submitOk(submit) && walkActive && walkIndex === 3 && signer === "alice") {
      markBeatDone(3);
    }
    // Beat 5 still needs finalize — Bob approve alone does not unlock Finish.
  } catch (e) {
    showError("prop-log", e.message);
  }
}

async function proposalFinalize() {
  const id = document.getElementById("prop-id").value;
  setLog("prop-log", "finalizing...");
  try {
    const out = await api(`/api/proposal/${id}/finalize`, { method: "POST", body: "{}" });
    showSubmit("prop-log", out);
    if (submitOk(out)) {
      if (walkActive && walkIndex === 4) markBeatDone(4);
    }
  } catch (e) {
    showError("prop-log", e.message);
  }
}

async function pmResolveInit() {
  const body = {
    market_id: Number(document.getElementById("pm-market").value),
    winning_outcome: Number(document.getElementById("pm-outcome").value),
    pm_contract_id: document.getElementById("pm-contract").value.trim(),
    registry_account_id: Number(document.getElementById("pm-account").value),
    threshold: Number(document.getElementById("pm-threshold").value),
    summary: document.getElementById("pm-summary").value.trim() || null,
    push: true,
  };
  setLog("pm-log", "init + push...");
  try {
    const out = await api("/api/pm-resolve/init", { method: "POST", body: JSON.stringify(body) });
    document.getElementById("pm-id").value = out.id;
    setLog(
      "pm-log",
      `init ok\nid: ${out.id}\ndigest: ${out.signed_digest}\npushed: ${out.pushed}\npartials: 0/${out.blob.threshold}`,
      true
    );
    await pmResolveList();
  } catch (e) {
    toastIfNeedsTestnet(e);
    setLog("pm-log", e.message, false);
  }
}

async function pmRefreshChain() {
  const hint = document.getElementById("pm-chain-hint");
  if (hint) hint.textContent = "Loading from testnet…";
  try {
    const [dep, accounts, markets] = await Promise.all([
      api("/api/deployments/pm"),
      api("/api/registry/accounts?limit=64"),
      api("/api/pm/markets?limit=50"),
    ]);
    const pmEl = document.getElementById("pm-contract");
    if (pmEl && !pmEl.value.trim()) {
      pmEl.value = dep.pm_contract_id;
    }

    const council = document.getElementById("pm-council-pick");
    if (council) {
      const prev = council.value;
      council.innerHTML = "";
      const blank = document.createElement("option");
      blank.value = "";
      blank.textContent = accounts.length ? "— pick council —" : "— no accounts —";
      council.appendChild(blank);
      for (const a of accounts) {
        const opt = document.createElement("option");
        opt.value = String(a.id);
        opt.textContent = a.label;
        opt.dataset.threshold = String(a.threshold);
        council.appendChild(opt);
      }
      if (prev && [...council.options].some((o) => o.value === prev)) council.value = prev;
    }

    const marketPick = document.getElementById("pm-market-pick");
    if (marketPick) {
      const prev = marketPick.value;
      marketPick.innerHTML = "";
      const blank = document.createElement("option");
      blank.value = "";
      blank.textContent = markets.length ? "— pick market —" : "— no markets —";
      marketPick.appendChild(blank);
      const sorted = [...markets].sort(
        (a, b) => Number(b.under_review) - Number(a.under_review) || a.id - b.id
      );
      for (const m of sorted) {
        const opt = document.createElement("option");
        opt.value = String(m.id);
        opt.textContent = (m.under_review ? "★ " : "") + m.label;
        marketPick.appendChild(opt);
      }
      if (prev && [...marketPick.options].some((o) => o.value === prev)) marketPick.value = prev;
    }

    const under = markets.filter((m) => m.under_review).length;
    if (hint) {
      hint.textContent = `PM ${dep.pm_contract_id.slice(0, 10)}… · ${accounts.length} account(s) · ${markets.length} market(s) (${under} under review)`;
    }
  } catch (e) {
    toastIfNeedsTestnet(e);
    if (hint) hint.textContent = e.message;
    setLog("pm-log", e.message, false);
  }
}

function pmOnCouncilPick() {
  const sel = document.getElementById("pm-council-pick");
  const opt = sel && sel.selectedOptions[0];
  if (!opt || !opt.value) return;
  document.getElementById("pm-account").value = opt.value;
  if (opt.dataset.threshold) {
    document.getElementById("pm-threshold").value = opt.dataset.threshold;
  }
}

function pmOnMarketPick() {
  const sel = document.getElementById("pm-market-pick");
  const opt = sel && sel.selectedOptions[0];
  if (!opt || !opt.value) return;
  document.getElementById("pm-market").value = opt.value;
  const summary = document.getElementById("pm-summary");
  if (summary && !summary.value.trim()) {
    summary.value = `resolve market ${opt.value}`;
  }
}

async function pmResolveList() {
  try {
    const items = await api("/api/pm-resolve/list");
    const list = document.getElementById("pm-list");
    if (!items.length) {
      list.innerHTML = "<p class=\"pane-hint\">No pm_council_resolve blobs on the collector yet.</p>";
      return;
    }
    list.innerHTML = "";
    for (const it of items) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "id-row";
      row.innerHTML =
        `<strong>${it.id.slice(0, 16)}…</strong> ` +
        `<span class="pill">${it.partials_count}/${it.threshold}</span>`;
      row.addEventListener("click", () => {
        document.getElementById("pm-id").value = it.id;
        pmResolveStatus();
      });
      list.appendChild(row);
    }
  } catch (e) {
    toastIfNeedsTestnet(e);
    setLog("pm-log", e.message, false);
  }
}

async function pmResolveStatus() {
  const id = document.getElementById("pm-id").value.trim();
  if (!id) {
    setLog("pm-log", "set blob id first", false);
    return;
  }
  setLog("pm-log", "status...");
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}`);
    document.getElementById("pm-market").value = out.market_id;
    document.getElementById("pm-outcome").value = out.winning_outcome;
    document.getElementById("pm-contract").value = out.pm_contract_id;
    document.getElementById("pm-account").value = out.registry_account_id;
    document.getElementById("pm-threshold").value = out.threshold;
    if (out.human_summary) document.getElementById("pm-summary").value = out.human_summary;
    let text =
      `status\nmarket=${out.market_id} outcome=${out.winning_outcome}\n` +
      `partials=${out.partials_count}/${out.threshold} ready=${out.ready}\n` +
      `digest=${out.signed_digest}\npm=${out.pm_contract_id}\naccount=${out.registry_account_id}`;
    if (out.registry_warn) text += `\n${out.registry_warn}`;
    setLog("pm-log", text, true);
  } catch (e) {
    toastIfNeedsTestnet(e);
    setLog("pm-log", e.message, false);
  }
}

async function pmResolvePreview() {
  const id = document.getElementById("pm-id").value.trim();
  if (!id) {
    setLog("pm-log", "set blob id first", false);
    return;
  }
  setLog("pm-log", "preview (no signing)...");
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}/preview`);
    const box = document.getElementById("pm-preview");
    box.hidden = false;
    box.innerHTML =
      `<strong>council-resolve.v2 fingerprint</strong><br>` +
      `digest: ${out.digest_hex}<br>` +
      `mnemonic: ${out.digest_mnemonic}<br>` +
      `safety: ${out.digest_safety_number}<br>` +
      `market=${out.market_id} outcome=${out.winning_outcome}<br>` +
      `pm=${out.pm_contract_id}<br>account=${out.registry_account_id} threshold=${out.threshold}`;
    document.getElementById("pm-market").value = out.market_id;
    document.getElementById("pm-outcome").value = out.winning_outcome;
    document.getElementById("pm-contract").value = out.pm_contract_id;
    document.getElementById("pm-account").value = out.registry_account_id;
    document.getElementById("pm-threshold").value = out.threshold;
    document.getElementById("pm-confirm").checked = false;
    document.getElementById("pm-sign-btn").disabled = true;
    setLog("pm-log", "preview ok — compare mnemonic with co-signers, then confirm + sign", true);
  } catch (e) {
    toastIfNeedsTestnet(e);
    setLog("pm-log", e.message, false);
  }
}

async function pmResolveSign() {
  const id = document.getElementById("pm-id").value.trim();
  const signer = document.getElementById("pm-signer").value.trim();
  if (!id || !signer) {
    setLog("pm-log", "need blob id and signer", false);
    return;
  }
  if (!document.getElementById("pm-confirm").checked) {
    setLog("pm-log", "check the confirm box after preview", false);
    return;
  }
  setLog("pm-log", "signing...");
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}/sign`, {
      method: "POST",
      body: JSON.stringify({ signer, confirm: true }),
    });
    setLog(
      "pm-log",
      `signed as ${signer}\npartials=${out.partials_count}/${out.threshold} ready=${out.ready}\npk=${out.signer_pk}\ndigest=${out.digest_hex}`,
      true
    );
    await pmResolveList();
  } catch (e) {
    toastIfNeedsTestnet(e);
    setLog("pm-log", e.message, false);
  }
}

async function pmResolveSubmit() {
  const id = document.getElementById("pm-id").value.trim();
  if (!id) {
    setLog("pm-log", "set blob id first", false);
    return;
  }
  setLog("pm-log", "submitting resolve...");
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}/submit`, {
      method: "POST",
      body: "{}",
    });
    showSubmit("pm-log", out);
  } catch (e) {
    toastIfNeedsTestnet(e);
    showError("pm-log", e.message);
  }
}

function applyPmPrefillsFromUrl() {
  const params = new URLSearchParams(location.search);
  const setIf = (id, key) => {
    if (!params.has(key)) return;
    const el = document.getElementById(id);
    if (el) el.value = params.get(key);
  };
  setIf("pm-market", "market");
  setIf("pm-outcome", "outcome");
  setIf("pm-contract", "pm");
  setIf("pm-account", "account");
  setIf("pm-threshold", "threshold");
  setIf("pm-summary", "summary");
}

const SLIDE_HASHES = new Set(["top", "demo", "use-cases", ""]);

function tabFromUrl() {
  const params = new URLSearchParams(location.search);
  const q = params.get("tab");
  if (q) return q;
  const hash = (location.hash || "").replace(/^#/, "");
  // Page slides use #top / #demo / #use-cases — not chapter tabs.
  if (SLIDE_HASHES.has(hash)) return null;
  if (hash) return hash;
  return null;
}

function goBeat(delta) {
  const active = document.querySelector(".beat-dots .tab.active");
  let i = 0;
  if (active && active.dataset.beat) {
    const b = parseInt(active.dataset.beat, 10);
    if (!Number.isNaN(b)) i = b - 1;
  }
  let target = Math.max(0, Math.min(BEATS.length - 1, i + delta));
  // While walking, do not advance past incomplete beats (free browse stays open).
  if (walkActive) {
    const maxAllowed = maxWalkIndexAllowed();
    if (delta > 0 && target > maxAllowed) return;
    target = Math.min(target, maxAllowed);
  }
  i = target;
  const beat = BEATS[i];
  const dot = document.querySelector(`.beat-dots .tab[data-beat="${i + 1}"]`);
  if (dot) {
    document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
      const on = btn === dot;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-selected", on ? "true" : "false");
    });
  }
  if (walkActive) {
    walkIndex = i;
    prepareBeatEntry(i);
  }
  activateTab(beat.tab, {
    fromBeatNav: true,
    beatPhase: beat.beatPhase === "finalize" ? "finalize" : undefined,
  });
  if (walkActive) setWalkUi(true);
  const label = document.getElementById("status-beat-label");
  if (label && dot) {
    label.textContent = `Beat ${dot.dataset.beat} · ${dot.textContent.replace(/^\d+\s·\s/, "")}`;
  }
}

function resolveWalkIndexForTab(name, opts) {
  if (opts.beat != null && !Number.isNaN(opts.beat)) {
    return Math.max(0, Math.min(BEATS.length - 1, opts.beat - 1));
  }
  if (name === "proposal") {
    return opts.beatPhase === "finalize" ? 4 : 3;
  }
  const idx = BEATS.findIndex((b) => b.tab === name);
  return idx >= 0 ? idx : null;
}

function activateTab(name, opts = {}) {
  // While walking, beat dots must not jump past incomplete beats (drawer tabs still free).
  let walkNextIdx = null;
  if (walkActive && !opts.fromWalk && !opts.fromBeatNav) {
    walkNextIdx = resolveWalkIndexForTab(name, opts);
    if (walkNextIdx != null && walkNextIdx > maxWalkIndexAllowed()) return;
  }

  if (DRAWER_TABS.has(name)) openDevDrawer();

  document.querySelectorAll(".tab").forEach((btn) => {
    // goBeat already picked the Propose vs Finalize dot.
    if (opts.fromBeatNav && btn.closest(".beat-dots")) return;
    const on = btn.dataset.tab === name;
    btn.classList.toggle("active", on);
    btn.setAttribute("aria-selected", on ? "true" : "false");
  });
  // Beats 4 + 5 share data-tab=proposal; keep only the matching data-beat active when set.
  if (!opts.fromBeatNav && name === "proposal") {
    const prefer = opts.beatPhase === "finalize" ? "5" : "4";
    document.querySelectorAll('.beat-dots .tab[data-tab="proposal"]').forEach((btn) => {
      const on = btn.dataset.beat === prefer;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-selected", on ? "true" : "false");
    });
  }
  document.querySelectorAll(".tab-panel").forEach((panel) => {
    const on = panel.id === `panel-${name}`;
    panel.classList.toggle("active", on);
    if (on) panel.removeAttribute("hidden");
    else panel.setAttribute("hidden", "");
  });
  if (walkActive && !opts.fromWalk && !opts.fromBeatNav && walkNextIdx != null) {
    walkIndex = walkNextIdx;
    prepareBeatEntry(walkIndex);
    setWalkUi(true);
  }
  if (name === "setup") refreshSetupStatus().catch((e) => console.error(e));
  if (name === "party") refreshParty().catch((e) => console.error(e));
  if (name === "pm-resolve") {
    pmRefreshChain().catch((e) => console.error(e));
    pmResolveList().catch((e) => console.error(e));
  }
  syncGuideForTab(name);
  const label = document.getElementById("status-beat-label");
  const beatDot = document.querySelector(`.beat-dots .tab.active[data-tab="${name}"]`)
    || document.querySelector(`.beat-dots .tab[data-tab="${name}"]`);
  if (label) {
    if (beatDot) label.textContent = `Beat ${beatDot.dataset.beat} · ${beatDot.textContent.replace(/^\d+\s·\s/, "")}`;
    else label.textContent = `Drawer · ${name}`;
  }
}

wireNameTargets();
applyPmPrefillsFromUrl();
refreshIdentities().catch((e) => console.error(e));
refreshSetupStatus().catch((e) => console.error(e));

function wireConfirmGates() {
  const propConfirm = document.getElementById("prop-confirm");
  if (propConfirm) {
    propConfirm.addEventListener("change", () => syncPropApproveGate());
  }
  const pmConfirm = document.getElementById("pm-confirm");
  const pmBtn = document.getElementById("pm-sign-btn");
  if (pmConfirm && pmBtn) {
    pmConfirm.addEventListener("change", () => {
      pmBtn.disabled = !pmConfirm.checked;
    });
  }
}
wireConfirmGates();

document.getElementById("setup-my-pk-copy").addEventListener("click", async () => {
  const pk = document.getElementById("setup-my-pk-value").textContent;
  try {
    await navigator.clipboard.writeText(pk);
  } catch {
    prompt("Public key", pk);
  }
});

document.querySelectorAll(".tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    const opts = {};
    if (btn.dataset.beatPhase) opts.beatPhase = btn.dataset.beatPhase;
    if (btn.dataset.beat) {
      const b = parseInt(btn.dataset.beat, 10);
      if (!Number.isNaN(b)) opts.beat = b;
    }
    activateTab(btn.dataset.tab, opts);
  });
});

const initialTab = tabFromUrl();
if (initialTab) {
  activateTab(initialTab);
}

function on(id, ev, fn) {
  const el = document.getElementById(id);
  if (el) el.addEventListener(ev, fn);
}

on("btn-walkthrough", "click", () => startWalkthrough());
on("btn-walkthrough-exit", "click", () => exitWalkthrough());
on("btn-walk-prev", "click", () => {
  if (walkIndex > 0) goWalkChapter(walkIndex - 1);
});
on("btn-walk-next", "click", () => {
  if (!beatDone[walkIndex]) return;
  if (walkIndex >= BEATS.length - 1) {
    exitWalkthrough();
    setGuide(
      "Walkthrough complete",
      "You’ve walked the five-beat proposals story. Browse any beat freely, or open the developer drawer — start again to replay with a fresh account id."
    );
    return;
  }
  goWalkChapter(walkIndex + 1);
});

on("btn-beat-prev", "click", () => goBeat(-1));
on("btn-beat-next", "click", () => goBeat(1));

// Highlight page-nav dots from scroll position (cover / demo / use-cases).
(function wireSlideDots() {
  const root = document.getElementById("slides");
  const dots = [...document.querySelectorAll(".dots .dot")];
  if (!root || !dots.length) return;
  const sections = dots
    .map((d) => document.getElementById(d.dataset.slide))
    .filter(Boolean);

  function scrollToHash(behavior) {
    const id = (location.hash || "#top").replace(/^#/, "") || "top";
    const el = document.getElementById(id);
    if (!el || !root.contains(el)) return;
    // Window doesn't scroll — .slides owns overflow.
    const top = el.offsetTop - root.offsetTop;
    root.scrollTo({ top, behavior: behavior || "auto" });
  }

  const sync = () => {
    let best = 0;
    let bestDist = Infinity;
    sections.forEach((sec, i) => {
      const dist = Math.abs(sec.getBoundingClientRect().top);
      if (dist < bestDist) {
        bestDist = dist;
        best = i;
      }
    });
    dots.forEach((d, i) => d.classList.toggle("active", i === best));
  };
  root.addEventListener("scroll", sync, { passive: true });
  window.addEventListener("hashchange", () => {
    scrollToHash("smooth");
    sync();
  });
  // Initial hash (e.g. #demo from cover CTA / deep link).
  scrollToHash("auto");
  sync();
})();
