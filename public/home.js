let allData = null;

fetch('/api/tree')
  .then(r => {
    if (!r.ok) throw new Error('Failed to load tree');
    return r.json();
  })
  .then(data => {
    allData = data;
    renderTree(data, '', false);
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
    const items = (year.children || []).filter(item => {
      if (item.is_dir) return false;

      const tags = (item.meta && item.meta.tags) ? item.meta.tags : [];
      if (hideR18 && tags.includes('r18')) return false;

      if (!query) return true;

      const name = item.name.replace(/\.md$/i, '').toLowerCase();
      const creator = (item.meta && item.meta.creator) ? item.meta.creator.toLowerCase() : '';
      return name.includes(query) || creator.includes(query);
    });

    if (items.length === 0) return;
    totalVisible += items.length;

    const details = document.createElement('details');
    if (index === 0 || query) {
      details.setAttribute('open', '');
    }

    const summary = document.createElement('summary');
    summary.innerHTML = year.name + ' <span class="year-count">(' + items.length + ')</span>';
    details.appendChild(summary);

    const filesDiv = document.createElement('div');
    filesDiv.className = 'files';

    items.forEach(item => {
      const displayName = item.name.replace(/\.md$/i, '').trim();
      let linkPath = item.path;
      if (linkPath.endsWith('.md')) linkPath = linkPath.slice(0, -3);

      const creator = (item.meta && item.meta.creator) ? item.meta.creator : '';
      const tagline = (item.meta && item.meta.tagline) ? item.meta.tagline : '';
      const tags = (item.meta && item.meta.tags) ? item.meta.tags : [];
      const isR18 = tags.includes('r18');
      const badgeHtml = isR18 ? '<span class="card-badge">R18</span>' : '';

      const a = document.createElement('a');
      a.href = linkPath;
      a.className = 'file-card';

      let thumbHtml;
      if (item.thumbnail) {
        thumbHtml = '<div class="card-thumb">' + badgeHtml +
          '<img src="' + item.thumbnail + '" alt="' +
          escapeHtml(displayName) + '" loading="lazy" /></div>';
      } else {
        thumbHtml = '<div class="card-thumb-placeholder">' + badgeHtml + '\u2728</div>';
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

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}
