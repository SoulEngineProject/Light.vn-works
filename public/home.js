let allData = null;
let t = {}; // translations

const LANG_PARAM = new URLSearchParams(location.search).get('lang');
const LANG = LANG_PARAM === 'ja' || LANG_PARAM === 'en'
  ? LANG_PARAM
  : (navigator.language.startsWith('ja') ? 'ja' : 'en');

// Restore state from URL
const PARAMS = new URLSearchParams(location.search);
const R18_PARAM = PARAMS.get('r18');
const SEARCH_PARAM = PARAMS.get('search');
if (R18_PARAM === '0') {
  document.getElementById('hide-r18').checked = false;
}
if (SEARCH_PARAM) {
  document.getElementById('search').value = SEARCH_PARAM;
  if (SEARCH_PARAM === 'r18') {
    document.getElementById('hide-r18').checked = false;
  }
}

// Load translations from embedded data
if (typeof LANG_DATA !== 'undefined') {
  for (var key in LANG_DATA) {
    t[key] = LANG_DATA[key][LANG] || LANG_DATA[key]['en'] || '';
  }
}

// Setup language toggle and apply translations immediately
setupLangToggle();
applyStaticTranslations();

// - Render from server-embedded data (TREE_DATA, LANG_DATA, TAG_INFO, TAG_BAR)
// - No API fetch needed; everything is baked into the HTML at serve time
if (typeof TREE_DATA !== 'undefined') {
  allData = TREE_DATA;
  var initialQuery = document.getElementById('search').value.trim().toLowerCase();
  renderTree(TREE_DATA, initialQuery, document.getElementById('hide-r18').checked);
  scrollToHash();
  updateGameCount(TREE_DATA);
  buildRibbon(TREE_DATA);
  buildTagBar();
}

function applyStaticTranslations() {
  setHtml('lang-managed-by', t.managed_by);
  setText('lang-subtitle', t.subtitle);
  setText('lang-cta', t.cta);
  setText('lang-contribute', t.contribute);
  setText('lang-contribute-link', t.contribute_link);
  var contributeLink = document.getElementById('lang-contribute-link');
  if (contributeLink && t.contribute_url) {
    contributeLink.href = t.contribute_url;
  }
  setText('lang-hide-r18', t.hide_r18);

  const search = document.getElementById('search');
  if (search) {
    search.placeholder = t.search_placeholder;
  }

  const cta = document.getElementById('lang-cta');
  if (cta && t.engine_url) {
    cta.href = t.engine_url;
  }
}

function setupLangToggle() {
  const btn = document.getElementById('lang-toggle');
  if (!btn) {
    return;
  }
  btn.textContent = LANG === 'ja' ? 'English' : '日本語';
  btn.addEventListener('click', function() {
    const other = LANG === 'ja' ? 'en' : 'ja';
    const url = new URL(location.href);
    url.searchParams.set('lang', other);
    if (!document.getElementById('hide-r18').checked) {
      url.searchParams.set('r18', '0');
    } else {
      url.searchParams.delete('r18');
    }
    location.href = url.toString();
  });
}

function setText(id, text) {
  const el = document.getElementById(id);
  if (el && text) {
    el.textContent = text;
  }
}

function setHtml(id, html) {
  const el = document.getElementById(id);
  if (el && html) {
    el.innerHTML = html;
  }
}

// Percent-encode each path segment individually so reserved chars like '#'
// in titles aren't read as fragment separators by the browser.
function encodePath(path) {
  return path.split('/').map(encodeURIComponent).join('/');
}

// - Build an href that preserves lang + r18 state so navigation doesn't reset the user's filter
// - Reads live checkbox state each call so mid-session toggles reflect on re-render
function buildHref(linkPath) {
  var parts = [];
  if (LANG_PARAM) {
    parts.push('lang=' + LANG);
  }
  var hideR18 = document.getElementById('hide-r18');
  if (hideR18 && !hideR18.checked) {
    parts.push('r18=0');
  }
  return encodePath(linkPath) + (parts.length ? '?' + parts.join('&') : '');
}

// - Debounce typing so we don't churn the URL bar on every keystroke
// - ~250ms feels synchronous when sharing, but long enough that mid-word pauses don't trigger writes
var syncTimer = null;
document.getElementById('search').addEventListener('input', function() {
  rerender();
  syncActiveTagChip();
  clearTimeout(syncTimer);
  syncTimer = setTimeout(syncUrl, 250);
});
document.getElementById('hide-r18').addEventListener('change', function() {
  rerender();
  syncUrl();
});

function rerender() {
  if (allData) {
    const query = document.getElementById('search').value.trim().toLowerCase();
    const hideR18 = document.getElementById('hide-r18').checked;
    renderTree(allData, query, hideR18);
  }
}

// - Mirror current UI state into the URL via replaceState for shareable links, without polluting browser history on every keystroke
// - Does not touch ?lang (owned by the language toggle handler) or the hash
function syncUrl() {
  const url = new URL(location.href);
  const search = document.getElementById('search').value.trim();
  const hideR18 = document.getElementById('hide-r18').checked;
  if (search) {
    url.searchParams.set('search', search);
  } else {
    url.searchParams.delete('search');
  }
  if (!hideR18) {
    url.searchParams.set('r18', '0');
  } else {
    url.searchParams.delete('r18');
  }
  history.replaceState(null, '', url.toString());
}

// - Pick the tag for the top-right priority badge slot
// - Mirrors src/lib.rs::pick_priority_tag
// - Priority order: R18 → Terrace and Ray → first other configured tag whose group has card_priority_badge: true
// - AI and language tags have card_priority_badge: false in tags.yaml, so they never promote here: AI uses the left slot, languages are metadata for the tag-bar filter only
function pickPriorityTag(tags, tagInfo) {
  let t = tags.find(x => x.toLowerCase() === 'r18');
  if (t) {
    return t;
  }
  t = tags.find(x => x.toLowerCase() === 'terrace and ray');
  if (t) {
    return t;
  }
  t = tags.find(x => {
    const info = tagInfo[x.toLowerCase()];
    return info && info.card_priority_badge;
  });
  return t || null;
}

function renderTree(data, query, hideR18) {
  const container = document.getElementById('tree');
  container.innerHTML = '';

  if (!data.children || data.children.length === 0) {
    container.innerHTML = '<p class="no-results">No works found yet.</p>';
    return;
  }

  const sortedYears = data.children
    .filter(y => y.is_dir && y.name.match(/^\d{4}$/))
    .sort((a, b) => b.name.localeCompare(a.name));

  let totalVisible = 0;

  sortedYears.forEach((year, index) => {
    let items = (year.children || []).filter(item => {
      if (item.is_dir) {
        return false;
      }

      const tags = (item.meta && item.meta.tags) ? item.meta.tags : [];
      if (hideR18 && tags.includes('r18')) {
        return false;
      }

      if (!query) {
        return true;
      }

      const name = item.name.replace(/\.md$/i, '');
      const creator = (item.meta && item.meta.creator) ? item.meta.creator : '';
      return workMatchesSearch(query, name, creator, tags);
    });

    items.sort((a, b) => {
      const ra = (a.meta && a.meta.released) || '';
      const rb = (b.meta && b.meta.released) || '';
      return rb.localeCompare(ra);
    });

    if (items.length === 0) {
      return;
    }
    totalVisible += items.length;

    const section = document.createElement('div');
    section.className = 'year-section';
    section.id = year.name;
    if (index === 0 || query) {
      section.classList.add('open');
    }

    const summary = document.createElement('div');
    summary.className = 'year-header';
    summary.innerHTML = year.name + ' <span class="year-count">(' + items.length + ')</span>';
    summary.addEventListener('click', function() {
      section.classList.toggle('open');
    });
    section.appendChild(summary);

    const filesDiv = document.createElement('div');
    filesDiv.className = 'files';

    items.forEach(item => {
      const displayName = item.name.replace(/\.md$/i, '').trim();
      let linkPath = item.path;
      if (linkPath.endsWith('.md')) {
        linkPath = linkPath.slice(0, -3);
      }

      const creator = (item.meta && item.meta.creator) ? item.meta.creator : '';
      const tagline = (item.meta && item.meta.tagline) ? item.meta.tagline : '';
      const tags = (item.meta && item.meta.tags) ? item.meta.tags : [];

      var tagInfo = (typeof TAG_INFO !== 'undefined') ? TAG_INFO : {};
      // - Two-slot layout: priority badge (top-right) + AI (top-left)
      // - See pickPriorityTag() for priority order
      let badges = '';
      const priorityTag = pickPriorityTag(tags, tagInfo);
      if (priorityTag) {
        const colour = tagInfo[priorityTag.toLowerCase()].colour;
        badges += '<span class="card-badge" style="background:' + colour + ';color:white">' + escapeHtml(priorityTag.toUpperCase()) + '</span>';
      }
      const aiInfo = tagInfo['ai'];
      if (aiInfo && tags.some(x => x.toLowerCase() === 'ai')) {
        badges += '<span class="card-badge card-badge-left" style="background:' + aiInfo.colour + ';color:white">AI</span>';
      }

      const a = document.createElement('a');
      a.href = buildHref(linkPath);
      a.className = 'file-card';

      let thumbHtml;
      if (item.thumbnail && item.thumbnail_composite) {
        thumbHtml = '<div class="card-thumb">' + badges +
          '<div class="card-thumb-composite" style="background-image:url(\'' + item.thumbnail + '\')"></div></div>';
      } else if (item.thumbnail) {
        // alt="" is intentional: .card-title below is the accessible label, and empty alt avoids flashing game titles in the image box during slow loads
        thumbHtml = '<div class="card-thumb">' + badges +
          '<img src="' + item.thumbnail + '" alt="" loading="lazy" onerror="retryImage.call(this)" /></div>';
      } else {
        thumbHtml = '<div class="card-thumb-placeholder">' + badges + '✨</div>';
      }

      a.innerHTML = thumbHtml +
        '<div class="card-body">' +
          '<div class="card-title">' + escapeHtml(displayName) + '</div>' +
          (creator ? '<div class="card-creator">by ' + escapeHtml(creator) + '</div>' : '') +
          (tagline ? '<div class="card-tagline">' + escapeHtml(tagline) + '</div>' : '') +
        '</div>';

      filesDiv.appendChild(a);
    });

    section.appendChild(filesDiv);
    container.appendChild(section);
  });

  if (totalVisible === 0 && query) {
    // Show the tag name for a tag: query, otherwise the raw text.
    const shown = parseTagQuery(query) || query;
    const msg = (t.no_results || 'No results for "{q}"').replace('{q}', escapeHtml(shown));
    container.innerHTML = '<p class="no-results">' + msg + '</p>';
  }
}

function buildRibbon(data) {
  const container = document.getElementById('ribbon');
  if (!container || !data.children) {
    return;
  }

  const items = [];
  data.children.forEach(year => {
    if (year.children) {
      year.children.forEach(item => {
        if (item.thumbnail) {
          let path = item.path;
          if (path.endsWith('.md')) {
            path = path.slice(0, -3);
          }
          const title = item.name.replace(/\.md$/i, '').trim();
          // - thumbnail_ribbon is the smaller (240x140) proxy URL for GitHub user-attachments
          // - Falls back to thumbnail for non-proxied URLs
          const url = item.thumbnail_ribbon || item.thumbnail;
          items.push({ url: url, path: path, title: title, composite: !!item.thumbnail_composite });
        }
      });
    }
  });

  if (items.length < 6) {
    return;
  }

  // Shuffle and cap to reduce initial image load
  for (let i = items.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [items[i], items[j]] = [items[j], items[i]];
  }
  const capped = items.slice(0, 30);

  const mid = Math.ceil(capped.length / 2);
  const row1 = capped.slice(0, mid);
  const row2 = capped.slice(mid);

  function buildTrack(entries, reverse) {
    const track = document.createElement('div');
    track.className = 'ribbon-track' + (reverse ? ' reverse' : '');

    // - Track the first N <img> elements per row: visible at rest, worth fetching before the off-screen ones
    // - Composites are skipped (they use background-image, not <img>, so fetchpriority doesn't apply)
    const HIGH_PRIORITY_LIMIT = 3;
    let imgIdx = 0;

    const all = entries.concat(entries);
    all.forEach(entry => {
      const a = document.createElement('a');
      a.href = buildHref(entry.path);
      // - aria-label is the accessible name for the link; title is the sighted-hover tooltip
      // - img below uses alt="" since this link already carries the name
      a.title = entry.title;
      a.setAttribute('aria-label', entry.title);
      if (entry.composite) {
        const div = document.createElement('div');
        div.className = 'ribbon-thumb-composite';
        div.style.backgroundImage = "url('" + entry.url + "')";
        a.appendChild(div);
      } else {
        const img = document.createElement('img');
        img.src = entry.url;
        img.loading = 'lazy';
        img.alt = '';
        img.onerror = retryImage;
        // - First ~3 visible at start get priority; the rest are off-screen, revealed via marquee scroll
        // - Older browsers ignore the hint
        img.fetchPriority = imgIdx < HIGH_PRIORITY_LIMIT ? 'high' : 'low';
        imgIdx++;
        a.appendChild(img);
      }
      track.appendChild(a);
    });

    return track;
  }

  container.appendChild(buildTrack(row1, false));
  container.appendChild(buildTrack(row2, true));

  // - Reveal when there's content to show
  // - The first image's `load` event triggers fade-in, pairing with fetchpriority on visible imgs so content is already populated by reveal time
  // - Composite-only ribbon: reveal immediately, since composites don't fire load events but do have content
  // - If every image fails permanently, ribbon stays hidden — correct, since a "revealed" empty container looks identical to a hidden one (opacity doesn't affect layout, container has no visible chrome of its own)
  var ribbonImages = container.querySelectorAll('img');
  function reveal() {
    container.classList.add('loaded');
  }
  if (ribbonImages.length === 0) {
    reveal();
  }
  ribbonImages.forEach(function(img) {
    if (img.complete) {
      reveal();
    } else {
      img.addEventListener('load', reveal, { once: true });
    }
  });
}

// - Render the tag-filter bar from server-embedded TAG_BAR
// - Each chip is a button that sets the search input to its tag name (idempotent click)
// - Configured tags use their colour; unconfigured tags get the default style
function buildTagBar() {
  const container = document.getElementById('tag-bar');
  if (!container || typeof TAG_BAR === 'undefined' || !TAG_BAR.length) {
    return;
  }

  const search = document.getElementById('search');

  TAG_BAR.forEach(function(entry) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'tag-chip' + (entry.colour ? '' : ' tag-default');
    btn.dataset.tag = entry.name;
    if (entry.colour) {
      btn.style.background = entry.colour;
      btn.style.color = 'white';
    }
    btn.innerHTML = escapeHtml(entry.name.toUpperCase()) +
      ' <span class="tag-chip-count">' + entry.count + '</span>';

    btn.addEventListener('click', function() {
      // - Toggle: clicking the active chip clears the filter; otherwise set a tag: query
      // - Multi-word tags are quoted so they parse as one token
      const current = parseTagQuery(search.value.trim().toLowerCase());
      if (current === entry.name.toLowerCase()) {
        search.value = '';
      } else {
        search.value = 'tag:' + (/\s/.test(entry.name) ? '"' + entry.name + '"' : entry.name);
      }
      rerender();
      syncActiveTagChip();
      syncUrl();
      search.focus();
    });

    container.appendChild(btn);
  });

  container.hidden = false;
  syncActiveTagChip();
}

// - Mark the chip matching the current tag: query as active
// - Only a tag: query activates a chip: plain text like "spook" parses to null and lights nothing, while tag:Spooktober activates the Spooktober chip
function syncActiveTagChip() {
  const container = document.getElementById('tag-bar');
  if (!container) {
    return;
  }
  const query = document.getElementById('search').value.trim().toLowerCase();
  const activeTag = parseTagQuery(query);
  const chips = container.querySelectorAll('.tag-chip');
  let anyActive = false;
  chips.forEach(function(chip) {
    const isActive = activeTag !== null && chip.dataset.tag.toLowerCase() === activeTag;
    chip.classList.toggle('active', isActive);
    if (isActive) {
      chip.setAttribute('aria-pressed', 'true');
      anyActive = true;
    } else {
      chip.removeAttribute('aria-pressed');
    }
  });
  container.classList.toggle('has-active', anyActive);
}

function updateGameCount(data) {
  let count = 0;
  if (data.children) {
    data.children.forEach(year => {
      if (year.children) {
        count += year.children.filter(c => !c.is_dir).length;
      }
    });
  }
  const el = document.getElementById('lang-game-count');
  if (el) {
    el.innerHTML = (t.game_count || '{n} games and counting.').replace('{n}', count);
  }
}

function scrollToHash() {
  const hash = location.hash.replace('#', '');
  if (!hash) {
    return;
  }
  const el = document.getElementById(hash);
  if (el) {
    el.classList.add('open');
    el.scrollIntoView({ behavior: 'smooth' });
  }
}

function retryImage() {
  var retries = parseInt(this.dataset.retries || '0');
  if (retries < 2) {
    this.dataset.retries = retries + 1;
    var img = this;
    var src = img.src;
    img.src = '';
    setTimeout(function() { img.src = src; }, retries === 0 ? 1000 : 3000);
  } else {
    this.dataset.error = '1';
  }
}


function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}
