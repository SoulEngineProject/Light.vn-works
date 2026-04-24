# Failure Considerations

Non-obvious graceful-degradation choices. Don't "fix" these into stricter behavior without understanding the reasoning.

## Ribbon & image loading

- **Ribbon fades in after 6s regardless of image state** (`public/home.js::buildRibbon`).
  - Why: on bad networks, image fetches can hang for minutes per attempt. The wall-clock fallback guarantees the ribbon appears.

- **Permanent-failure count only after retries exhausted** (`public/home.js`, `error` listener checks `data-error='1'`).
  - Why: intermediate errors are still retrying. Counting them immediately would reveal a broken-looking ribbon and defeat the 50% threshold.

- **Broken images hidden via CSS, not replaced** (`public/home.css`: `img[data-error] { display: none }`).
  - Why: collapsing the space degrades more gracefully than showing a torn-page icon.

- **`retryImage` backoff: 1s then 3s, max 2 retries** (`public/home.js::retryImage`).
  - Why: GitHub user-attachments occasionally flakes; most transient failures resolve within ~5s.

- **Dark gradient background under thumbnail containers** (`public/style.css`, combined rule targeting `.card-thumb`, `.ribbon-track img`, `.more-creator-thumb`).
  - Why: browsers render empty `<img>` elements as native-default white before content loads. On the dark UI, that flashes a visible white frame during lazy-load or year-section expansion. The gradient fills the pre-load state so the transition is "dark → image" instead of "white → image". Don't remove thinking it's dead code — the img covers it once loaded.

## Accessibility

- **`alt=""` on homepage card thumbnails** (`public/home.js`, card path).
  - Why: `.card-title` below the image is the accessible label. Alt text flashed as an ugly overlay during slow loads.

- **`alt=""` on ribbon thumbnails + `aria-label` on the link** (`public/home.js::buildTrack`).
  - Why: same flash problem as cards; link-level `aria-label` is the semantically correct accessible name when the img is decorative.

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
