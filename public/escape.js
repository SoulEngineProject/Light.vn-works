// - Escaping helpers for strings interpolated into innerHTML / inline styles.
// - Loaded as a plain script in the browser (exposes globals) and required by the Node tests.
(function (root) {
  // - Escape the five HTML specials for element and attribute contexts.
  // - Global regexes: string patterns replace only the first occurrence.
  // - '&' first, so the entities added by later replacements aren't re-escaped.
  function escapeHtml(str) {
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  // - Percent-encode chars that could terminate a CSS url('…') token or its
  //   wrapping style attribute. Mirrors escape_css_url in src/lib.rs.
  // - HTML entities don't survive into CSS (decoded before the CSS engine
  //   runs), so the URL itself has to carry the encoding.
  // - '%' stays as-is: source URLs are often already percent-encoded.
  function escapeCssUrl(url) {
    return String(url)
      .replace(/\\/g, '%5C')
      .replace(/'/g, '%27')
      .replace(/"/g, '%22')
      .replace(/\(/g, '%28')
      .replace(/\)/g, '%29')
      .replace(/ /g, '%20');
  }

  var api = { escapeHtml: escapeHtml, escapeCssUrl: escapeCssUrl };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.escapeHtml = escapeHtml;
    root.escapeCssUrl = escapeCssUrl;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
