// Tests for the home-page year-open cascade in public/view.js.
// Each test body has `// given:`, `// when:`, `// then:` sections.
const { test } = require('node:test');
const assert = require('node:assert');
const { computeOpenYearFlags, ribbonEntries } = require('../../public/view.js');

test('a query opens every group', () => {
  // given: groups of various sizes and an active query
  const counts = [10, 5, 8];

  // when: computing open flags with hasQuery = true
  const flags = computeOpenYearFlags(counts, true);

  // then: every group is open regardless of size
  assert.deepStrictEqual(flags, [true, true, true]);
});

test('a full newest group opens only itself', () => {
  // given: a first group that alone reaches the 6-work threshold
  const counts = [10, 5, 5];

  // when: computing open flags with no query
  const flags = computeOpenYearFlags(counts, false);

  // then: only the first group opens
  assert.deepStrictEqual(flags, [true, false, false]);
});

test('short groups cascade open until 6 works are shown', () => {
  // given: small groups whose running total stays under 6 for a while
  const counts = [3, 2, 4, 5];

  // when: computing open flags with no query
  const flags = computeOpenYearFlags(counts, false);

  // then:
  // - prior works before each group: 0, 3, 5, 9
  // - open while prior < 6, so the first three open and the fourth does not
  assert.deepStrictEqual(flags, [true, true, true, false]);
});

test('threshold is strict (exactly 6 stops the cascade)', () => {
  // given: a first group of exactly 6 works
  const counts = [6, 1];

  // when: computing open flags with no query
  const flags = computeOpenYearFlags(counts, false);

  // then: the second group stays closed (prior 6 is not < 6)
  assert.deepStrictEqual(flags, [true, false]);
});

test('the first group always opens', () => {
  // given: a single small group
  const counts = [4];

  // when: computing open flags with no query
  const flags = computeOpenYearFlags(counts, false);

  // then: it opens (prior works is 0)
  assert.deepStrictEqual(flags, [true]);
});

test('no groups yields no flags', () => {
  // given: an empty list of groups
  const counts = [];

  // when: computing open flags
  const flags = computeOpenYearFlags(counts, false);

  // then: an empty array is returned
  assert.deepStrictEqual(flags, []);
});

test('a custom threshold is honoured', () => {
  // given: groups and a minWorks of 3
  const counts = [2, 2, 2];

  // when: computing open flags with the custom threshold
  const flags = computeOpenYearFlags(counts, false, 3);

  // then: open while prior < 3 — first two open (prior 0, 2), third closed (prior 4)
  assert.deepStrictEqual(flags, [true, true, false]);
});

// - A two-year tree: a plain work, an R18 work (tagged in upper case), and a work with no thumbnail.
function ribbonTree() {
  return {
    children: [
      {
        name: '2025',
        children: [
          { title: 'Plain', href: '/works/2025/Plain', thumbnail: '/thumb/a/card', thumbnail_ribbon: '/thumb/a/ribbon', meta: { tags: [] } },
          { title: 'Adult', href: '/works/2025/Adult', thumbnail: '/thumb/b/card', meta: { tags: ['R18'] } }
        ]
      },
      {
        name: '2024',
        children: [
          { title: 'NoThumb', href: '/works/2024/NoThumb', meta: {} }
        ]
      }
    ]
  };
}

test('ribbon leaves out R18 works while Hide R18 is on', () => {
  // given: a tree with an R18 work tagged "R18"
  const data = ribbonTree();

  // when: building ribbon entries with Hide R18 on
  const entries = ribbonEntries(data, true);

  // then: only the plain work remains, whatever the tag's case
  assert.deepStrictEqual(entries.map(e => e.title), ['Plain']);
});

test('ribbon includes R18 works while Hide R18 is off', () => {
  // given: the same tree
  const data = ribbonTree();

  // when: building ribbon entries with Hide R18 off
  const entries = ribbonEntries(data, false);

  // then:
  // - both works with thumbnails appear, in tree order
  // - the work without a thumbnail never does
  assert.deepStrictEqual(entries.map(e => e.title), ['Plain', 'Adult']);
});

test('ribbon entries use the ribbon-size thumbnail and the server href', () => {
  // given: one work with a ribbon thumbnail, one with only a card thumbnail
  const data = ribbonTree();

  // when: building ribbon entries with Hide R18 off
  const [plain, adult] = ribbonEntries(data, false);

  // then:
  // - the smaller ribbon URL wins when present, else the card thumbnail
  // - the server-built href is used as-is
  assert.strictEqual(plain.url, '/thumb/a/ribbon');
  assert.strictEqual(adult.url, '/thumb/b/card');
  assert.strictEqual(plain.href, '/works/2025/Plain');
});
