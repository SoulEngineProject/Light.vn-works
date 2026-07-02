// Tests for the escaping helpers in public/escape.js.
// Each test body has `// given:`, `// when:`, `// then:` sections.
const { test } = require('node:test');
const assert = require('node:assert');
const { escapeHtml, escapeCssUrl } = require('../../public/escape.js');

test('escapeHtml escapes all five specials', () => {
  // given: a string with every HTML special character
  // when: escaping
  // then: each becomes its entity
  assert.strictEqual(escapeHtml('&<>"\''), '&amp;&lt;&gt;&quot;&#39;');
});

test('escapeHtml escapes repeated characters', () => {
  // given: repeated quotes — a first-occurrence-only replace misses the second
  // when: escaping
  // then: every occurrence is escaped
  assert.strictEqual(escapeHtml('""'), '&quot;&quot;');
  assert.strictEqual(escapeHtml("a'b'c"), 'a&#39;b&#39;c');
});

test('escapeHtml neutralizes an attribute breakout payload', () => {
  // given: a filename-style payload that closes an aria-label and adds a handler
  const payload = '" onmouseover="alert(1)';

  // when: escaping for a double-quoted attribute
  const out = escapeHtml(payload);

  // then: no raw quote remains, so the attribute cannot be closed
  assert.strictEqual(out.indexOf('"'), -1);
});

test('escapeHtml escapes & first, so its own output is not re-escaped', () => {
  // given: text that already looks like an entity
  // when: escaping
  // then: the & is escaped exactly once
  assert.strictEqual(escapeHtml('&lt;'), '&amp;lt;');
});

test('escapeHtml passes normal text through', () => {
  // given: plain English and Japanese text
  // when: escaping
  // then: unchanged
  assert.strictEqual(escapeHtml('My World 1章'), 'My World 1章');
});

test('escapeCssUrl leaves proxy URLs unchanged', () => {
  // given: the /thumb proxy URL shape
  // when: encoding for a CSS url('…') context
  // then: unchanged
  assert.strictEqual(escapeCssUrl('/thumb/abc-123/card'), '/thumb/abc-123/card');
});

test('escapeCssUrl does not double-encode existing %XX sequences', () => {
  // given: a URL that already carries percent-encoding
  // when: encoding
  // then: the % passes through untouched
  assert.strictEqual(escapeCssUrl('https://example.com/a%27b'), 'https://example.com/a%27b');
});

test('escapeCssUrl encodes every breakout character, repeated', () => {
  // given: a payload that would close url('…') and start a new declaration
  const payload = "x') no-repeat; background:url('evil\\";

  // when: encoding
  const out = escapeCssUrl(payload);

  // then: no quotes, parens, backslashes, or spaces survive
  assert.strictEqual(/['"()\\ ]/.test(out), false);
});
