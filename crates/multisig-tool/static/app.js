// Plain JS, no build step. Token from index.html.
// When TOKEN is still "__TOKEN__" (static Lab / Cloudflare), use in-browser MockLedger.
// When multisig-tool serve injects a real token, call the live /api/* mock or testnet backend.

const TOKEN = window.MULTISIG_TOOL_TOKEN;
const USE_FRONTEND_MOCK =
  window.MULTISIG_FRONTEND_MOCK === true ||
  !TOKEN ||
  TOKEN === "__TOKEN__";

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

const PURPOSES = ["Payout", "Config change", "Member rotation"];

/** Five-beat path (free browse via tabs + arrows). */
const BEATS = [
  { tab: "cast", beat: 1 },
  { tab: "council", beat: 2 },
  { tab: "check", beat: 3 },
  { tab: "proposal", beat: 4 },
  { tab: "finalize", beat: 5 },
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

let intentConfirmed = false;
let storyAccountId = null;
let storyProposalId = null;
let demoMode = "mock";
let statusThreshold = null;
let statusApprovals = null;
/** Councils created during this demo run: { id, members[], threshold, nonce?, meta? } */
let demoCouncils = [];
/** Proposals from this demo run: { id, accountId, purpose, description, status, approvals } */
let demoProposals = [];
/** Beat 2: selected identity names for Form council. */
let councilSelected = new Set();
/** Beat 3+: selected demo council id. */
let selectedCouncilId = null;
/** Beat 5: selected proposal id. */
let selectedProposalId = null;
/** Beat 5: single active approve-as signer. */
let activePropSigner = null;
/** Beat 1: who you are (blue header chip). */
let youIdentity = null;
let cachedIdentities = [];

async function api(path, opts = {}) {
  if (USE_FRONTEND_MOCK) {
    if (!window.MockLab || typeof window.MockLab.mockApi !== "function") {
      throw new Error("Frontend MockLedger missing — load /mock-ledger.js before app.js");
    }
    return window.MockLab.mockApi(path, opts);
  }
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

function submitOk(out) {
  return out && out.outcome !== "panic" && out.tx_status !== "failed";
}

const BANNER_MS = 10000;

function showLabBanner(html, ok = true) {
  const slot = el("demo-banner-slot");
  if (!slot) return;
  const existing = slot.querySelector(".lab-banner");
  const mount = () => {
    slot.innerHTML = "";
    const banner = document.createElement("div");
    banner.className = "lab-banner " + (ok ? "status-ok" : "status-err");
    banner.setAttribute("role", "status");
    banner.innerHTML = html;
    slot.appendChild(banner);
    clearTimeout(showLabBanner._t);
    showLabBanner._t = setTimeout(() => {
      banner.classList.add("fly-out");
      setTimeout(() => {
        if (banner.parentNode === slot) banner.remove();
      }, 280);
    }, BANNER_MS);
  };
  if (existing) {
    existing.classList.add("fly-out");
    clearTimeout(showLabBanner._t);
    setTimeout(mount, 260);
  } else {
    mount();
  }
}

function showToast(message) {
  showLabBanner(`<span class="outcome-meta">${escapeHtml(message)}</span>`, true);
}

function setOutcome(_id, html, ok = true) {
  // Success/failure (and progress notes) pop in above the demo header.
  showLabBanner(html, ok);
  // Hide any legacy in-pane outcome nodes so they don't steal space.
  if (_id) {
    const node = el(_id);
    if (node) {
      node.hidden = true;
      node.innerHTML = "";
    }
  }
}

function outcomeChip(ok) {
  return `<span class="outcome-chip ${ok ? "ok" : "fail"}">${ok ? "success" : "failed"}</span>`;
}

function formatSubmitHtml(out) {
  const ok = submitOk(out);
  const parts = [];
  parts.push(outcomeChip(ok));
  parts.push(`<span class="outcome-title">${escapeHtml(out.outcome || (ok ? "ok" : "error"))}</span>`);
  const lines = [];
  if (out.tx_status) lines.push(`tx: ${out.tx_status}`);
  if (out.tx_hash) lines.push(`hash: ${String(out.tx_hash).slice(0, 16)}…`);
  if (out.panic_line) lines.push(`panic: ${out.panic_line}`);
  if (out.note) lines.push(out.note);
  if (out.check !== undefined && out.check !== null) lines.push(`check: ${out.check}`);
  if (lines.length) {
    parts.push(`<span class="outcome-meta">${escapeHtml(lines.join(" · "))}</span>`);
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
      `<span class="outcome-meta">${escapeHtml(message)}</span>`,
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

function updateYouChip() {
  const chip = el("status-you");
  if (!chip) return;
  chip.hidden = false;
  chip.textContent = youIdentity ? `You: ${youIdentity}` : "You: —";
  chip.classList.toggle("is-set", !!youIdentity);
}

function updateHeaderCouncil({ animate } = {}) {
  const wrap = el("header-council");
  const icon = el("header-council-icon");
  const idEl = el("header-council-id");
  if (!wrap) return;
  const id = selectedCouncilId != null ? selectedCouncilId : storyAccountId;
  if (id == null) {
    wrap.hidden = true;
    wrap.setAttribute("aria-hidden", "true");
    wrap.classList.remove("pop");
    return;
  }
  wrap.hidden = false;
  wrap.setAttribute("aria-hidden", "false");
  if (icon) icon.innerHTML = COUNCIL_ICON_SVG;
  if (idEl) idEl.textContent = `#${id}`;
  if (animate) {
    wrap.classList.remove("pop");
    void wrap.offsetWidth;
    wrap.classList.add("pop");
  }
}

function updateStatusStrip({ account, threshold, approvals, proposal } = {}) {
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
  }
  if (proposal !== undefined) {
    updateHeaderApprovals(proposal);
    return;
  }
  const p = selectedProposalId != null
    ? demoProposals.find((x) => String(x.id) === String(selectedProposalId))
    : null;
  updateHeaderApprovals(p || null);
}

function updateHeaderApprovals(p) {
  const text = el("status-approvals-text");
  const state = el("status-prop-state");
  const n = statusApprovals != null ? statusApprovals : 0;
  const thr = statusThreshold;
  if (text) {
    text.textContent = thr != null ? `Approvals ${n}/${thr}` : `Approvals ${n}`;
  }
  if (!state) return;
  const onFinalize = !!document.querySelector("#panel-finalize.active");
  // Open/Finalized only while a proposal is selected on Finalize.
  if (!p || !onFinalize) {
    state.hidden = true;
    state.textContent = "";
    state.className = "approvals-state";
    return;
  }
  const finalized = p.status === "finalized" || p.finalized === true;
  state.hidden = false;
  state.textContent = finalized ? "Finalized" : "Open";
  state.className = "approvals-state " + (finalized ? "finalized" : "open");
  const count = p.approvals != null ? p.approvals : n;
  statusApprovals = count;
  const council = demoCouncils.find((c) => c.id === p.accountId);
  const t = thr != null ? thr : (council && council.threshold);
  if (text) {
    text.textContent = t != null ? `Approvals ${count}/${t}` : `Approvals ${count}`;
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
  updateHeaderCouncil();
  renderCouncilsLists();
}

function syncPropApproveGate() {
  const propBtn = el("prop-approve-btn");
  if (!propBtn) return;
  // Proposal id 0 is valid — don't treat it as falsy.
  propBtn.disabled = !intentConfirmed || selectedProposalId == null;
}

function setYouIdentity(name) {
  // Toggle off when clicking the same cast member again.
  if (name && youIdentity === name) {
    youIdentity = null;
  } else {
    youIdentity = name || null;
  }
  if (youIdentity) {
    activePropSigner = youIdentity;
    const signer = el("prop-signer");
    if (signer) signer.value = youIdentity;
  }
  updateYouChip();
  renderCastList(cachedIdentities);
  renderPropSignerList(cachedIdentities);
}

function prefillStoryFields() {
  const thr = el("create-threshold");
  if (thr) thr.value = String(STORY.threshold);
  const purpose = el("prop-purpose");
  if (purpose) purpose.value = "Payout";
  const desc = el("prop-description");
  if (desc && !desc.value) desc.value = "pay vendor invoice for Q3 tooling";
  const signer = el("prop-signer");
  if (signer) signer.value = STORY.propSigner;
  activePropSigner = STORY.propSigner;
  if (storyAccountId !== null) applyAccountIds(storyAccountId);
}

function purposeToFunction(purpose) {
  const map = {
    Payout: "noop",
    "Config change": "noop",
    "Member rotation": "noop",
  };
  return map[purpose] || STORY.propFunction;
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
  updateHeaderCouncil();
}

function pushDemoProposal(entry) {
  const existing = demoProposals.findIndex((p) => p.id === entry.id);
  if (existing >= 0) demoProposals[existing] = { ...demoProposals[existing], ...entry };
  else demoProposals.push(entry);
  selectedProposalId = entry.id;
  storyProposalId = entry.id;
  const propId = el("prop-id");
  if (propId) propId.value = String(entry.id);
  renderProposalsList();
  refreshIntentCard();
}

function renderSignerTile(id, { selected, deselected, active, youSelected, onClick } = {}) {
  const row = document.createElement("div");
  row.className = "id-row";
  if (selected) row.classList.add("selected");
  if (youSelected) row.classList.add("you-selected");
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
    const isYou = youIdentity === id.name;
    list.appendChild(
      renderSignerTile(id, {
        youSelected: isYou,
        deselected: !!youIdentity && !isYou,
        onClick: (ident) => {
          const wasYou = youIdentity === ident.name;
          setYouIdentity(ident.name);
          if (!wasYou) {
            // Soft-select for council when choosing “you”.
            if (!councilSelected.has(ident.name)) {
              councilSelected.add(ident.name);
            }
            renderCouncilPickList(cachedIdentities);
            showToast(`You are ${ident.name}`);
          } else {
            showToast(`${ident.name} deselected`);
          }
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
    const isYou = youIdentity === id.name;
    list.appendChild(
      renderSignerTile(id, {
        active: isActive,
        youSelected: isYou && isActive,
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

function renderCouncilCard(c, { onSelect, showDetail } = {}) {
  const card = document.createElement("div");
  card.className = "council-card" + (selectedCouncilId === c.id ? " selected" : "");
  card.dataset.id = String(c.id);
  const detailBtn = showDetail !== false
    ? `<button type="button" class="council-detail-btn" data-detail="${c.id}" title="Show members">detail</button>`
    : "";
  card.innerHTML =
    detailBtn +
    `<span class="council-card-icon">${COUNCIL_ICON_SVG}</span>` +
    `<span class="council-card-id">Council #${c.id}</span>` +
    `<span class="council-card-meta">Threshold ${c.threshold}-of-${c.members.length || "?"}</span>`;
  card.addEventListener("click", (ev) => {
    if (ev.target.closest("[data-detail]")) return;
    if (onSelect) onSelect(c);
  });
  const detail = card.querySelector("[data-detail]");
  if (detail) {
    detail.addEventListener("click", (ev) => {
      ev.stopPropagation();
      openCouncilDetail(c);
    });
  }
  return card;
}

function renderCouncilsLists() {
  const list = el("councils-list");
  if (!list) return;
  list.innerHTML = "";
  if (!demoCouncils.length) {
    const empty = document.createElement("p");
    empty.className = "councils-empty";
    empty.textContent = "No councils yet — form one on Beat 2.";
    list.appendChild(empty);
    return;
  }
  for (const c of demoCouncils) {
    list.appendChild(
      renderCouncilCard(c, {
        showDetail: true,
        onSelect: (council) => {
          selectedCouncilId = council.id;
          applyAccountIds(council.id);
          updateStatusStrip({ threshold: council.threshold });
          fetchCouncilOutcome(council.id);
          renderCouncilsLists();
        },
      })
    );
  }
}

function renderProposalsList() {
  const list = el("proposals-list") || el("prop-proposals-list");
  if (!list) return;
  list.innerHTML = "";
  if (!demoProposals.length) {
    const empty = document.createElement("p");
    empty.className = "councils-empty";
    empty.textContent = "No proposals yet — create one on Beat 4.";
    list.appendChild(empty);
    return;
  }
  // Newest first for visual priority; latest stays selected by default.
  const ordered = [...demoProposals].sort((a, b) => Number(b.id) - Number(a.id));
  for (const p of ordered) {
    const selected = String(selectedProposalId) === String(p.id);
    const confirmed = !!(selected && (p.intentConfirmed || intentConfirmed));
    const card = document.createElement("div");
    card.className = "proposal-card"
      + (selected ? " selected" : "")
      + (confirmed ? " confirmed" : "");
    card.dataset.id = String(p.id);
    let html =
      `<span class="proposal-card-council">Council #${escapeHtml(String(p.accountId))}</span>` +
      `<span class="proposal-card-id">Proposal #${escapeHtml(String(p.id))}</span>` +
      `<span class="proposal-card-purpose">${escapeHtml(p.purpose || "Intent")}</span>`;
    if (confirmed && p.description) {
      html += `<span class="proposal-card-desc">${escapeHtml(p.description)}</span>`;
    }
    card.innerHTML = html;
    card.addEventListener("click", () => selectProposal(p.id));
    list.appendChild(card);
  }
}

function selectProposal(id) {
  const p = demoProposals.find((x) => x.id === id || String(x.id) === String(id));
  if (!p) return;
  selectedProposalId = p.id;
  storyProposalId = p.id;
  const propId = el("prop-id");
  if (propId) propId.value = String(p.id);
  if (p.accountId != null) applyAccountIds(p.accountId);
  intentConfirmed = !!p.intentConfirmed;
  syncPropApproveGate();
  renderProposalsList();
  refreshIntentCard();
  updateStatusCard(p);
  updateApproveSection();
}

function refreshIntentCard() {
  const card = el("intent-card");
  const purposeEl = el("intent-purpose");
  const descEl = el("intent-description");
  if (!card) return;
  const p = demoProposals.find((x) => x.id === selectedProposalId || String(x.id) === String(selectedProposalId));
  if (!p || selectedProposalId == null) {
    card.hidden = true;
    updateApproveSection();
    return;
  }
  // After confirm, collapse intent into the proposal tile.
  if (p.intentConfirmed || intentConfirmed) {
    card.hidden = true;
    updateApproveSection();
    return;
  }
  card.hidden = false;
  if (purposeEl) purposeEl.textContent = p.purpose || "Intent";
  if (descEl) descEl.textContent = p.description || "(no description)";
  const confirmBtn = el("btn-intent-confirm");
  if (confirmBtn) {
    confirmBtn.disabled = false;
    confirmBtn.textContent = "Looks right — continue";
  }
  updateApproveSection();
}

function updateApproveSection() {
  const section = el("approve-section");
  if (!section) return;
  const show = selectedProposalId != null && intentConfirmed;
  section.hidden = !show;
  if (show) {
    renderPropSignerList(cachedIdentities);
    const p = demoProposals.find((x) => x.id === selectedProposalId || String(x.id) === String(selectedProposalId));
    if (p) updateStatusCard(p);
  }
}

function updateStatusCard(p) {
  // Status lives in the header Approvals pill (Open / Finalized).
  if (!p) {
    updateHeaderApprovals(null);
    return;
  }
  if (p.approvals != null) statusApprovals = p.approvals;
  const council = demoCouncils.find((c) => c.id === p.accountId);
  if (council && council.threshold != null) statusThreshold = council.threshold;
  updateHeaderApprovals(p);
}

function openCouncilDetail(c) {
  const pop = el("council-detail-popover");
  const title = el("council-detail-title");
  const membersEl = el("council-detail-members");
  if (!pop || !membersEl) return;
  if (title) title.textContent = `Council #${c.id} · Threshold ${c.threshold}-of-${c.members.length}`;
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
  setOutcome("query-outcome", `<span class="outcome-meta">Looking up council #${id}…</span>`, true);
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
    setOutcome(
      "query-outcome",
      `${outcomeChip(true)}<span class="outcome-title">Council #${id}</span>` +
        `<span class="outcome-meta"><strong>Threshold:</strong> ${out.threshold}-of-${memberNames.length || "?"}</span>` +
        `<span class="outcome-meta"><strong>Members:</strong> ${escapeHtml(memberNames.length ? memberNames.join(", ") : "(see keys)")}</span>`,
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
  setOutcome("create-outcome", `<span class="outcome-meta">Creating council…</span>`, true);
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
    if (submitOk(out) && createdId != null) {
      pushDemoCouncil({ id: createdId, members, threshold });
      applyAccountIds(createdId);
      updateStatusStrip({ threshold, approvals: 0 });
      updateHeaderCouncil({ animate: true });
      setOutcome(
        "create-outcome",
        `${outcomeChip(true)}<span class="outcome-title">Council #${createdId} formed</span>` +
          `<span class="outcome-meta"><strong>Threshold:</strong> ${threshold}-of-${members.length}</span>` +
          `<span class="outcome-meta"><strong>Members:</strong> ${escapeHtml(members.join(", "))}</span>`,
        true
      );
    } else {
      showSubmitOutcome("create-outcome", out);
    }
  } catch (e) {
    showErrorOutcome("create-outcome", e.message);
  }
}

function getSelectedPurpose() {
  const hid = el("prop-purpose");
  return (hid && hid.value.trim()) || "Payout";
}

function wirePurposeChips() {
  const wrap = el("prop-purpose-chips");
  if (!wrap) return;
  wrap.querySelectorAll(".purpose-chip").forEach((btn) => {
    btn.addEventListener("click", () => {
      wrap.querySelectorAll(".purpose-chip").forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      const hid = el("prop-purpose");
      if (hid) hid.value = btn.dataset.purpose || "Payout";
    });
  });
}

async function proposalCreate() {
  let account = selectedCouncilId != null ? Number(selectedCouncilId) : NaN;
  const acctEl = el("prop-account-id");
  if (Number.isNaN(account) && acctEl) account = parseInt(acctEl.value, 10);
  if (Number.isNaN(account) && storyAccountId != null) account = Number(storyAccountId);
  if (Number.isNaN(account)) {
    showErrorOutcome("prop-outcome", "Form a council first (Beat 2).");
    return;
  }
  const purpose = getSelectedPurpose();
  const descEl = el("prop-description");
  const description = descEl ? descEl.value.trim() : "";
  if (!description) {
    showErrorOutcome("prop-outcome", "Add a short description.");
    return;
  }
  // Keep mock chain fields as STORY constants; purpose/description are UI/demo context.
  const target = STORY.propTarget;
  const functionName = purposeToFunction(purpose) || STORY.propFunction;
  const args_hex = STORY.propArgsHex;
  const deadline = STORY.propDeadline;
  setOutcome("prop-outcome", `<span class="outcome-meta">Proposing…</span>`, true);
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
    const id = out.allocated_id_hint;
    storyProposalId = id;
    pushDemoProposal({
      id,
      accountId: account,
      purpose,
      description,
      status: "open",
      approvals: 0,
      finalized: false,
      intentConfirmed: false,
      fingerprintHtml: null,
    });
    updateStatusStrip({ approvals: 0 });
    intentConfirmed = false;
    syncPropApproveGate();
    updateApproveSection();
    if (submitOk(submit)) {
      setOutcome(
        "prop-outcome",
        `${outcomeChip(true)}<span class="outcome-title">Proposal #${id}</span>` +
          `<span class="outcome-meta">${escapeHtml(purpose)} · ${escapeHtml(description)}</span>`,
        true
      );
    } else {
      showSubmitOutcome("prop-outcome", { ...submit, note: `proposal id ${id}` });
    }
  } catch (e) {
    showErrorOutcome("prop-outcome", e.message);
  }
}

async function confirmIntent() {
  const id = selectedProposalId != null ? selectedProposalId : (el("prop-id") && el("prop-id").value);
  if (id == null || id === "") {
    showErrorOutcome("finalize-outcome", "Select a proposal first.");
    return;
  }
  try {
    // Still hit preview so approve has a confirmed digest under the hood.
    const out = await api(`/api/proposal/${id}/preview`);
    const html =
      `mnemonic: ${escapeHtml(out.digest_mnemonic)} · ` +
      `digest: ${escapeHtml(String(out.digest_hex).slice(0, 16))}…`;
    const p = demoProposals.find((x) => x.id === id || String(x.id) === String(id));
    if (p) {
      p.fingerprintHtml = html;
      p.intentConfirmed = true;
    }
    intentConfirmed = true;
    syncPropApproveGate();
    renderProposalsList();
    refreshIntentCard();
    updateApproveSection();
    showToast("Intent confirmed — Approve unlocked");
  } catch (e) {
    intentConfirmed = false;
    syncPropApproveGate();
    updateApproveSection();
    showErrorOutcome("finalize-outcome", e.message);
  }
}

async function proposalApprove() {
  const idEl = el("prop-id");
  const signerEl = el("prop-signer");
  const id = idEl ? idEl.value : (selectedProposalId != null ? String(selectedProposalId) : "");
  const signer = signerEl ? signerEl.value.trim() : (activePropSigner || "");
  if (!id) {
    showErrorOutcome("finalize-outcome", "Select a proposal first.");
    return;
  }
  if (!signer) {
    showErrorOutcome("finalize-outcome", "Select a signer tile.");
    return;
  }
  if (!intentConfirmed) {
    showErrorOutcome("finalize-outcome", "Confirm the intent first.");
    return;
  }
  setOutcome("finalize-outcome", `<span class="outcome-meta">Approving as ${escapeHtml(signer)}…</span>`, true);
  try {
    const out = await api(`/api/proposal/${id}/approve`, {
      method: "POST",
      body: JSON.stringify({ signer, confirm: true }),
    });
    const submit = out.submit || out;
    let approvals = null;
    try {
      const st = await api(`/api/proposal/${id}`);
      if (st && st.approvals_len != null) approvals = st.approvals_len;
    } catch (_) {}
    if (approvals == null) {
      approvals = (statusApprovals != null ? statusApprovals : 0) + 1;
    }
    updateStatusStrip({ approvals });
    const p = demoProposals.find((x) => String(x.id) === String(id));
    if (p) {
      p.approvals = approvals;
      updateStatusCard(p);
    }
    showSubmitOutcome("finalize-outcome", {
      ...submit,
      note: `approved as ${signer}`,
    });
  } catch (e) {
    showErrorOutcome("finalize-outcome", e.message);
  }
}

async function proposalFinalize() {
  const idEl = el("prop-id");
  const id = idEl ? idEl.value : (selectedProposalId != null ? String(selectedProposalId) : "");
  if (!id) {
    showErrorOutcome("finalize-outcome", "Select a proposal first.");
    return;
  }
  setOutcome("finalize-outcome", `<span class="outcome-meta">Finalizing…</span>`, true);
  try {
    const out = await api(`/api/proposal/${id}/finalize`, { method: "POST", body: "{}" });
    const p = demoProposals.find((x) => String(x.id) === String(id));
    if (p && submitOk(out)) {
      p.status = "finalized";
      p.finalized = true;
      updateStatusCard(p);
    }
    if (submitOk(out)) {
      setOutcome(
        "finalize-outcome",
        `${outcomeChip(true)}<span class="outcome-title">Proposal #${id} finalized</span>`,
        true
      );
    } else {
      showSubmitOutcome("finalize-outcome", out);
    }
  } catch (e) {
    showErrorOutcome("finalize-outcome", e.message);
  }
}

function prepareBeatEntry(index) {
  const beat = BEATS[index];
  if (!beat) return;
  if (beat.beat === 4) {
    if (storyAccountId !== null) applyAccountIds(storyAccountId);
    updateHeaderCouncil();
  }
  if (beat.beat === 5) {
    // Prefer latest proposal; default approve-as to “you”, else bob.
    if (demoProposals.length) {
      const latest = [...demoProposals].sort((a, b) => Number(b.id) - Number(a.id))[0];
      selectProposal(latest.id);
    } else if (storyProposalId != null) {
      selectProposal(storyProposalId);
    }
    if (youIdentity) {
      activePropSigner = youIdentity;
    } else {
      activePropSigner = "bob";
    }
    const signer = el("prop-signer");
    if (signer) signer.value = activePropSigner;
    // Restore confirm state from the selected proposal (don't wipe it).
    const p = demoProposals.find((x) => x.id === selectedProposalId || String(x.id) === String(selectedProposalId));
    intentConfirmed = !!(p && p.intentConfirmed);
    syncPropApproveGate();
    renderPropSignerList(cachedIdentities);
    renderProposalsList();
    refreshIntentCard();
    updateApproveSection();
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

function goToBeatIndex(target) {
  const idx = Math.max(0, Math.min(BEATS.length - 1, target));
  const beat = BEATS[idx];
  if (!beat) return;
  const dot = document.querySelector(`.beat-dots .tab[data-beat="${idx + 1}"]`);
  if (dot) {
    document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
      const on = btn === dot;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-selected", on ? "true" : "false");
    });
  }
  prepareBeatEntry(idx);
  activateTab(beat.tab, { fromBeatNav: true });
}

function goBeat(delta) {
  const active = document.querySelector(".beat-dots .tab.active");
  let i = 0;
  if (active && active.dataset.beat) {
    const b = parseInt(active.dataset.beat, 10);
    if (!Number.isNaN(b)) i = b - 1;
  }
  goToBeatIndex(i + delta);
}

function activateTab(name, opts = {}) {
  document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
    if (opts.fromBeatNav) return;
    const on = btn.dataset.tab === name;
    btn.classList.toggle("active", on);
    btn.setAttribute("aria-selected", on ? "true" : "false");
  });
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
  if (name === "proposal") updateHeaderCouncil();
  if (name === "finalize") {
    renderProposalsList();
    renderPropSignerList(cachedIdentities);
    refreshIntentCard();
    updateApproveSection();
    const p = selectedProposalId != null
      ? demoProposals.find((x) => String(x.id) === String(selectedProposalId))
      : null;
    updateStatusCard(p || null);
  } else {
    // Open/Finalized chip is finalize-only.
    updateHeaderApprovals(null);
  }
  if (name === "council") renderCouncilPickList(cachedIdentities);
  if (name === "cast") renderCastList(cachedIdentities);
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

function wireBeatTabs() {
  document.querySelectorAll(".beat-dots .tab").forEach((btn) => {
    btn.addEventListener("click", () => {
      const opts = {};
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

function wireFingerprintTip() {
  const btn = el("btn-fp-tip");
  const tip = el("fp-tip");
  if (!btn || !tip) return;
  btn.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const open = tip.hidden;
    tip.hidden = !open;
    btn.setAttribute("aria-expanded", open ? "true" : "false");
  });
  document.addEventListener("click", (ev) => {
    if (tip.hidden) return;
    if (btn.contains(ev.target) || tip.contains(ev.target)) return;
    tip.hidden = true;
    btn.setAttribute("aria-expanded", "false");
  });
}

function wireLtrInputs() {
  document.querySelectorAll("input, textarea").forEach((node) => {
    node.setAttribute("dir", "ltr");
    const force = () => {
      node.setAttribute("dir", "ltr");
      if (node.style) {
        node.style.direction = "ltr";
        node.style.unicodeBidi = "isolate";
        node.style.textAlign = "left";
      }
    };
    node.addEventListener("focus", force);
    node.addEventListener("input", force);
    node.addEventListener("keyup", force);
  });
}

function wireHeaderCouncil() {
  on("header-council", "click", () => {
    // Beat 3 · Look up
    goToBeatIndex(2);
    if (selectedCouncilId != null) fetchCouncilOutcome(selectedCouncilId);
  });
}

// Init
prefillStoryFields();
wireIdentityPopover();
wirePurposeChips();
wireBeatTabs();
wireFingerprintTip();
wireLtrInputs();
wireHeaderCouncil();
on("btn-beat-prev", "click", () => goBeat(-1));
on("btn-beat-next", "click", () => goBeat(1));
on("btn-close-council-detail", "click", () => closeCouncilDetail());

(async function boot() {
  if (USE_FRONTEND_MOCK) {
    try {
      if (window.MockLab && window.MockLab.selfTest) window.MockLab.selfTest();
      console.info("Lab: frontend MockLedger active (static demo)");
    } catch (e) {
      console.error("MockLedger selfTest failed", e);
    }
  }
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
  for (const name of STORY.cast) councilSelected.add(name);
  renderCouncilPickList(cachedIdentities);
  updateStatusStrip({ threshold: STORY.threshold, approvals: 0 });
  updateYouChip();
  updateHeaderCouncil();

  // Seed header icon element even when empty (SVG ready).
  const icon = el("header-council-icon");
  if (icon && !icon.innerHTML) icon.innerHTML = COUNCIL_ICON_SVG;

  const initialTab = tabFromUrl();
  if (initialTab) activateTab(initialTab);
})();

// Expose handlers used by inline onclick attributes.
window.submitCreateAccount = submitCreateAccount;
window.proposalCreate = proposalCreate;
window.proposalApprove = proposalApprove;
window.proposalFinalize = proposalFinalize;
window.confirmIntent = confirmIntent;
