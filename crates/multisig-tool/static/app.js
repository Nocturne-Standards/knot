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
  propTarget: "",
  propFunction: "set_value",
  propArgsHex: "",
  propDeadline: 0,
  propSigner: "alice",
};

const CHAPTERS = [
  {
    tab: "cast",
    step: "Chapter 1 of 7 — Meet the cast",
    text: "Alice, Bob, and Carol are the treasury signers. The walkthrough creates them if missing. Click Next when you see all three as signing identities.",
  },
  {
    tab: "council",
    step: "Chapter 2 of 7 — Form the treasury",
    text: "Create a 2-of-3 council with alice,bob,carol. Press create_account — the new account id is filled into later chapters automatically.",
  },
  {
    tab: "check",
    step: "Chapter 3 of 7 — Look it up",
    text: "Free-read the council. Query should show three members and threshold 2. No gas, just confirmation.",
  },
  {
    tab: "payout",
    step: "Chapter 4 of 7 — Approve a payout",
    text: "Alice and Bob sign “approve payout #42”. Submit the quorum. Prefer check/diagnose only as a hint — free-read can look untrusted.",
  },
  {
    tab: "aggregate",
    step: "Chapter 5 of 7 — Same payout, cheaper",
    text: "Optional detour: same message and signers, but one aggregate verify on-chain. Skip with Next if you want the main plot.",
  },
  {
    tab: "rotate",
    step: "Chapter 6 of 7 — Rotate the council",
    text: "Drop Bob. Current members alice+bob authorize the new set alice,carol. After success, Look it up should show the new roster and a bumped nonce.",
  },
  {
    tab: "proposal",
    step: "Chapter 7 of 7 — Multi-person approve",
    text: "Propose the wire message, approve as Alice, then finalize when approvals ≥ threshold. On a second machine, Bob (or Carol) would approve from their own keystore.",
  },
];

const BROWSE = {
  setup: {
    step: "Chapter 0 — Setup",
    text: "Confirm your keystore is unlocked, create a signing identity, and check the collector URL configured server-side.",
  },
  cast: {
    step: "Chapter 1 — Meet the cast",
    text: "Create named BLS keys that stay in this process. Foreign members can be imported as pk-only.",
  },
  council: {
    step: "Chapter 2 — Form the treasury",
    text: "Register an M-of-N member set on-chain. Naming keys grants the creator no special power.",
  },
  check: {
    step: "Chapter 3 — Look it up",
    text: "Confirm members, threshold, and nonce after creates or rotations — free reads, no gas.",
  },
  payout: {
    step: "Chapter 4 — Approve a payout",
    text: "Per-signature quorum: each signer signs the message; on-chain verifies each BLS sig.",
  },
  aggregate: {
    step: "Chapter 5 — Cheaper verify",
    text: "Same story as a payout, but aggregated multisig — one pairing check on-chain.",
  },
  rotate: {
    step: "Chapter 6 — Rotate members",
    text: "Current members authorize a new set. Best place for crisp pass/fail demos.",
  },
  proposal: {
    step: "Chapter 7 — Multi-person",
    text: "Propose → each machine approves → finalize. Coordination is the chain, not a file handoff.",
  },
  "pm-resolve": {
    step: "Chapter 8 — PM resolve",
    text: "Init a council-resolve blob, collect secure BLS partials via the collector, then submit prediction-market.resolve when partials ≥ threshold.",
  },
  party: {
    step: "Party finder",
    text: "Sign up your local identity's public key on the shared roster, then pick members to prefill Form council.",
  },
};

let walkActive = false;
let walkIndex = 0;
let storyAccountId = null;

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

function syncGuideForTab(tab) {
  if (walkActive) {
    const ch = CHAPTERS[walkIndex];
    setGuide(ch.step, ch.text);
    return;
  }
  const b = BROWSE[tab];
  if (b) setGuide(b.step, b.text);
  else {
    setGuide(
      "Browse freely",
      "Pick a chapter below, or start the example walkthrough to load Alice, Bob, and Carol and step through a complete payout story."
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
    next.textContent = walkIndex >= CHAPTERS.length - 1 ? "Finish" : "Next chapter →";
  }
}

function goWalkChapter(i) {
  walkIndex = Math.max(0, Math.min(CHAPTERS.length - 1, i));
  const ch = CHAPTERS[walkIndex];
  activateTab(ch.tab, { fromWalk: true });
  setWalkUi(true);
  syncGuideForTab(ch.tab);
}

async function startWalkthrough() {
  try {
    await ensureStoryCast();
    prefillStoryFields();
    goWalkChapter(0);
  } catch (e) {
    alert(e.message);
  }
}

function exitWalkthrough() {
  setWalkUi(false);
  const active = document.querySelector(".tab.active");
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
    showError("quorum-log", e.message);
  }
}

async function checkQuorum() {
  setLog("quorum-log", "check/diagnose (free)...");
  try {
    const out = await api("/api/quorum/diagnose", { method: "POST", body: JSON.stringify(quorumBody()) });
    showSubmit("quorum-log", out);
  } catch (e) {
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
    showError("agg-log", e.message);
  }
}

async function checkQuorumAgg() {
  setLog("agg-log", "check (free)...");
  try {
    const out = await api("/api/quorum-agg/check", { method: "POST", body: JSON.stringify(aggBody()) });
    showSubmit("agg-log", out);
  } catch (e) {
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
    document.getElementById("prop-id").value = String(out.allocated_id_hint);
    showSubmit("prop-log", {
      ...submit,
      log: (submit.log || "") + `\nallocated_id_hint: ${out.allocated_id_hint}`,
    });
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
    document.getElementById("prop-approve-btn").disabled = true;
    setLog("prop-log", "preview ok — check fingerprint, then confirm + approve", true);
  } catch (e) {
    setLog("prop-log", e.message, false);
  }
}

async function proposalApprove() {
  const id = document.getElementById("prop-id").value;
  const signer = document.getElementById("prop-signer").value.trim();
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

const BEAT_TABS = ["cast", "council", "check", "proposal"];

function beatIndexForTab(tab) {
  const i = BEAT_TABS.indexOf(tab);
  return i >= 0 ? i : 0;
}

function goBeat(delta) {
  const active = document.querySelector(".beat-dots .tab.active");
  const cur = active ? active.dataset.tab : "cast";
  let i = beatIndexForTab(cur);
  // Prefer data-beat when both Propose/Finalize share data-tab=proposal.
  if (active && active.dataset.beat) {
    const b = parseInt(active.dataset.beat, 10);
    if (!Number.isNaN(b)) i = Math.max(0, Math.min(BEAT_TABS.length - 1, b - 1));
  }
  i = Math.max(0, Math.min(BEAT_TABS.length - 1, i + delta));
  const nextTab = BEAT_TABS[i];
  const dot = document.querySelector(`.beat-dots .tab[data-beat="${i + 1}"]`);
  if (dot) {
    document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
      const on = btn === dot;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-selected", on ? "true" : "false");
    });
  }
  activateTab(nextTab, { fromBeatNav: true });
  const label = document.getElementById("status-beat-label");
  if (label && dot) label.textContent = `Beat ${dot.dataset.beat} · ${dot.textContent.replace(/^\d+\s·\s/, "")}`;
}

function activateTab(name, opts = {}) {
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
  if (walkActive && !opts.fromWalk) {
    const idx = CHAPTERS.findIndex((c) => c.tab === name);
    if (idx >= 0) walkIndex = idx;
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
  const propBtn = document.getElementById("prop-approve-btn");
  if (propConfirm && propBtn) {
    propConfirm.addEventListener("change", () => {
      propBtn.disabled = !propConfirm.checked;
    });
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
  if (walkIndex >= CHAPTERS.length - 1) {
    exitWalkthrough();
    setGuide(
      "Walkthrough complete",
      "You’ve walked the treasury story. Browse any chapter freely, or start again to replay with a fresh account id."
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
  sync();
})();
