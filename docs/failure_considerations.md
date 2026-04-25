# Failure Considerations

Self Note: Think of this like an interview question. "What can fail?" "Any single points of failures?"
Non-obvious graceful-degradation choices. Don't "fix" these into stricter behavior without understanding the reasoning.

## Ribbon & image loading

- **Ribbon reveals on first image load, with 6s wall-clock fallback** (`public/home.js::buildRibbon`).
  - Why: pairs with `fetchpriority="high"` on the visible-at-rest imgs to reveal as soon as content is ready (typically <200ms). The 6s fallback covers networks where every fetch hangs — without it, the ribbon could stay invisible forever if all images permanently fail to load.

- **Broken images hidden via CSS, not replaced** (`public/home.css`: `img[data-error] { display: none }`).
  - Why: collapsing the space degrades more gracefully than showing a torn-page icon.

- **`retryImage` backoff: 1s then 3s, max 2 retries** (`public/home.js::retryImage`).
  - Why: GitHub user-attachments occasionally flakes; most transient failures resolve within ~5s.

- **Dark gradient background under thumbnail containers** (`public/style.css`, combined rule targeting `.card-thumb`, `.ribbon-track img`, `.more-creator-thumb`).
  - Why: browsers render empty `<img>` elements as native-default white before content loads. On the dark UI, that flashes a visible white frame during lazy-load or year-section expansion. The gradient fills the pre-load state so the transition is "dark → image" instead of "white → image". Don't remove thinking it's dead code — the img covers it once loaded.

## Accessibility

- **`alt=""` on every `<img>` where adjacent text is the accessible label** (homepage cards + ribbon in `public/home.js`; hero, gallery, editor preview, more-from thumbs in `src/app.rs`).
  - Why: the game title lives in `.card-title` / `<h1>` / `.more-creator-title` next to each image. Duplicating it in `alt` adds nothing for screen readers and flashes as an ugly overlay during slow loads. Ribbon thumbs additionally set `aria-label` on the wrapping `<a>` since there's no visible text label in the marquee. Don't "fix" the empty alts — they're deliberate.

## Server-side rendering

- **Frontmatter YAML parse failure → default `GameMeta`** (`src/lib.rs::parse_frontmatter`).
  - Why: malformed frontmatter shouldn't error the page, just omit metadata.

- **Invalid `tags.yaml` → empty map** (`src/lib.rs::load_tag_config`).
  - Why: silent fallback rather than panicking the server on startup. Tags render as plain, unstyled badges.

- **Invalid `aliases.yaml` → empty map** (`src/lib.rs::load_aliases`).
  - Why: same philosophy — "More from" sections just won't cross-link aliases.

- **Composite detection requires `width` and `height` attrs on the `<img>`** (`src/lib.rs::ImageInfo::is_composite`).
  - Why: without dimensions we can't judge aspect ratio. Fall back to plain `<img>` rendering, not an error.

- **Path traversal guard in `render_markdown`** (`src/app.rs`, rejects `..`, `/`, oversized year/title).
  - Why: defense in depth. `ServeDir` and `read_to_string` might not block every variant equivalently.

- **Thumbnail chain never fails** (`src/app.rs::build_games_index`: `thumbnail_index` → first image → sparkle placeholder at render time).
  - Why: an out-of-bounds `thumbnail_index` falls back to the first image, and an absent thumbnail renders the ✨ placeholder rather than leaving empty space.

- **Per-file parse panic at startup → log + skip** (`src/app.rs::build_games_index`, wrapped in `catch_unwind`).
  - Why: a malformed file shouldn't crash the whole server. The bad file is missing from the games index (404 on request); everything else serves normally. Log identifies which file needs attention.

## Thumbnail proxy

- **Cache miss responds with 302 + `Cache-Control: no-store`** (`src/app.rs::serve_thumb`).
  - Why: without `no-store`, browsers can cache the redirect decision, meaning subsequent requests go directly to GitHub and bypass our warming cache — the entire proxy benefit evaporates. Forcing revalidation on every miss lets the cache actually warm up.

- **Populate concurrency capped at 8** (`Semaphore::new(8)` in `build_app`).
  - Why: a first-visitor burst of 30+ uncached thumbnails would otherwise spawn 30+ concurrent HTTP fetch + image decode + resize tasks. Cap lets populate happen steadily without starving request handling. Tune lower if server CPU becomes contended.

- **In-flight debouncing via Mutex<HashSet>** (`thumb_in_flight` in AppState).
  - Why: two simultaneous misses for the same (UUID, size) would otherwise spawn duplicate fetch-and-resize tasks. The set ensures only one populate per key is in flight at a time.

- **Populate failures don't poison the cache** (populate_thumbnail).
  - Why: if GitHub has a 30-second blip, caching a sentinel would lock the UUID into broken state until expiry. Instead we log + release the in-flight slot; next miss retries. Worst case for a permanently-broken UUID is one fetch attempt per request, which is cheap.

- **Single retry with 300ms delay on transient fetch failures** (populate_thumbnail).
  - Why: HTTP/2 pooled connections occasionally die mid-response — "connection closed before message complete". A cheap single retry covers the flake. 4xx/5xx are not retried (those are genuine, not transient).

- **`pool_idle_timeout(20s)` on the reqwest client** (`build_app`).
  - Why: reqwest's default idle pool timeout (90s) is longer than GitHub's server-side connection reaper, so we'd keep trying to reuse dead connections. 20s is a safety margin under GitHub's timeout.

- **`/thumb/:uuid/:size` whitelists on `thumb_originals`** (`src/app.rs::serve_thumb`).
  - Why: UUID not in the index → 404. Prevents the route from being usable as a general GitHub proxy and bounds the cache to catalog size.
