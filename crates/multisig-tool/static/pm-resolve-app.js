// Standalone PM council-resolve UI. Token from pm-resolve.html; signing is server-side.

const TOKEN = window.MULTISIG_TOOL_TOKEN;

async function api(path, opts = {}) {
  const headers = {
    "X-Multisig-Tool-Token": TOKEN,
    ...(opts.body ? { "Content-Type": "application/json" } : {}),
    ...(opts.headers || {}),
  };
  const res = await fetch(path, { ...opts, headers });
  const text = await res.text();
  let data;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = text;
  }
  if (!res.ok) {
    const msg = typeof data === "string" ? data : data?.error || data?.message || text || res.statusText;
    throw new Error(msg);
  }
  return data;
}

function log(msg, ok = null) {
  const el = document.getElementById("log");
  el.textContent = typeof msg === "string" ? msg : JSON.stringify(msg, null, 2);
  if (ok === true) el.style.borderColor = "#3d9a6a";
  else if (ok === false) el.style.borderColor = "#a05050";
  else el.style.borderColor = "";
}

function qs(name) {
  return new URLSearchParams(window.location.search).get(name);
}

function applyPrefills() {
  const map = {
    market: "market",
    outcome: "outcome",
    pm: "pm",
    account: "account",
    threshold: "threshold",
    summary: "summary",
    id: "blob-id",
  };
  for (const [q, id] of Object.entries(map)) {
    const v = qs(q);
    if (v != null && v !== "") {
      const el = document.getElementById(id);
      if (el) el.value = v;
    }
  }
}

async function refreshIdentities() {
  const list = await api("/api/identities");
  const sel = document.getElementById("signer");
  const prev = sel.value;
  sel.innerHTML = "";
  for (const id of list) {
    if (id.pk_only) continue;
    const opt = document.createElement("option");
    opt.value = id.name;
    opt.textContent = id.name;
    sel.appendChild(opt);
  }
  if (prev && [...sel.options].some((o) => o.value === prev)) sel.value = prev;
  else if (qs("signer")) sel.value = qs("signer");
}

async function refreshSetup() {
  const line = document.getElementById("setup-line");
  try {
    const st = await api("/api/setup/status");
    const ids = await api("/api/identities");
    const names = ids.filter((i) => !i.pk_only).map((i) => i.name);
    const coll = st.collector_configured
      ? `<span class="collector-ok">collector: ${st.collector_url || "configured"}</span>`
      : `<span class="collector-bad">collector not configured - set MULTISIG_COLLECTOR_URL before starting this UI (init+push needs it)</span>`;
    line.innerHTML = `Identities: ${names.length ? names.join(", ") : "(none)"} · ${coll}`;
  } catch (e) {
    line.textContent = e.message;
  }
}

async function refreshChainData() {
  const hint = document.getElementById("chain-hint");
  hint.textContent = "Loading from testnet…";
  try {
    const [dep, accounts, markets] = await Promise.all([
      api("/api/deployments/pm"),
      api("/api/registry/accounts?limit=64"),
      api("/api/pm/markets?limit=50"),
    ]);
    const pmEl = document.getElementById("pm");
    if (!pmEl.value.trim()) {
      pmEl.value = dep.pm_contract_id;
    }

    const council = document.getElementById("council-pick");
    const prevCouncil = council.value;
    council.innerHTML = "";
    const blankC = document.createElement("option");
    blankC.value = "";
    blankC.textContent = accounts.length ? "- pick council -" : "- no accounts -";
    council.appendChild(blankC);
    for (const a of accounts) {
      const opt = document.createElement("option");
      opt.value = String(a.id);
      opt.textContent = a.label;
      opt.dataset.threshold = String(a.threshold);
      council.appendChild(opt);
    }
    if (prevCouncil && [...council.options].some((o) => o.value === prevCouncil)) {
      council.value = prevCouncil;
    }

    const marketPick = document.getElementById("market-pick");
    const prevMarket = marketPick.value;
    marketPick.innerHTML = "";
    const blankM = document.createElement("option");
    blankM.value = "";
    blankM.textContent = markets.length ? "- pick market -" : "- no markets -";
    marketPick.appendChild(blankM);
    const sorted = [...markets].sort((a, b) => Number(b.under_review) - Number(a.under_review) || a.id - b.id);
    for (const m of sorted) {
      const opt = document.createElement("option");
      opt.value = String(m.id);
      opt.textContent = (m.under_review ? "★ " : "") + m.label;
      marketPick.appendChild(opt);
    }
    if (prevMarket && [...marketPick.options].some((o) => o.value === prevMarket)) {
      marketPick.value = prevMarket;
    }

    const under = markets.filter((m) => m.under_review).length;
    hint.textContent = `PM ${dep.pm_contract_id.slice(0, 10)}… · ${accounts.length} account(s) · ${markets.length} market(s) (${under} under review)`;
  } catch (e) {
    hint.textContent = e.message;
    throw e;
  }
}

function onCouncilPick() {
  const sel = document.getElementById("council-pick");
  const opt = sel.selectedOptions[0];
  if (!opt || !opt.value) return;
  document.getElementById("account").value = opt.value;
  if (opt.dataset.threshold) {
    document.getElementById("threshold").value = opt.dataset.threshold;
  }
}

function onMarketPick() {
  const sel = document.getElementById("market-pick");
  const opt = sel.selectedOptions[0];
  if (!opt || !opt.value) return;
  document.getElementById("market").value = opt.value;
  const summary = document.getElementById("summary");
  if (!summary.value.trim()) {
    summary.value = `resolve market ${opt.value}`;
  }
}

async function createIdentity() {
  const name = document.getElementById("id-name").value.trim();
  if (!name) {
    log("enter a name", false);
    return;
  }
  try {
    await api("/api/identities", { method: "POST", body: JSON.stringify({ name }) });
    document.getElementById("id-name").value = "";
    await refreshIdentities();
    await refreshSetup();
    document.getElementById("signer").value = name;
    log(`created identity ${name}`, true);
  } catch (e) {
    log(e.message, false);
  }
}

async function initPush() {
  const body = {
    market_id: Number(document.getElementById("market").value),
    winning_outcome: Number(document.getElementById("outcome").value),
    pm_contract_id: document.getElementById("pm").value.trim(),
    registry_account_id: Number(document.getElementById("account").value),
    threshold: Number(document.getElementById("threshold").value),
    summary: document.getElementById("summary").value.trim() || null,
    push: true,
  };
  log("init + push…");
  try {
    const out = await api("/api/pm-resolve/init", { method: "POST", body: JSON.stringify(body) });
    document.getElementById("blob-id").value = out.id;
    log(
      `init ok\nid: ${out.id}\ndigest: ${out.signed_digest}\npushed: ${out.pushed}\nshare this id with co-signers`,
      true
    );
    await listBlobs();
    await status();
  } catch (e) {
    log(e.message, false);
  }
}

async function listBlobs() {
  const host = document.getElementById("blob-list");
  try {
    const items = await api("/api/pm-resolve/list");
    if (!items.length) {
      host.innerHTML = "<p class=\"hint\">No pm_council_resolve blobs on the collector.</p>";
      return;
    }
    host.innerHTML = "";
    for (const it of items) {
      const row = document.createElement("button");
      row.type = "button";
      row.innerHTML = `<strong>${it.id.slice(0, 18)}…</strong> <span class="status-pill">${it.partials_count}/${it.threshold}</span>`;
      row.addEventListener("click", () => {
        document.getElementById("blob-id").value = it.id;
        status();
      });
      host.appendChild(row);
    }
  } catch (e) {
    host.innerHTML = `<p class="hint">${e.message}</p>`;
  }
}

async function status() {
  const id = document.getElementById("blob-id").value.trim();
  if (!id) {
    log("set blob id first", false);
    return;
  }
  log("status…");
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}`);
    document.getElementById("market").value = out.market_id;
    document.getElementById("outcome").value = out.winning_outcome;
    document.getElementById("pm").value = out.pm_contract_id;
    document.getElementById("account").value = out.registry_account_id;
    document.getElementById("threshold").value = out.threshold;
    if (out.human_summary) document.getElementById("summary").value = out.human_summary;
    const ready = document.getElementById("ready-line");
    ready.innerHTML = out.ready
      ? `<span class="status-pill ready">ready to submit · ${out.partials_count}/${out.threshold}</span>`
      : `<span class="status-pill">${out.partials_count}/${out.threshold} partials</span>`;
    let text =
      `market=${out.market_id} outcome=${out.winning_outcome} (${out.winning_outcome === 0 ? "YES" : "NO"})\n` +
      `partials=${out.partials_count}/${out.threshold} ready=${out.ready}\n` +
      `digest=${out.signed_digest}\npm=${out.pm_contract_id}\naccount=${out.registry_account_id}`;
    if (out.registry_warn) text += `\n${out.registry_warn}`;
    log(text, true);
  } catch (e) {
    log(e.message, false);
  }
}

async function preview() {
  const id = document.getElementById("blob-id").value.trim();
  if (!id) {
    log("set blob id first", false);
    return;
  }
  log("preview…");
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}/preview`);
    const box = document.getElementById("preview-box");
    box.style.display = "block";
    box.textContent =
      `digest: ${out.digest_hex}\n` +
      `mnemonic: ${out.digest_mnemonic}\n` +
      `safety: ${out.digest_safety_number}\n` +
      `market=${out.market_id} outcome=${out.winning_outcome}\n` +
      `pm=${out.pm_contract_id}\naccount=${out.registry_account_id} threshold=${out.threshold}`;
    document.getElementById("market").value = out.market_id;
    document.getElementById("outcome").value = out.winning_outcome;
    document.getElementById("pm").value = out.pm_contract_id;
    document.getElementById("account").value = out.registry_account_id;
    document.getElementById("threshold").value = out.threshold;
    document.getElementById("confirm").checked = false;
    document.getElementById("btn-sign").disabled = true;
    log("preview ok - compare mnemonic, then check confirm + sign", true);
  } catch (e) {
    log(e.message, false);
  }
}

async function sign() {
  const id = document.getElementById("blob-id").value.trim();
  const signer = document.getElementById("signer").value.trim();
  if (!id || !signer) {
    log("need blob id and signer", false);
    return;
  }
  if (!document.getElementById("confirm").checked) {
    log("check the confirm box after preview", false);
    return;
  }
  log(`signing as ${signer}…`);
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}/sign`, {
      method: "POST",
      body: JSON.stringify({ signer, confirm: true }),
    });
    log(
      `signed\npartials=${out.partials_count}/${out.threshold} ready=${out.ready}\npk=${out.signer_pk}\ndigest=${out.digest_hex}`,
      true
    );
    await status();
    await listBlobs();
  } catch (e) {
    log(e.message, false);
  }
}

async function submit() {
  const id = document.getElementById("blob-id").value.trim();
  if (!id) {
    log("set blob id first", false);
    return;
  }
  log("submitting resolve (rusk-wallet)…");
  try {
    const out = await api(`/api/pm-resolve/${encodeURIComponent(id)}/submit`, {
      method: "POST",
      body: "{}",
    });
    log(`submitted\n${JSON.stringify(out, null, 2)}`, true);
  } catch (e) {
    log(e.message, false);
  }
}

document.getElementById("btn-new-id").addEventListener("click", createIdentity);
document.getElementById("btn-refresh-ids").addEventListener("click", () =>
  refreshIdentities().then(refreshSetup).catch((e) => log(e.message, false))
);
document.getElementById("btn-refresh-chain").addEventListener("click", () =>
  refreshChainData().catch((e) => log(e.message, false))
);
document.getElementById("council-pick").addEventListener("change", onCouncilPick);
document.getElementById("market-pick").addEventListener("change", onMarketPick);
document.getElementById("btn-init").addEventListener("click", initPush);
document.getElementById("btn-preview").addEventListener("click", preview);
document.getElementById("btn-sign").addEventListener("click", sign);
document.getElementById("btn-status").addEventListener("click", status);
document.getElementById("btn-list").addEventListener("click", listBlobs);
document.getElementById("btn-submit").addEventListener("click", submit);
document.getElementById("confirm").addEventListener("change", (e) => {
  document.getElementById("btn-sign").disabled = !e.target.checked;
});

applyPrefills();
refreshIdentities()
  .then(refreshSetup)
  .then(() => refreshChainData().catch((e) => log(e.message, false)))
  .then(() => {
    if (document.getElementById("blob-id").value.trim()) status();
    else listBlobs().catch(() => {});
  })
  .catch((e) => log(e.message, false));
