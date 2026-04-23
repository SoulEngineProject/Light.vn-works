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

## Restart on content change

All markdown files are parsed once at startup into an in-memory `HashMap<canonical_path, ParsedGame>` (`src/app.rs::build_games_index`). This is the sole source of truth; the tree JSON, creator index, and game-page rendering all derive from it. Editing a file on disk does **not** live-update — the server must be restarted.

- **Why**: perf (no per-request parse or disk I/O) and simplicity (single walk of `works/`, one source of truth). The tree and creator index were already built at startup, so restart-on-change was already the de facto contract for most content changes; this makes it explicit and consistent.
- **Considered and deferred**: `notify`-based file watching. Cross-platform file watching is a known source of subtle bugs (event coalescing, editor-atomic-write patterns differ per OS and per editor), and the win over "restart the server" is small for a content site deployed via push. Revisit if the dev loop starts to chafe.
