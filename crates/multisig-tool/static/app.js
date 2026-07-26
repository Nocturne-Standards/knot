// Plain JS, no build step. Token from index.html; signing is server-side.

const TOKEN = window.MULTISIG_TOOL_TOKEN;

const STORY = {
  cast: ["alice", "bob", "carol"],
  members: "alice,bob,carol",
  threshold: 2,
  // Mock-safe noop target: 31 zero bytes + 0x01 (32-byte ContractId hex).
  propTarget: "0000000000000000000000000000000000000000000000000000000000000001",
  propFunction: "noop",
  propArgsHex: "",
  propDeadline: 999999999,
  propSigner: "alice",
};

/** Five-beat proposals path (free browse via tabs + arrows). */
const BEATS = [
  { tab: "cast", beat: 1 },
  { tab: "council", beat: 2 },
  { tab: "check", beat: 3 },
  { tab: "proposal", beat: 4, beatPhase: null },
  { tab: "proposal", beat: 5, beatPhase: "finalize" },
];

const COPY_ICON_SVG =
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">` +
  `<rect x="9" y="9" width="13" height="13" rx="2"/>` +
  `<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>` +
  `</svg>`;

const COUNCIL_ICON_SVG =
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">` +
  `<circle cx="9" cy="8" r="3"/><circle cx="16" cy="9" r="2.5"/>` +
  `<path d="M3 19c1.5-3.5 4-5 6-5s4.5 1.5 6 5"/>` +
  `<path d="M14 19c.8-2 2.2-3 4-3 1.2 0 2.2.4 3 1.2"/>` +
  `</svg>`;

let propPreviewShown = false;
let storyAccountId = null;
let storyProposalId = null;
let demoMode = "mock";
let statusThreshold = null;
let statusApprovals = null;
/** Councils created during this demo run: { id, members[], threshold, nonce?, meta? } */
let demoCouncils = [];
/** Beat 2: selected identity names for Form council. */
let councilSelected = new Set();
/** Beat 3 / 4–5: selected demo council id. */
let selectedCouncilId = null;
/** Beat 4–5: single active approve-as signer. */
let activePropSigner = null;
/** Last cast click — soft hint for later beats. */
let lastClickedIdentity = null;
let cachedIdentities = [];

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

function el(id) {
  return document.getElementById(id);
}

function splitNames(s) {
  return String(s || "").split(",").map((x) => x.trim()).filter(Boolean);
}

function submitOk(out) {
  return out && out.outcome !== "panic" && out.tx_status !== "failed";
}

function showToast(message) {
  let toast = el("lab-toast");
  if (!toast) {
    toast = document.createElement("div");
    toast.id = "lab-toast";
    toast.setAttribute("role", "status");
    Object.assign(toast.style, {
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
    document.body.appendChild(toast);
  }
  toast.textContent = message;
  toast.style.opacity = "1";
  toast.hidden = false;
  clearTimeout(showToast._t);
  showToast._t = setTimeout(() => {
    toast.style.opacity = "0";
    setTimeout(() => { toast.hidden = true; }, 220);
  }, 4800);
}

function setOutcome(id, html, ok = true) {
  const node = el(id);
  if (!node) return;
  node.hidden = false;
  node.className = "outcome " + (ok ? "status-ok" : "status-err");
  node.innerHTML = html;
}

function outcomeChip(ok) {
  return `<span class="outcome-chip ${ok ? "ok" : "fail"}">${ok ? "success" : "failed"}</span>`;
}

function formatSubmitHtml(out) {
  const ok = submitOk(out);
  const parts = [];
  parts.push(outcomeChip(ok));
  parts.push(`<span class="outcome-title">${out.outcome || (ok ? "ok" : "error")}</span>`);
  const lines = [];
  if (out.tx_status) lines.push(`tx: ${out.tx_status}`);
  if (out.tx_hash) lines.push(`hash: ${String(out.tx_hash).slice(0, 16)}…`);
  if (out.panic_line) lines.push(`panic: ${out.panic_line}`);
  if (out.note) lines.push(out.note);
  if (out.check !== undefined && out.check !== null) lines.push(`check: ${out.check}`);
  if (lines.length) {
    parts.push(`<p class="outcome-body">${escapeHtml(lines.join("\n"))}</p>`);
  }
  return { html: parts.join(" "), ok };
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function showSubmitOutcome(outcomeId, out) {
  const f = formatSubmitHtml(out);
  setOutcome(outcomeId, f.html, f.ok);
}

function showErrorOutcome(outcomeId, message) {
  setOutcome(
    outcomeId,
    `${outcomeChip(false)}<span class="outcome-title">error</span>` +
      `<p class="outcome-body">${escapeHtml(message)}</p>`,
    false
  );
}

function applyDemoMode(mode) {
  demoMode = mode === "testnet" ? "testnet" : "mock";
  const badge = el("status-mode");
  if (badge) {
    badge.dataset.mode = demoMode;
    badge.textContent = demoMode === "testnet" ? "Testnet" : "Mock";
  }
}

function updateStatusStrip({ account, threshold, approvals } = {}) {
  if (account !== undefined && account !== null) {
    const node = el("status-account");
    if (node) node.textContent = `Account ${account}`;
  }
  if (threshold !== undefined && threshold !== null) {
    statusThreshold = threshold;
    const node = el("status-threshold");
    if (node) node.textContent = `Threshold ${threshold}`;
  }
  if (approvals !== undefined && approvals !== null) {
    statusApprovals = approvals;
    const node = el("status-approvals");
    if (node) {
      const t = statusThreshold != null ? `/${statusThreshold}` : "";
      node.textContent = `Approvals ${approvals}${t}`;
    }
  }
}

function applyAccountIds(id) {
  if (id === null || id === undefined || Number.isNaN(id)) return;
  storyAccountId = id;
  selectedCouncilId = id;
  const s = String(id);
  const propAcct = el("prop-account-id");
  if (propAcct) propAcct.value = s;
  updateStatusStrip({ account: id });
  renderCouncilsLists();
}

function syncPropApproveGate() {
  const propConfirm = el("prop-confirm");
  const propBtn = el("prop-approve-btn");
  if (!propConfirm || !propBtn) return;
  propBtn.disabled = !propConfirm.checked || !propPreviewShown;
}

function prefillStoryFields() {
  const thr = el("create-threshold");
  if (thr) thr.value = String(STORY.threshold);
  setReadonlyPropFields();
  const signer = el("prop-signer");
  if (signer) signer.value = STORY.propSigner;
  activePropSigner = STORY.propSigner;
  if (storyAccountId !== null) applyAccountIds(storyAccountId);
}

function setReadonlyPropFields() {
  const map = [
    ["prop-target", "prop-target-display", STORY.propTarget, (v) => (v.length > 18 ? `${v.slice(0, 10)}…${v.slice(-4)}` : v)],
    ["prop-function", "prop-function-display", STORY.propFunction, (v) => v],
    ["prop-args-hex", "prop-args-display", STORY.propArgsHex, (v) => (v ? v : "(empty)")],
    ["prop-deadline", "prop-deadline-display", String(STORY.propDeadline), (v) => v],
  ];
  for (const [hid, did, val, fmt] of map) {
    const hidden = el(hid);
    const disp = el(did);
    if (hidden) hidden.value = val;
    if (disp) disp.textContent = fmt(val);
  }
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

function pushDemoCouncil({ id, members, threshold, nonce, meta }) {
  const existing = demoCouncils.findIndex((c) => c.id === id);
  const entry = {
    id,
    members: members ? [...members] : [],
    threshold,
    nonce: nonce != null ? nonce : null,
    meta: meta || null,
  };
  if (existing >= 0) demoCouncils[existing] = { ...demoCouncils[existing], ...entry };
  else demoCouncils.push(entry);
  selectedCouncilId = id;
  renderCouncilsLists();
}

function renderSignerTile(id, { selected, deselected, active, onClick } = {}) {
  const row = document.createElement("div");
  row.className = "id-row";
  if (selected) row.classList.add("selected");
  if (deselected) row.classList.add("deselected");
  if (active) row.classList.add("active-signer");
  row.dataset.name = id.name;
  const kind = id.pk_only
    ? `<span class="pill">pk-only</span>`
    : `<span class="pill signing">signing</span>`;
  row.innerHTML =
    `<span class="id-name"><span class="id-name-text">${escapeHtml(id.name)}</span> ${kind}</span>` +
    `<span class="pk" title="${escapeHtml(id.pk_base58)}">${escapeHtml(id.pk_base58.slice(0, 14))}…</span>` +
    `<button type="button" class="tiny icon-btn" data-copy="${escapeHtml(id.pk_base58)}" title="Copy public key" aria-label="Copy public key">${COPY_ICON_SVG}</button>`;
  row.addEventListener("click", (ev) => {
    if (ev.target.closest("[data-copy]")) return;
    if (onClick) onClick(id, row, ev);
  });
  const copyBtn = row.querySelector("[data-copy]");
  if (copyBtn) {
    copyBtn.addEventListener("click", async (ev) => {
      ev.stopPropagation();
      try {
        await navigator.clipboard.writeText(id.pk_base58);
        showToast(`Copied ${id.name} pk`);
      } catch {
        prompt("Public key", id.pk_base58);
      }
    });
  }
  return row;
}

async function refreshIdentities() {
  const identities = await api("/api/identities");
  cachedIdentities = identities;
  renderCastList(identities);
  renderCouncilPickList(identities);
  renderPropSignerList(identities);
}

function renderCastList(identities) {
  const list = el("identities-list");
  if (!list) return;
  list.innerHTML = "";
  for (const id of identities) {
    list.appendChild(
      renderSignerTile(id, {
        onClick: (ident) => {
          lastClickedIdentity = ident.name;
          activePropSigner = ident.name;
          const signer = el("prop-signer");
          if (signer) signer.value = ident.name;
          // Soft select for council if not yet forming.
          if (!councilSelected.has(ident.name)) {
            councilSelected.add(ident.name);
          }
          renderCouncilPickList(cachedIdentities);
          renderPropSignerList(cachedIdentities);
          showToast(`${ident.name} ready for later beats`);
        },
      })
    );
  }
}

function renderCouncilPickList(identities) {
  const list = el("council-pick-list");
  if (!list) return;
  list.innerHTML = "";
  const anySelected = councilSelected.size > 0;
  for (const id of identities) {
    const isSel = councilSelected.has(id.name);
    list.appendChild(
      renderSignerTile(id, {
        selected: isSel,
        deselected: anySelected && !isSel,
        onClick: (ident) => {
          if (councilSelected.has(ident.name)) councilSelected.delete(ident.name);
          else councilSelected.add(ident.name);
          renderCouncilPickList(cachedIdentities);
        },
      })
    );
  }
}

function renderPropSignerList(identities) {
  const list = el("prop-signer-list");
  if (!list) return;
  list.innerHTML = "";
  const signing = identities.filter((i) => !i.pk_only);
  for (const id of signing) {
    const isActive = activePropSigner === id.name;
    list.appendChild(
      renderSignerTile(id, {
        active: isActive,
        deselected: activePropSigner && !isActive,
        onClick: (ident) => {
          activePropSigner = ident.name;
          const signer = el("prop-signer");
          if (signer) signer.value = ident.name;
          renderPropSignerList(cachedIdentities);
        },
      })
    );
  }
}

function renderCouncilCard(c, { onSelect } = {}) {
  const card = document.createElement("div");
  card.className = "council-card" + (selectedCouncilId === c.id ? " selected" : "");
  card.dataset.id = String(c.id);
  card.innerHTML =
    `<button type="button" class="council-detail-btn" data-detail="${c.id}" title="Show members">detail</button>` +
    `<span class="council-card-icon">${COUNCIL_ICON_SVG}</span>` +
    `<span class="council-card-id">#${c.id}</span>` +
    `<span class="council-card-meta">${c.threshold}-of-${c.members.length || "?"}</span>`;
  card.addEventListener("click", (ev) => {
    if (ev.target.closest("[data-detail]")) return;
    if (onSelect) onSelect(c);
  });
  const detailBtn = card.querySelector("[data-detail]");
  if (detailBtn) {
    detailBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      openCouncilDetail(c);
    });
  }
  return card;
}

function renderCouncilsLists() {
  for (const listId of ["councils-list", "prop-councils-list"]) {
    const list = el(listId);
    if (!list) continue;
    list.innerHTML = "";
    if (!demoCouncils.length) {
      const empty = document.createElement("p");
      empty.className = "councils-empty";
      empty.textContent = "No councils yet — form one on Beat 2.";
      list.appendChild(empty);
      continue;
    }
    for (const c of demoCouncils) {
      list.appendChild(
        renderCouncilCard(c, {
          onSelect: (council) => {
            selectedCouncilId = council.id;
            applyAccountIds(council.id);
            updateStatusStrip({ threshold: council.threshold });
            if (listId === "councils-list") {
              fetchCouncilOutcome(council.id);
            }
            renderCouncilsLists();
          },
        })
      );
    }
  }
}

function openCouncilDetail(c) {
  const pop = el("council-detail-popover");
  const title = el("council-detail-title");
  const membersEl = el("council-detail-members");
  if (!pop || !membersEl) return;
  if (title) title.textContent = `Council #${c.id} · ${c.threshold}-of-${c.members.length}`;
  membersEl.innerHTML = "";
  for (const name of c.members) {
    const ident = cachedIdentities.find((i) => i.name === name) || {
      name,
      pk_base58: "(unknown)",
      pk_only: false,
    };
    membersEl.appendChild(renderSignerTile(ident, {}));
  }
  if (!c.members.length) {
    membersEl.innerHTML = `<p class="councils-empty">No member names stored yet.</p>`;
  }
  pop.hidden = false;
}

function closeCouncilDetail() {
  const pop = el("council-detail-popover");
  if (pop) pop.hidden = true;
}

async function fetchCouncilOutcome(id) {
  setOutcome("query-outcome", `<p class="outcome-body">Looking up council #${id}…</p>`, true);
  try {
    const out = await api(`/api/account/${id}`);
    if (!out) {
      setOutcome("query-outcome", `${outcomeChip(false)}<span class="outcome-title">not found</span>`, false);
      return;
    }
    let meta = null;
    try { meta = await api(`/api/account/${id}/meta`); } catch (_) {}
    const members = Array.isArray(out.members)
      ? out.members.map((m) => (typeof m === "string" ? m : m.name || JSON.stringify(m)))
      : [];
    // Prefer names from demoCouncils when available.
    const demo = demoCouncils.find((c) => c.id === Number(id) || c.id === id);
    const memberNames = demo && demo.members.length ? demo.members : members;
    pushDemoCouncil({
      id: Number(id),
      members: memberNames,
      threshold: out.threshold,
      nonce: out.nonce != null ? out.nonce : (meta && meta.nonce),
      meta,
    });
    updateStatusStrip({
      account: id,
      threshold: out.threshold,
      approvals: statusApprovals != null ? statusApprovals : 0,
    });
    const lines = [
      `threshold: ${out.threshold}`,
      `members: ${memberNames.length ? memberNames.join(", ") : "(see keys)"}`,
    ];
    if (out.nonce != null) lines.push(`nonce: ${out.nonce}`);
    if (meta) {
      if (meta.nonce != null && out.nonce == null) lines.push(`nonce: ${meta.nonce}`);
    }
    setOutcome(
      "query-outcome",
      `${outcomeChip(true)}<span class="outcome-title">Council #${id}</span>` +
        `<p class="outcome-body">${escapeHtml(lines.join("\n"))}</p>`,
      true
    );
  } catch (e) {
    showErrorOutcome("query-outcome", e.message);
  }
}

function setIdentityPopover(open) {
  const pop = el("identity-popover");
  const btn = el("btn-add-identity");
  if (!pop || !btn) return;
  pop.hidden = !open;
  btn.setAttribute("aria-expanded", open ? "true" : "false");
  if (open) {
    const input = el("new-identity-name");
    if (input) {
      input.value = "";
      setTimeout(() => input.focus(), 30);
    }
  }
}

async function createIdentity() {
  const input = el("new-identity-name");
  const name = input ? input.value.trim() : "";
  if (!name) return;
  try {
    await api("/api/identities", { method: "POST", body: JSON.stringify({ name }) });
    if (input) input.value = "";
    setIdentityPopover(false);
    await refreshIdentities();
    showToast(`Created ${name}`);
  } catch (e) {
    alert(e.message);
  }
}

async function submitCreateAccount() {
  const members = [...councilSelected];
  if (!members.length) {
    showErrorOutcome("create-outcome", "Select at least one member.");
    return;
  }
  const thrEl = el("create-threshold");
  const threshold = thrEl ? parseInt(thrEl.value, 10) : STORY.threshold;
  setOutcome("create-outcome", `<p class="outcome-body">Creating council…</p>`, true);
  const icon = el("council-success-icon");
  if (icon) {
    icon.hidden = true;
    icon.classList.remove("pop");
  }
  try {
    const out = await api("/api/account/create", {
      method: "POST",
      body: JSON.stringify({ members, threshold }),
    });
    let createdId = null;
    try {
      const n = await api("/api/account/next-id");
      const next = typeof n === "number" ? n : (typeof n === "string" && /^\d+$/.test(n) ? parseInt(n, 10) : null);
      if (out.outcome !== "panic" && next != null) createdId = next - 1;
    } catch (_) {}
    showSubmitOutcome("create-outcome", out);
    if (submitOk(out) && createdId != null) {
      pushDemoCouncil({ id: createdId, members, threshold });
      applyAccountIds(createdId);
      updateStatusStrip({ threshold, approvals: 0 });
      if (icon) {
        icon.hidden = false;
        // restart animation
        void icon.offsetWidth;
        icon.classList.add("pop");
      }
      setOutcome(
        "create-outcome",
        `${outcomeChip(true)}<span class="outcome-title">Council #${createdId} formed</span>` +
          `<p class="outcome-body">${threshold}-of-${members.length}: ${escapeHtml(members.join(", "))}</p>`,
        true
      );
    }
  } catch (e) {
    showErrorOutcome("create-outcome", e.message);
  }
}

async function proposalCreate() {
  const acctEl = el("prop-account-id");
  let account = acctEl ? parseInt(acctEl.value, 10) : NaN;
  if (Number.isNaN(account) && selectedCouncilId != null) account = Number(selectedCouncilId);
  if (Number.isNaN(account)) {
    showErrorOutcome("prop-outcome", "Select a registry account first.");
    return;
  }
  const target = (el("prop-target") && el("prop-target").value.trim()) || STORY.propTarget;
  const functionName = (el("prop-function") && el("prop-function").value.trim()) || STORY.propFunction;
  const args_hex = (el("prop-args-hex") && el("prop-args-hex").value.trim()) || "";
  const deadlineEl = el("prop-deadline");
  const deadline = deadlineEl ? (parseInt(deadlineEl.value, 10) || 0) : STORY.propDeadline;
  setOutcome("prop-outcome", `<p class="outcome-body">Proposing…</p>`, true);
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
    const propId = el("prop-id");
    const propDisp = el("prop-id-display");
    if (propId) propId.value = String(out.allocated_id_hint);
    if (propDisp) propDisp.textContent = String(out.allocated_id_hint);
    showSubmitOutcome("prop-outcome", {
      ...submit,
      note: `proposal id ${out.allocated_id_hint}`,
    });
    updateStatusStrip({ approvals: 0 });
    propPreviewShown = false;
    const confirm = el("prop-confirm");
    if (confirm) confirm.checked = false;
    syncPropApproveGate();
  } catch (e) {
    showErrorOutcome("prop-outcome", e.message);
  }
}

async function proposalStatus() {
  const idEl = el("prop-id");
  const id = idEl ? idEl.value : "";
  if (!id) {
    showErrorOutcome("prop-outcome", "Propose first to get a proposal id.");
    return;
  }
  setOutcome("prop-outcome", `<p class="outcome-body">Fetching status…</p>`, true);
  try {
    const out = await api(`/api/proposal/${id}`);
    if (!out) {
      setOutcome("prop-outcome", `${outcomeChip(false)}<span class="outcome-title">not found</span>`, false);
      return;
    }
    if (out.approvals_len != null) updateStatusStrip({ approvals: out.approvals_len });
    const lines = [
      `proposal #${id}`,
      `approvals: ${out.approvals_len != null ? out.approvals_len : "?"}`,
      out.status != null ? `status: ${out.status}` : null,
      out.finalized != null ? `finalized: ${out.finalized}` : null,
    ].filter(Boolean);
    setOutcome(
      "prop-outcome",
      `${outcomeChip(true)}<span class="outcome-title">Status</span>` +
        `<p class="outcome-body">${escapeHtml(lines.join("\n"))}</p>`,
      true
    );
  } catch (e) {
    showErrorOutcome("prop-outcome", e.message);
  }
}

async function proposalPreview() {
  const idEl = el("prop-id");
  const id = idEl ? idEl.value : "";
  if (!id) {
    showErrorOutcome("prop-outcome", "Propose first to get a proposal id.");
    return;
  }
  const box = el("prop-preview");
  setOutcome("prop-outcome", `<p class="outcome-body">Previewing fingerprint…</p>`, true);
  try {
    const out = await api(`/api/proposal/${id}/preview`);
    if (box) {
      box.hidden = false;
      box.className = "outcome fingerprint status-ok";
      box.innerHTML =
        `<strong>Fingerprint</strong><br>` +
        `mnemonic: ${escapeHtml(out.digest_mnemonic)}<br>` +
        `safety: ${escapeHtml(String(out.digest_safety_number))}<br>` +
        `digest: ${escapeHtml(String(out.digest_hex).slice(0, 24))}…<br>` +
        `fn=${escapeHtml(out.function_name)} · deadline=${escapeHtml(String(out.deadline))}`;
    }
    const confirm = el("prop-confirm");
    if (confirm) confirm.checked = false;
    propPreviewShown = true;
    syncPropApproveGate();
    setOutcome(
      "prop-outcome",
      `${outcomeChip(true)}<span class="outcome-title">Preview ready</span>` +
        `<p class="outcome-body">Compare fingerprint, then confirm + approve.</p>`,
      true
    );
  } catch (e) {
    propPreviewShown = false;
    syncPropApproveGate();
    showErrorOutcome("prop-outcome", e.message);
  }
}

async function proposalApprove() {
  const idEl = el("prop-id");
  const signerEl = el("prop-signer");
  const id = idEl ? idEl.value : "";
  const signer = signerEl ? signerEl.value.trim() : (activePropSigner || "");
  if (!id) {
    showErrorOutcome("prop-outcome", "Propose first.");
    return;
  }
  if (!signer) {
    showErrorOutcome("prop-outcome", "Select a signer tile.");
    return;
  }
  if (!propPreviewShown) {
    showErrorOutcome("prop-outcome", "Preview the proposal first, then confirm + approve.");
    return;
  }
  const confirm = el("prop-confirm");
  if (!confirm || !confirm.checked) {
    showErrorOutcome("prop-outcome", "Check the confirm box after preview.");
    return;
  }
  setOutcome("prop-outcome", `<p class="outcome-body">Approving as ${escapeHtml(signer)}…</p>`, true);
  try {
    const out = await api(`/api/proposal/${id}/approve`, {
      method: "POST",
      body: JSON.stringify({ signer, confirm: true }),
    });
    const submit = out.submit || out;
    showSubmitOutcome("prop-outcome", {
      ...submit,
      note: `approved as ${signer}`,
    });
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
  } catch (e) {
    showErrorOutcome("prop-outcome", e.message);
  }
}

async function proposalFinalize() {
  const idEl = el("prop-id");
  const id = idEl ? idEl.value : "";
  if (!id) {
    showErrorOutcome("prop-outcome", "Propose first.");
    return;
  }
  setOutcome("prop-outcome", `<p class="outcome-body">Finalizing…</p>`, true);
  try {
    const out = await api(`/api/proposal/${id}/finalize`, { method: "POST", body: "{}" });
    showSubmitOutcome("prop-outcome", out);
  } catch (e) {
    showErrorOutcome("prop-outcome", e.message);
  }
}

function prepareBeatEntry(index) {
  const beat = BEATS[index];
  if (!beat) return;
  if (beat.beat === 4) {
    setReadonlyPropFields();
    activePropSigner = "alice";
    const signer = el("prop-signer");
    if (signer) signer.value = "alice";
    const confirm = el("prop-confirm");
    if (confirm) confirm.checked = false;
    propPreviewShown = false;
    const btn = el("prop-approve-btn");
    if (btn) btn.disabled = true;
    if (storyAccountId !== null) applyAccountIds(storyAccountId);
    renderPropSignerList(cachedIdentities);
  }
  if (beat.beat === 5) {
    activePropSigner = "bob";
    const signer = el("prop-signer");
    if (signer) signer.value = "bob";
    const confirm = el("prop-confirm");
    if (confirm) confirm.checked = false;
    propPreviewShown = false;
    const btn = el("prop-approve-btn");
    if (btn) btn.disabled = true;
    if (storyProposalId !== null) {
      const propId = el("prop-id");
      const propDisp = el("prop-id-display");
      if (propId) propId.value = String(storyProposalId);
      if (propDisp) propDisp.textContent = String(storyProposalId);
    }
    renderPropSignerList(cachedIdentities);
  }
}

const SLIDE_HASHES = new Set(["top", "demo", "use-cases", ""]);

function tabFromUrl() {
  const params = new URLSearchParams(location.search);
  const q = params.get("tab");
  if (q) return q;
  const hash = (location.hash || "").replace(/^#/, "");
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
  const target = Math.max(0, Math.min(BEATS.length - 1, i + delta));
  const beat = BEATS[target];
  const dot = document.querySelector(`.beat-dots .tab[data-beat="${target + 1}"]`);
  if (dot) {
    document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
      const on = btn === dot;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-selected", on ? "true" : "false");
    });
  }
  prepareBeatEntry(target);
  activateTab(beat.tab, {
    fromBeatNav: true,
    beatPhase: beat.beatPhase === "finalize" ? "finalize" : undefined,
  });
}

function activateTab(name, opts = {}) {
  document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
    if (opts.fromBeatNav) return;
    const on = btn.dataset.tab === name;
    btn.classList.toggle("active", on);
    btn.setAttribute("aria-selected", on ? "true" : "false");
  });
  if (!opts.fromBeatNav && name === "proposal") {
    const prefer = opts.beatPhase === "finalize" ? "5" : "4";
    document.querySelectorAll('.beat-dots .tab[data-tab="proposal"]').forEach((btn) => {
      const on = btn.dataset.beat === prefer;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-selected", on ? "true" : "false");
    });
  }
  // When clicking a beat tab with data-beat, prefer that phase.
  if (!opts.fromBeatNav && opts.beat != null) {
    document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
      const on = String(btn.dataset.beat) === String(opts.beat);
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-selected", on ? "true" : "false");
    });
    const idx = Math.max(0, Math.min(BEATS.length - 1, opts.beat - 1));
    prepareBeatEntry(idx);
  }
  document.querySelectorAll(".beat-viewport .tab-panel").forEach((panel) => {
    const on = panel.id === `panel-${name}`;
    panel.classList.toggle("active", on);
    if (on) panel.removeAttribute("hidden");
    else panel.setAttribute("hidden", "");
  });
  if (name === "check") renderCouncilsLists();
  if (name === "proposal") {
    renderCouncilsLists();
    renderPropSignerList(cachedIdentities);
  }
  if (name === "council") renderCouncilPickList(cachedIdentities);
}

function on(id, ev, fn) {
  const node = el(id);
  if (node) node.addEventListener(ev, fn);
}

function wireIdentityPopover() {
  on("btn-add-identity", "click", (ev) => {
    ev.stopPropagation();
    const pop = el("identity-popover");
    setIdentityPopover(pop ? pop.hidden : true);
  });
  on("btn-create-identity", "click", () => createIdentity());
  on("btn-cancel-identity", "click", () => setIdentityPopover(false));
  on("new-identity-name", "keydown", (ev) => {
    if (ev.key === "Enter") {
      ev.preventDefault();
      createIdentity();
    } else if (ev.key === "Escape") {
      setIdentityPopover(false);
    }
  });
  document.addEventListener("click", (ev) => {
    const wrap = document.querySelector(".add-identity-wrap");
    const pop = el("identity-popover");
    if (!wrap || !pop || pop.hidden) return;
    if (!wrap.contains(ev.target)) setIdentityPopover(false);
  });
}

function wireConfirmGates() {
  const propConfirm = el("prop-confirm");
  if (propConfirm) {
    propConfirm.addEventListener("change", () => syncPropApproveGate());
  }
}

function wireBeatTabs() {
  document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
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
}

// Highlight page-nav dots from scroll position (cover / demo / use-cases).
(function wireSlideDots() {
  const root = el("slides");
  const dots = [...document.querySelectorAll(".dots .dot")];
  if (!root || !dots.length) return;
  const sections = dots
    .map((d) => document.getElementById(d.dataset.slide))
    .filter(Boolean);

  function scrollToHash(behavior) {
    const id = (location.hash || "#top").replace(/^#/, "") || "top";
    const node = document.getElementById(id);
    if (!node || !root.contains(node)) return;
    const top = node.offsetTop - root.offsetTop;
    root.scrollTo({ top, behavior: behavior || "auto" });
  }

  const sync = () => {
    let best = 0;
    let bestDist = Infinity;
    const rootRect = root.getBoundingClientRect();
    sections.forEach((sec, i) => {
      const dist = Math.abs(sec.getBoundingClientRect().top - rootRect.top);
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
  scrollToHash("auto");
  sync();
})();

// Init
prefillStoryFields();
wireIdentityPopover();
wireConfirmGates();
wireBeatTabs();
on("btn-beat-prev", "click", () => goBeat(-1));
on("btn-beat-next", "click", () => goBeat(1));
on("btn-close-council-detail", "click", () => closeCouncilDetail());

(async function boot() {
  try {
    const status = await api("/api/setup/status");
    if (status && status.demo_mode) applyDemoMode(status.demo_mode);
  } catch (_) {}
  try {
    await ensureStoryCast();
  } catch (e) {
    console.error(e);
    try { await refreshIdentities(); } catch (e2) { console.error(e2); }
  }
  // Soft-select story cast for Form council convenience.
  for (const name of STORY.cast) councilSelected.add(name);
  renderCouncilPickList(cachedIdentities);
  updateStatusStrip({ threshold: STORY.threshold, approvals: 0 });

  const initialTab = tabFromUrl();
  if (initialTab) activateTab(initialTab);
})();
