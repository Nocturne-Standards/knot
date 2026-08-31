# Knot demo (static, backend-free)

Hosted mock walkthrough for **Knot** — M-of-N BLS multisig councils.
`demo/dist` is built from [`../crates/knot-tool/static`](../crates/knot-tool/static)
(the same HTML/CSS/JS the Rust `knot-tool` binary embeds and serves locally),
with `window.KNOT_FRONTEND_MOCK = true` injected so the page runs entirely on
[`mock-ledger.js`](../crates/knot-tool/static/mock-ledger.js)'s pure client-side
mock ledger. No server, no Worker, no Durable Object, nothing persisted —
state lives only in the browser tab and resets on reload.

## Status

**Wiring complete — live Pages deploy pending.** Target:
`https://knot.nocturne-standards.org` (attach custom domain in the
Cloudflare dashboard after the first upload — see checklist below).

**Do not flip `products.knot.site` in `nocturne-standards-site/published.json`
until `knot.nocturne-standards.org` is confirmed live** (`curl -I` 2xx/3xx).
Per the release runbook, marketing only points at a product once its
subdomain resolves.

## Run

```bash
cd demo
npm run serve     # builds dist/ then serves http://127.0.0.1:8892/
```

If `npm run serve` hangs, `python3 -m http.server 8892 --directory dist`
(after `npm run build`) is the fallback.

Mock only — no chain writes, no real keys. Identities are fixture BLS-shaped
keys, signatures/digests are deterministic UI stand-ins (see comments at the
top of `mock-ledger.js`).

## Build

```bash
cd demo
npm run build     # ./build.sh: copies crates/knot-tool/static/ into dist/,
                   # injects window.KNOT_FRONTEND_MOCK = true into dist/index.html
```

Single source of truth stays `crates/knot-tool/static/`. Any fix to
`app.js`/`style.css`/`mock-ledger.js` for the Rust tool ships to this demo
automatically on the next build — nothing here is hand-duplicated.

## Deploy

Same credential pattern as `nocturne-standards-site` / `nocturne-docs` /
`ambit/demo` (macOS keychain service `cloudflare-nocturne-pages`, account
`97a7e08f5732716f69a9165cdc1d7a38`).

```bash
cd demo
npm install
npm run deploy    # builds dist/, then wrangler pages deploy dist --project-name=knot-demo
```

`dist/` is generated (gitignored) and only ever contains copied static
assets — `wrangler.toml`, `package.json`, `build.sh`, and this README live
one level up in `demo/` and are never part of the upload. `.assetsignore`
is kept for parity with sibling nocturne Pages projects in case
`pages_build_output_dir` ever changes.

### Operator checklist (first ship)

1. **Store the Pages API token** (one-time; token needs
   `Account | Cloudflare Pages | Edit` on account
   `97a7e08f5732716f69a9165cdc1d7a38`), if not already stored for a sibling
   project on this machine:
   ```bash
   printf "Cloudflare API token: "
   read -s CF_TOKEN
   echo
   security add-generic-password -a "$USER" -s cloudflare-nocturne-pages -w "$CF_TOKEN"
   unset CF_TOKEN
   ```
2. `cd demo && npm install && npm run deploy` — creates/updates Cloudflare
   Pages project `knot-demo`.
3. Cloudflare dashboard → Workers & Pages → `knot-demo` → **Custom domains**
   → add `knot.nocturne-standards.org` (same zone as other nocturne
   subdomains; Pages provisions DNS + TLS automatically).
4. Verify `https://knot.nocturne-standards.org` — walk all 5 beats (Cast →
   Council → Look up → Propose → Finalize) plus Quorum verify and Rotate
   council.
5. Only then: flip `products.knot.site` in
   `nocturne-standards-site/published.json` and redeploy marketing, per the
   release runbook.
