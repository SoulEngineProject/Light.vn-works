// - Search matching for the home-page filter.
// - Loaded as a plain script in the browser (exposes globals) and required by the Node tests.
(function (root) {
  // - Parse a `tag:` query into the exact tag name to match on.
  // - Returns null for plain-text queries, which fall back to broad substring search.
  // - Handles an optional pair of surrounding quotes so multi-word tags work: tag:"a b".
  function parseTagQuery(query) {
    var q = (query || '').trim();
    if (q.slice(0, 4).toLowerCase() !== 'tag:') {
      return null;
    }
    var rest = q.slice(4).trim();
    if (rest.length >= 2 && rest[0] === '"' && rest[rest.length - 1] === '"') {
      rest = rest.slice(1, -1).trim();
    }
    return rest ? rest.toLowerCase() : null;
  }

  // - Whether a work matches the search box.
  // - `tag:` queries match a tag exactly; everything else is broad substring over title / creator / tags.
  // - Matching is case-insensitive throughout.
  function workMatchesSearch(query, name, creator, tags) {
    var q = (query || '').trim().toLowerCase();
    if (!q) {
      return true;
    }
    var tag = parseTagQuery(q);
    if (tag !== null) {
      return tags.some(function (t) { return t.toLowerCase() === tag; });
    }
    var joined = tags.join(' ').toLowerCase();
    return name.toLowerCase().indexOf(q) !== -1 ||
      creator.toLowerCase().indexOf(q) !== -1 ||
      joined.indexOf(q) !== -1;
  }

  var api = { parseTagQuery: parseTagQuery, workMatchesSearch: workMatchesSearch };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.parseTagQuery = parseTagQuery;
    root.workMatchesSearch = workMatchesSearch;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
