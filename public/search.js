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

  // - The work's own spelling of tag `name`, matched case-insensitively; undefined if absent.
  // - Every tag comparison goes through here (or hasTag) so none can quietly be case-sensitive.
  // - Mirrors src/lib.rs::find_tag / has_tag.
  function findTag(tags, name) {
    var want = name.toLowerCase();
    return (tags || []).find(function (t) { return t.toLowerCase() === want; });
  }

  function hasTag(tags, name) {
    return findTag(tags, name) !== undefined;
  }

  // - The search that filters to exactly one tag: tag:ai, or tag:"Terrace and Ray" when the name has spaces.
  // - Mirrors src/lib.rs::tag_query; both are checked against tests/fixtures/tag_queries.json.
  function tagQuery(name) {
    return name.indexOf(' ') !== -1 ? 'tag:"' + name + '"' : 'tag:' + name;
  }

  // - Whether a search names exactly this tag, as `tag:x` or plain `x`, ignoring case.
  // - Used where a search implies a setting (searching r18 turns off "Hide R18").
  function searchNamesTag(query, name) {
    var q = (query || '').trim();
    var want = name.toLowerCase();
    return q.toLowerCase() === want || parseTagQuery(q) === want;
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
      return hasTag(tags, tag);
    }
    var joined = tags.join(' ').toLowerCase();
    return name.toLowerCase().indexOf(q) !== -1 ||
      creator.toLowerCase().indexOf(q) !== -1 ||
      joined.indexOf(q) !== -1;
  }

  var api = {
    parseTagQuery: parseTagQuery,
    workMatchesSearch: workMatchesSearch,
    findTag: findTag,
    hasTag: hasTag,
    searchNamesTag: searchNamesTag,
    tagQuery: tagQuery
  };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.parseTagQuery = parseTagQuery;
    root.workMatchesSearch = workMatchesSearch;
    root.findTag = findTag;
    root.hasTag = hasTag;
    root.searchNamesTag = searchNamesTag;
    root.tagQuery = tagQuery;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
