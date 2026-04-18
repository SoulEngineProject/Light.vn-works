# Light.vn-works

https://light-vn-works.onrender.com/

Light.vn Works Server + Client

## Adding or modifying a game

Make a PR with the following template.    
Filename: `works/<year>/<title>.md`  
Content:
```
---
creator: <name>
released: <YYYY/MM/DD>
link_label: <platform name, e.g. itch.io, Steam, Booth>
link_url: <url to game page>
tagline: <one-line description>
---

<paste 3-4 screenshots here - github will upload them automatically>
<img width="384" height="216" alt="image" src="....whatever github creates..." />

---
Synopsis text here.
```

For multiple links, add `extra_links`:
```
extra_links:
  - label: Steam
    url: https://store.steampowered.com/app/...
  - label: Booth
    url: https://example.booth.pm/items/...
```

For R18 games, add `tags`:
```
tags: [r18]
```

## Build and run

Requires [Rust](https://rustup.rs/).

```
cargo build
cargo run
```

Open http://localhost:8080

If changes don't appear, hard refresh with `Ctrl+Shift+R`.

### Testing on phone

1. Connect phone and PC to the same WiFi
2. Find your PC's IP: run `ipconfig` and look for the IPv4 address
3. On your phone, open `http://<your-ip>:8080`
4. Use a private/incognito tab to avoid cache issues
   - Safari: tap tabs icon → swipe to "Private" → tap +

### Tests

```
cargo test
```
