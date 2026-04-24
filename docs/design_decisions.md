# Design Decisions

## GitHub for image storage

- **No extra cost**: allows us to save images through github, instead of using a CDN
- **One source of data**: keeps everything (data) to just one repo, instead of spread out on multiple infra
- **Easy contribution**: just make a PR

## `Cache-Control: no-cache` over long `max-age`

Static assets send `Cache-Control: no-cache` rather than a long `max-age, immutable`. "no-cache" means "cache, but revalidate every time" — combined with the `Last-Modified` header `ServeDir` emits, browsers send conditional GETs and receive `304 Not Modified` (no body) for unchanged files.

- **Why not long `max-age`**: without cache-busted filenames (build hashes or version query strings), aggressive caching would ship stale CSS/JS to users after a deploy. We have no build tooling today.
- **Tradeoff accepted**: every asset request costs one conditional-GET roundtrip. Body only transfers when the file actually changes. Upgrade path (long `max-age, immutable` with versioned filenames) is noted in `performance_todo.md`.

## Server-embed tree data in home HTML

The home page inlines `TREE_DATA`, `LANG_DATA`, and `TAG_COLOURS` as JSON in a `<script>` tag rather than fetching them client-side.

- **Why**: eliminates the initial API roundtrip and loading state. Page renders immediately with full data.
- **Tradeoff accepted**: HTML response size grows with catalog. Fine at 241 entries; revisit around 1000+ (noted in `performance_todo.md`).
- **Compression offsets most of the cost**: `CompressionLayer` shrinks the embedded JSON ~70% on the wire.

## Thumbnail proxy with lazy populate

Homepage thumbnails (ribbon + card) are served via `/thumb/:uuid/:size` — a Rust-side proxy that fetches the original GitHub user-attachment, resizes to a display-appropriate dimension, re-encodes as WebP q=80 lossy, and caches the bytes in memory. Normal cards get 600×400, ribbon 240×140. Composites (wide-aspect strips) get larger targets (1600×400 / 900×400) and never upscale — the homepage renders them with CSS `background-size: 340%` zoom/crop, which needs enough source resolution to survive retina + zoom without upsampling blur.

- **Lazy populate + background warmup at startup**: Server starts listening immediately (no blocking pre-fetch). A background task kicked off from `build_app` sweeps every (UUID, size) through `populate_thumbnail`. Visitors who race with warmup get 302 fallback (same as pure lazy). Within ~60s of startup, cache is fully warm. Races between warmer and user requests are harmless thanks to in-flight debouncing.
  - **Easy to revert**: the lazy path (`serve_thumb` cache-miss → `populate_thumbnail`) and the warmup batch-trigger are decoupled. To switch back to lazy-only, delete the one `tokio::spawn(warm_all_thumbnails(...))` line in `build_app`. Other hybrids (warm only ribbons, warm after a delay, warm via an endpoint) are equally small changes — all just "who calls `populate_thumbnail` and when."
- **WebP q=80 lossy via the `webp` crate** (libwebp C bindings): ~20–30% smaller than JPEG q=80 at equivalent visual quality, and preserves alpha channels so thumbnails with transparency don't flatten to black. Universally supported in modern browsers.
- **UUID as cache key**: GitHub's user-attachment URLs already contain stable unique IDs. We reuse them — no UUID generation, natural dedup across games sharing the same image, trivially traceable proxy URL.
- **Whitelist**: `/thumb/:uuid/:size` 404s for any UUID not in the index, so the route can't be used as a general GitHub proxy.
- **Scope**: thumbnails only. Hero, gallery, and editor-mockup images fetch full-size directly from GitHub — resizing them would lose the detail users care about.
- **Tradeoff accepted**: CPU/RAM on the server vs. bandwidth + roundtrip latency for visitors. At 241 games × 2 sizes × ~15KB = ~7MB RAM, well worth it.

## Restart on content change

All markdown files are parsed once at startup into an in-memory `HashMap<canonical_path, ParsedGame>` (`src/app.rs::build_games_index`). This is the sole source of truth; the tree JSON, creator index, and game-page rendering all derive from it. Editing a file on disk does **not** live-update — the server must be restarted.

- **Why**: perf (no per-request parse or disk I/O) and simplicity (single walk of `works/`, one source of truth). The tree and creator index were already built at startup, so restart-on-change was already the de facto contract for most content changes; this makes it explicit and consistent.
- **Considered and deferred**: `notify`-based file watching. Cross-platform file watching is a known source of subtle bugs (event coalescing, editor-atomic-write patterns differ per OS and per editor), and the win over "restart the server" is small for a content site deployed via push. Revisit if the dev loop starts to chafe.
