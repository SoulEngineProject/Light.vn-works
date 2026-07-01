// Tests for the home-page search matching in public/search.js.
// Each test body has `// given:`, `// when:`, `// then:` sections.
const { test } = require('node:test');
const assert = require('node:assert');
const { parseTagQuery, workMatchesSearch } = require('../../public/search.js');

test('parseTagQuery recognises the tag: prefix', () => {
  // given: queries that start with the tag: prefix, some quoted or padded
  // when: parsing each into its target tag
  // then: the bare, lowercased tag name is returned
  assert.strictEqual(parseTagQuery('tag:r18'), 'r18');
  assert.strictEqual(parseTagQuery('tag:UI'), 'ui');
  assert.strictEqual(parseTagQuery('tag:"Terrace and Ray"'), 'terrace and ray');
  assert.strictEqual(parseTagQuery('tag:  ai  '), 'ai');
});

test('parseTagQuery returns null for non-tag queries', () => {
  // given: empty tag: queries and plain-text queries
  // when: parsing each
  // then: null is returned so the caller falls back to broad search
  assert.strictEqual(parseTagQuery('tag:'), null);
  assert.strictEqual(parseTagQuery('tag:""'), null);
  assert.strictEqual(parseTagQuery('r18'), null);
  assert.strictEqual(parseTagQuery('notag:x'), null);
  assert.strictEqual(parseTagQuery(''), null);
});

test('tag: query matches a tag exactly, not by substring', () => {
  // given: a work tagged UI, and another whose title contains "ui" but lacks the tag
  // when: filtering with the exact tag: query "tag:ui"
  // then: only the genuinely tagged work matches — this is the bug the feature fixes
  assert.strictEqual(
    workMatchesSearch('tag:ui', 'GUI Template', '', ['English', 'UI']),
    true
  );
  assert.strictEqual(
    workMatchesSearch('tag:ui', 'GUI Template', '', ['Puzzle']),
    false
  );
});

test('multi-word tag: query matches exactly', () => {
  // given: a work carrying the multi-word "Terrace and Ray" tag
  // when: filtering with the quoted tag: query
  // then: it matches on the exact multi-word tag
  assert.strictEqual(
    workMatchesSearch('tag:"terrace and ray"', 'Some Work', '', ['Terrace and Ray']),
    true
  );
});

test('plain text stays broad (substring over title/creator/tags)', () => {
  // given: works matched via title, tag, or creator, plus one that matches nothing
  // when: filtering with plain (non-tag:) text
  // then: any substring hit across the fields matches; unrelated text does not
  assert.strictEqual(workMatchesSearch('ui', 'GUI Template', '', []), true);
  assert.strictEqual(workMatchesSearch('ui', 'Whatever', '', ['UI']), true);
  assert.strictEqual(workMatchesSearch('ada', 'Title', 'Ada Lovelace', []), true);
  assert.strictEqual(workMatchesSearch('zzz', 'Title', 'Creator', ['Tag']), false);
});

test('empty query matches everything', () => {
  // given: an empty query and a whitespace-only query
  // when: filtering any work
  // then: the work always matches (no filter applied)
  assert.strictEqual(workMatchesSearch('', 'Anything', 'Anyone', ['Tag']), true);
  assert.strictEqual(workMatchesSearch('   ', 'Anything', 'Anyone', ['Tag']), true);
});

test('tag: prefix is case-insensitive', () => {
  // given: tag: queries with the prefix in mixed or upper case
  // when: parsing them and filtering a UI-tagged work
  // then: the prefix is recognised regardless of case
  assert.strictEqual(parseTagQuery('TAG:ui'), 'ui');
  assert.strictEqual(parseTagQuery('Tag:UI'), 'ui');
  assert.strictEqual(
    workMatchesSearch('TAG:ui', 'Some Work', '', ['UI']),
    true
  );
});

test('tag: matches a tag that is not first in the list', () => {
  // given: a work whose UI tag sits after other tags
  // when: filtering with "tag:ui"
  // then: every tag is scanned, so the match is still found
  assert.strictEqual(
    workMatchesSearch('tag:ui', 'Some Work', '', ['English', 'Puzzle', 'UI']),
    true
  );
});

test('plain text matches a tag substring', () => {
  // given: a work tagged "English"
  // when: filtering with the plain substring "eng"
  // then: broad search matches across the joined tags
  assert.strictEqual(workMatchesSearch('eng', 'Some Work', '', ['English']), true);
});

test('unclosed quote is treated literally (documents behavior)', () => {
  // given: a malformed tag: query with a lone opening quote
  // when: parsing it and filtering a UI-tagged work
  // then: the quote is not stripped, so it matches no clean tag (safe-fails)
  assert.strictEqual(parseTagQuery('tag:"ui'), '"ui');
  assert.strictEqual(workMatchesSearch('tag:"ui', 'GUI', '', ['UI']), false);
});
