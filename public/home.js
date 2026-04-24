let allData = null;
let t = {}; // translations

const NEW_THRESHOLD_DAYS = 90;
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

// Render from server-embedded data (TREE_DATA, LANG_DATA, TAG_COLOURS).
// No API fetch needed — everything is baked into the HTML at serve time.
if (typeof TREE_DATA !== 'undefined') {
  allData = TREE_DATA;
  var initialQuery = document.getElementById('search').value.trim().toLowerCase();
  renderTree(TREE_DATA, initialQuery, document.getElementById('hide-r18').checked);
  scrollToHash();
  updateGameCount(TREE_DATA);
  buildRibbon(TREE_DATA);
}

function applyStaticTranslations() {
  setHtml('lang-managed-by', t.managed_by);
  setText('lang-subtitle', t.subtitle);
  setText('lang-cta', t.cta);
  setText('lang-contribute', t.contribute);
  setText('lang-contribute-link', t.contribute_link);
  var contributeLink = document.getElementById('lang-contribute-link');
  if (contributeLink && t.contribute_url) contributeLink.href = t.contribute_url;
  setText('lang-hide-r18', t.hide_r18);

  const search = document.getElementById('search');
  if (search) search.placeholder = t.search_placeholder;

  const cta = document.getElementById('lang-cta');
  if (cta && t.engine_url) cta.href = t.engine_url;
}

function setupLangToggle() {
  const btn = document.getElementById('lang-toggle');
  if (!btn) return;
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
  if (el && text) el.textContent = text;
}

function setHtml(id, html) {
  const el = document.getElementById(id);
  if (el && html) el.innerHTML = html;
}

// Percent-encode each path segment individually so reserved chars like '#'
// in titles don't get interpreted as fragment separators by the browser.
function encodePath(path) {
  return path.split('/').map(encodeURIComponent).join('/');
}

// Build an href that preserves lang + r18 state so navigation doesn't silently
// reset the user's filter. Reads live checkbox state each call so mid-session
// toggles are reflected on re-render.
function buildHref(linkPath) {
  var parts = [];
  if (LANG_PARAM) parts.push('lang=' + LANG);
  var hideR18 = document.getElementById('hide-r18');
  if (hideR18 && !hideR18.checked) parts.push('r18=0');
  return encodePath(linkPath) + (parts.length ? '?' + parts.join('&') : '');
}

document.getElementById('search').addEventListener('input', rerender);
document.getElementById('hide-r18').addEventListener('change', rerender);

function rerender() {
  if (allData) {
    const query = document.getElementById('search').value.trim().toLowerCase();
    const hideR18 = document.getElementById('hide-r18').checked;
    renderTree(allData, query, hideR18);
  }
}

function isNewGame(released) {
  if (!released) return false;
  const parts = released.split('/');
  if (parts.length < 3) return false;
  const date = new Date(parts[0], parts[1] - 1, parts[2]);
  const now = new Date();
  return (now - date) / (1000 * 60 * 60 * 24) <= NEW_THRESHOLD_DAYS;
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
  const newBadgeText = t.new_badge || 'New';

  sortedYears.forEach((year, index) => {
    let items = (year.children || []).filter(item => {
      if (item.is_dir) return false;

      const tags = (item.meta && item.meta.tags) ? item.meta.tags : [];
      if (hideR18 && tags.includes('r18')) return false;

      if (!query) return true;

      const name = item.name.replace(/\.md$/i, '').toLowerCase();
      const creator = (item.meta && item.meta.creator) ? item.meta.creator.toLowerCase() : '';
      const tagStr = tags.join(' ').toLowerCase();
      return name.includes(query) || creator.includes(query) || tagStr.includes(query);
    });

    items.sort((a, b) => {
      const ra = (a.meta && a.meta.released) || '';
      const rb = (b.meta && b.meta.released) || '';
      return rb.localeCompare(ra);
    });

    if (items.length === 0) return;
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

    items.forEach((item, cardIndex) => {
      const displayName = item.name.replace(/\.md$/i, '').trim();
      let linkPath = item.path;
      if (linkPath.endsWith('.md')) linkPath = linkPath.slice(0, -3);

      const creator = (item.meta && item.meta.creator) ? item.meta.creator : '';
      const tagline = (item.meta && item.meta.tagline) ? item.meta.tagline : '';
      const released = (item.meta && item.meta.released) ? item.meta.released : '';
      const tags = (item.meta && item.meta.tags) ? item.meta.tags : [];
      const isR18 = tags.includes('r18');
      const isNew = isNewGame(released);

      var tagColours = (typeof TAG_COLOURS !== 'undefined') ? TAG_COLOURS : {};
      let badges = '';
      tags.forEach(function(tag) {
        var colour = tagColours[tag.toLowerCase()];
        if (!colour) return;
        badges += '<span class="card-badge" style="background:' + colour + ';color:white">' + escapeHtml(tag.toUpperCase()) + '</span>';
      });
      if (isNew) badges += '<span class="card-badge badge-new">' + escapeHtml(newBadgeText) + '</span>';

      const a = document.createElement('a');
      a.href = buildHref(linkPath);
      a.className = 'file-card';

      let thumbHtml;
      if (item.thumbnail && item.thumbnail_composite) {
        thumbHtml = '<div class="card-thumb">' + badges +
          '<div class="card-thumb-composite" style="background-image:url(\'' + item.thumbnail + '\')"></div></div>';
      } else if (item.thumbnail) {
        // alt="" is intentional: .card-title below is the accessible label, and empty alt avoids flashing game titles in the image box during slow loads.
        thumbHtml = '<div class="card-thumb">' + badges +
          '<img src="' + item.thumbnail + '" alt="" loading="lazy" onerror="retryImage.call(this)" /></div>';
      } else {
        thumbHtml = '<div class="card-thumb-placeholder">' + badges + '\u2728</div>';
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
    const msg = (t.no_results || 'No results for "{q}"').replace('{q}', escapeHtml(query));
    container.innerHTML = '<p class="no-results">' + msg + '</p>';
  }
}

function buildRibbon(data) {
  const container = document.getElementById('ribbon');
  if (!container || !data.children) return;

  const items = [];
  data.children.forEach(year => {
    if (year.children) {
      year.children.forEach(item => {
        if (item.thumbnail) {
          let path = item.path;
          if (path.endsWith('.md')) path = path.slice(0, -3);
          const title = item.name.replace(/\.md$/i, '').trim();
          // thumbnail_ribbon is the smaller (240x140) proxy URL for GitHub
          // user-attachments; falls back to thumbnail for non-proxied URLs.
          const url = item.thumbnail_ribbon || item.thumbnail;
          items.push({ url: url, path: path, title: title, composite: !!item.thumbnail_composite });
        }
      });
    }
  });

  if (items.length < 6) return;

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

    const all = entries.concat(entries);
    all.forEach(entry => {
      const a = document.createElement('a');
      a.href = buildHref(entry.path);
      // aria-label is the accessible name for the link; title is the sighted-hover tooltip. img below uses alt="" since this link already carries the name.
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
        a.appendChild(img);
      }
      track.appendChild(a);
    });

    return track;
  }

  container.appendChild(buildTrack(row1, false));
  container.appendChild(buildTrack(row2, true));

  // Ribbon starts invisible (CSS opacity:0). Fade in once 50% of images
  // have settled (loaded or permanently failed) to avoid progressive
  // pop-in. 6s hard fallback caps worst-case latency on bad networks.
  var ribbonImages = container.querySelectorAll('img');
  var loaded = 0;
  var total = ribbonImages.length;
  var threshold = Math.floor(total * 0.5);
  function tick() {
    loaded++;
    if (loaded >= threshold) container.classList.add('loaded');
  }
  ribbonImages.forEach(function(img) {
    if (img.complete) {
      loaded++;
      return;
    }
    img.addEventListener('load', tick);
    img.addEventListener('error', function() {
      // retryImage sets data-error='1' only on the final failed retry;
      // intermediate errors are still retrying and shouldn't count.
      if (img.dataset.error === '1') tick();
    });
  });
  if (loaded >= threshold) container.classList.add('loaded');
  setTimeout(function() { container.classList.add('loaded'); }, 6000);
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
  if (el) el.innerHTML = (t.game_count || '{n} games and counting.').replace('{n}', count);
}

function scrollToHash() {
  const hash = location.hash.replace('#', '');
  if (!hash) return;
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
