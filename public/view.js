// - Home-page view decisions: which year groups start expanded, which works the ribbon may show.
// - Loaded as a plain script in the browser (exposes globals) and required by the Node tests.
// - Needs hasTag from search.js: a global in the browser, required in Node.
(function (root) {
  var hasTag = (typeof module !== 'undefined' && module.exports)
    ? require('./search.js').hasTag
    : function (tags, name) { return root.hasTag(tags, name); };

  // - Open the newest group, then keep opening groups until at least `minWorks`
  //   works are shown across the opened groups (cascade).
  // - A query opens every group.
  // - `visibleCounts` is the work count per rendered group, in display order.
  function computeOpenYearFlags(visibleCounts, hasQuery, minWorks) {
    var threshold = (minWorks === undefined) ? 6 : minWorks;
    var prior = 0;
    return visibleCounts.map(function (count) {
      var open = hasQuery || prior < threshold;
      prior += count;
      return open;
    });
  }

  // - Works the ribbon may show, in tree order (the caller shuffles).
  // - Only works with a thumbnail; R18 works are left out while "Hide R18" is on.
  // - `data` is the server-embedded tree: years, each with `children` works.
  function ribbonEntries(data, hideR18) {
    var entries = [];
    ((data && data.children) || []).forEach(function (year) {
      (year.children || []).forEach(function (item) {
        var tags = (item.meta && item.meta.tags) ? item.meta.tags : [];
        if (!item.thumbnail || (hideR18 && hasTag(tags, 'r18'))) {
          return;
        }
        entries.push({
          // - thumbnail_ribbon is the smaller (240x140) proxy URL; plain thumbnail covers non-proxied images
          url: item.thumbnail_ribbon || item.thumbnail,
          href: item.href,
          title: item.title,
          composite: !!item.thumbnail_composite
        });
      });
    });
    return entries;
  }

  var api = { computeOpenYearFlags: computeOpenYearFlags, ribbonEntries: ribbonEntries };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.computeOpenYearFlags = computeOpenYearFlags;
    root.ribbonEntries = ribbonEntries;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
