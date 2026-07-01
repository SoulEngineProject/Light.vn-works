// Tests for the home-page year-open cascade in public/view.js.
// Each test body has `// given:`, `// when:`, `// then:` sections.
const { test } = require('node:test');
const assert = require('node:assert');
const { computeOpenYearFlags } = require('../../public/view.js');

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
