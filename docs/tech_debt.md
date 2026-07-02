# Tech Debt

To me: Known gaps, ordered by risk. All small fixes.

## `BASE_URL` not set in production

`base_url` (`src/app.rs`) trusts `x-forwarded-proto` + `Host` when the `BASE_URL` env var is unset, and those feed the canonical URL, OG tags, sitemap, and Atom feed. A request with a forged Host header gets a response whose canonical/OG URLs point at the attacker's host — mostly harmless for direct visitors, but wrong canonical data could reach a crawler via a cached copy.

- Fix: set `BASE_URL=https://light-vn-works.onrender.com` on Render. The header path stays as a dev-only fallback.

## Inline `TREE_DATA` carries dead weight (deferred)

The homepage JS reads exactly four meta fields from the embedded tree JSON — `tags`, `creator`, `tagline`, `released` (`public/home.js`, sole consumer). Measured on the live index: the tree JSON is ~171 KB, and the five client-unused fields (`link_label` 6.7, `link_url` 13.9, `extra_links` 24.2, `date_added` 4.3, `thumbnail_index` 5.5 KB) total ~55 KB — a third of the payload, roughly 16 KB per page load after gzip.

- Deferred: `/api/tree` serves the same pre-serialized string and needs the full meta, so a trim requires a second, client-only serialization (two tree strings in memory) rather than a one-line `skip_serializing_if`.
- If revisited, pair it with the "split TREE_DATA around 1000+ entries" item in `performance_todo.md` — one restructuring instead of two.

## Housekeeping

- **Strict CSP (`script-src` without `'unsafe-inline'`)**: the shipped CSP blocks external script loads but still allows inline scripts, so an injected inline `<script>` would run. Closing it needs three refactors: the homepage data blob as `<script type="application/json">` + `JSON.parse` (non-executing types are exempt from `script-src`), the lang-toggle/copyLink inline scripts moved to a shared file, and the `onerror`/`onclick` handlers in generated HTML replaced with `addEventListener`.
- **CSP `img-src` pins a GitHub S3 bucket**: anonymous user-attachment fetches redirect to `github-production-user-asset-6210df.s3.amazonaws.com`, an implementation detail GitHub could rotate — unproxied hero/gallery images would break if it does. Rotation is now alarmed rather than silent: blocked loads POST a CSP violation to `/csp-report`, logged at `warn` with the `blocked-uri`. If it bites before the fix below, widen that one entry to `https://*.s3.amazonaws.com` as a stopgap — coarser, but low risk for `img-src`.
  - Real fix: route hero/gallery/editor images through a proxy so only same-origin URLs reach the page. `img-src` then collapses to `'self'` + goatcounter, and the S3 pin goes away. This does not reuse the current cache: `thumb_cache`/`thumb_originals` hold one resized thumbnail per work (~261 × 2 × ~15 KB ≈ 7 MB); hero/gallery images are served full-size straight from GitHub and aren't registered at all.
  - Full-size is a different scale — ~745 images, hundreds of KB each. Three shapes, none free:
    - Full-size RAM cache: simplest, but ~100–300 MB against Render free tier's 512 MB. Likely infeasible.
    - Streaming passthrough (proxy bytes per request, no server cache, long browser `Cache-Control`): no RAM growth, but cold fetches route through Render instead of GitHub's CDN — spends Render bandwidth, adds a hop of latency. The likely first cut.
    - New large-WebP variant cached like thumbnails: bounded memory, at the quality loss `design_decisions.md` currently rejects for detail images.
- **Off-site images caught at PR time, not exhaustively**: `first_offsite_image` (`src/lib.rs`) lints every works body in CI so a tracking-pixel `<img>` fails the build (covers `src`/`srcset`/markdown images). It's a string scan, so a browser-parsed sink it doesn't model could still slip through — the CSP `img-src` allowlist stays the runtime backstop.
