// Tests for the wrap-around index helper in public/lightbox.js.
// Each test body has `// given:`, `// when:`, `// then:` sections.
const { test } = require('node:test');
const assert = require('node:assert');
const { nextIndex } = require('../../public/lightbox.js');

test('next wraps past the last index to the first', () => {
  // given: sitting on the last of five images, moving forward
  // when: computing the next index
  // then: it wraps to 0
  assert.strictEqual(nextIndex(4, 5, 1), 0);
});

test('prev wraps past the first index to the last', () => {
  // given: sitting on the first image, moving backward
  // when: computing the next index
  // then: it wraps to the last
  assert.strictEqual(nextIndex(0, 5, -1), 4);
});

test('mid-range moves by one in each direction', () => {
  // given: a middle position
  // when: stepping forward and back
  // then: the neighbours are returned
  assert.strictEqual(nextIndex(2, 5, 1), 3);
  assert.strictEqual(nextIndex(2, 5, -1), 1);
});

test('a single image always stays on index 0', () => {
  // given: one image, either direction
  // when: computing the next index
  // then: it never leaves 0
  assert.strictEqual(nextIndex(0, 1, 1), 0);
  assert.strictEqual(nextIndex(0, 1, -1), 0);
});

test('empty set returns 0 rather than dividing by zero', () => {
  // given: no images
  // when: computing the next index
  // then: 0, not NaN
  assert.strictEqual(nextIndex(0, 0, 1), 0);
});
