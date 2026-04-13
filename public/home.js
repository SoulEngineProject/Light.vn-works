let allData = null;

const NEW_THRESHOLD_DAYS = 90;

fetch('/api/tree')
  .then(r => {
    if (!r.ok) throw new Error('Failed to load tree');
    return r.json();
  })
  .then(data => {
    allData = data;
    renderTree(data, '', false);
    scrollToHash();
    updateGameCount(data);
    buildRibbon(data);
  })
  .catch(err => {
    document.getElementById('tree').innerHTML =
      '<p class="no-results">Error loading works: ' + err.message + '</p>';
  });

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

    // Sort newer releases first within each year
    items.sort((a, b) => {
      const ra = (a.meta && a.meta.released) || '';
      const rb = (b.meta && b.meta.released) || '';
      return rb.localeCompare(ra);
    });

    if (items.length === 0) return;
    totalVisible += items.length;

    const details = document.createElement('details');
    details.id = year.name;
    if (index === 0 || query) {
      details.setAttribute('open', '');
    }

    const summary = document.createElement('summary');
    summary.innerHTML = year.name + ' <span class="year-count">(' + items.length + ')</span>';
    details.appendChild(summary);

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

      let badges = '';
      if (isR18) badges += '<span class="card-badge card-badge-r18">R18</span>';
      if (isNew) badges += '<span class="card-badge card-badge-new">New</span>';

      const a = document.createElement('a');
      a.href = linkPath;
      a.className = 'file-card';
      a.style.animationDelay = (cardIndex * 0.04) + 's';

      let thumbHtml;
      if (item.thumbnail) {
        thumbHtml = '<div class="card-thumb">' + badges +
          '<img src="' + item.thumbnail + '" alt="' +
          escapeHtml(displayName) + '" loading="lazy" /></div>';
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

    details.appendChild(filesDiv);
    container.appendChild(details);
  });

  if (totalVisible === 0 && query) {
    container.innerHTML = '<p class="no-results">No results for "' + escapeHtml(query) + '"</p>';
  }
}

function buildRibbon(data) {
  const container = document.getElementById('ribbon');
  if (!container || !data.children) return;

  // Collect all thumbnails with paths
  const items = [];
  data.children.forEach(year => {
    if (year.children) {
      year.children.forEach(item => {
        if (item.thumbnail) {
          let path = item.path;
          if (path.endsWith('.md')) path = path.slice(0, -3);
          const title = item.name.replace(/\.md$/i, '').trim();
          items.push({ url: item.thumbnail, path: path, title: title });
        }
      });
    }
  });

  if (items.length < 6) return;

  // Shuffle
  for (let i = items.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [items[i], items[j]] = [items[j], items[i]];
  }

  // Split into two rows
  const mid = Math.ceil(items.length / 2);
  const row1 = items.slice(0, mid);
  const row2 = items.slice(mid);

  function buildTrack(entries, reverse) {
    const track = document.createElement('div');
    track.className = 'ribbon-track' + (reverse ? ' reverse' : '');

    // Duplicate for seamless loop
    const all = entries.concat(entries);
    all.forEach(entry => {
      const a = document.createElement('a');
      a.href = entry.path;
      a.title = entry.title;
      const img = document.createElement('img');
      img.src = entry.url;
      img.loading = 'lazy';
      img.alt = entry.title;
      a.appendChild(img);
      track.appendChild(a);
    });

    return track;
  }

  container.appendChild(buildTrack(row1, false));
  container.appendChild(buildTrack(row2, true));
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
  const el = document.getElementById('game-count');
  if (el) el.textContent = count;
}

function scrollToHash() {
  const hash = location.hash.replace('#', '');
  if (!hash) return;
  const el = document.getElementById(hash);
  if (el) {
    el.setAttribute('open', '');
    el.scrollIntoView({ behavior: 'smooth' });
  }
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}
