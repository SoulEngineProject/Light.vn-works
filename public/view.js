// - Decides which home-page year groups start expanded.
// - Loaded as a plain script in the browser (exposes a global) and required by the Node tests.
(function (root) {
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

  var api = { computeOpenYearFlags: computeOpenYearFlags };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.computeOpenYearFlags = computeOpenYearFlags;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
