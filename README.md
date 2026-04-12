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
