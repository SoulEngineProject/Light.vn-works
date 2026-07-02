# Searchability / SEO

## To Self: What SEO is
- SEO (Search Engine Optimization) is making a site easy for search engines to discover, understand, and index, so its pages surface in search results.
- Two halves matter here:
  - **Discovery** — can a crawler find every page? (Handled by the sitemap + robots.txt below.)
  - **Understanding** — once found, does each page describe itself? (Handled by per-page `<title>` / `description` / `og:*` tags.)
- It also covers **link previews**: the `og:*` tags decide how a URL looks when pasted into Discord, X, Bluesky, Slack, etc. Every major platform reads `og:*`, including X (which falls back to it when the X-only `twitter:*` tags are absent), so we don't bother with `twitter:*`.

## The discoverability problem
- The home page builds its game links in **JavaScript** (`home.js` renders the year tree client-side from embedded `TREE_DATA`).
- A crawler that doesn't execute JS sees a near-empty `<body>` with no links to the game pages.
- So the individual `/works/YYYY/title` pages — the actual content — are effectively unreachable by crawlers unless we hand them the list directly.

## Sitemap
- `/sitemap.xml` (`serve_sitemap` in `src/app.rs`, built by `build_sitemap` in `src/lib.rs`).
- Generated from the in-memory games index (`state.games`) on each request, so it's always current after a restart — no separate build step.
- One `<loc>` for the home page plus one per game. URLs are **absolute** and each path segment is **percent-encoded** (game titles contain spaces and non-ASCII). Sorted for deterministic output.
- **No `<lastmod>` — deliberate.** `<lastmod>` means "last modified", but the only date we have is `released` (published), which never changes when a page is later edited. So it would mislead crawlers — e.g. adding a tag to a 2017 work today would still advertise a 2017 date. Since search engines also largely ignore inconsistent `<lastmod>`, and a truly-accurate value (git per-file date) doesn't survive Render's mtime-resetting deploys, we omit it entirely and let crawlers schedule their own re-crawls.

## robots.txt
- `/robots.txt` (`serve_robots`) allows all crawlers and points them at the sitemap.
- Generated (not a static file) so the `Sitemap:` line always carries the right absolute base.

## Base URL
- `base_url()` resolves the absolute origin used in both handlers:
  - `BASE_URL` env var wins (pin it for a custom domain).
  - Otherwise derived from the request's `X-Forwarded-Proto` (default `https`) + `Host` header, so it self-adjusts to whatever domain the sitemap is fetched from.
  - Falls back to the production host if no `Host` header is present.

## Per-page metadata
- Game pages (`public/game.html`) are server-rendered HTML with `<title>`, `meta description` (the tagline), and `og:title` / `og:description` / `og:image` / `og:url`. `og:image` is the first screenshot (already an absolute GitHub URL). Once a crawler reaches them (via the sitemap), they're fully indexable and share nicely.
- The home page (`public/index.html`) has its own `og:*` tags; `og:image` is the site icon, rendered as an **absolute** URL (built from `base_url()`) so link-preview scrapers can fetch it.

## Canonical URLs
- Both pages emit `<link rel="canonical">` + `og:url` pointing at the **param-less** absolute URL (`base_url()` + the path).
- Why: the app appends `?lang=`, `?r18=`, and `?search=`, which would otherwise look like many duplicate pages and split ranking signals. The canonical consolidates them onto one URL.
- Language: canonical points at the default (English) URL. Proper `hreflang` alternates for the `?lang=ja` variant are deferred — lang is a query param over server-rendered content, so hreflang is a larger change.

## Known gaps (not yet done)
- No `hreflang` alternates for en/ja (see Canonical URLs).
- The home page `<html lang="en">` is fixed even when `?lang=ja` renders Japanese content.
- No structured data (JSON-LD) — optional, only worth it if rich results are a goal.
