# Performance TODO

Ordered by impact.

## Medium impact

### Self-host the Inter font

`index.html` and `game.html` load Inter from `fonts.googleapis.com` with `preconnect` hints. Self-hosting a subset (Latin + Japanese glyphs) eliminates two external origins from the critical path and removes a render-blocking CSS request.

### Trim initial ribbon image fetches

`public/home.js::buildRibbon` kicks off up to 30 image requests on load. Largely defanged by the `/thumb` proxy — ribbon thumbs are now ~11KB WebP instead of ~150KB full-size, so total ribbon bandwidth is ~330KB on cold browser cache. Only worth revisiting if the ribbon becomes measurably slow on mobile or catalogs scale to thousands of items.

## Low priority (watch as the catalog grows)

### Precompress static files

`tower_http::services::ServeDir::precompressed_gzip()` serves `file.css.gz` siblings if present, eliminating per-request compression CPU for static assets. Requires a build step to generate the `.gz` files. Only worth it if CPU becomes a bottleneck under load.

### Split `TREE_DATA` once the catalog is large

`TREE_DATA` inlined into every home page HTML response grows linearly with the catalog. Fine at 241 entries; reconsider around 1000+ by splitting into per-year JSON fetched on demand.

### Upgrade `Cache-Control` to long `max-age, immutable`

Requires cache-busted filenames (build hash or version query string). Lets browsers skip even the conditional `If-Modified-Since` roundtrip for static assets. Bigger win than `no-cache` for repeat visitors, but needs a build step.

