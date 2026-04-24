# Performance TODO

Ordered by impact.

## Medium impact

### Self-host the Inter font

`index.html` and `game.html` load Inter from `fonts.googleapis.com` with `preconnect` hints. Self-hosting a subset (Latin + Japanese glyphs) eliminates two external origins from the critical path and removes a render-blocking CSS request.

### Trim initial ribbon image fetches

`public/home.js::buildRibbon` kicks off up to 30 image requests on load. Even with `loading="lazy"`, browsers fetch most of them because the ribbon is above the fold.

**Fix:** cap at ~12–16 visible thumbs for the marquee, or preload only the first row and let the second lazy-load when it scrolls into view.

## Low priority (watch as the catalog grows)

### Precompress static files

`tower_http::services::ServeDir::precompressed_gzip()` serves `file.css.gz` siblings if present, eliminating per-request compression CPU for static assets. Requires a build step to generate the `.gz` files. Only worth it if CPU becomes a bottleneck under load.

### Split `TREE_DATA` once the catalog is large

`TREE_DATA` inlined into every home page HTML response grows linearly with the catalog. Fine at 241 entries; reconsider around 1000+ by splitting into per-year JSON fetched on demand.

### Upgrade `Cache-Control` to long `max-age, immutable`

Requires cache-busted filenames (build hash or version query string). Lets browsers skip even the conditional `If-Modified-Since` roundtrip for static assets. Bigger win than `no-cache` for repeat visitors, but needs a build step.

