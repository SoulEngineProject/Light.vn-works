// - Click-to-enlarge screenshot viewer for game pages.
// - Loaded as a plain script in the browser (progressive enhancement) and
//   required by the Node tests for the pure index helper.
(function (root) {
  // - Next index in a wrap-around ring. dir is +1 / -1.
  // - Pure so the Node tests can cover the wrap without a DOM.
  function nextIndex(current, len, dir) {
    if (len <= 0) {
      return 0;
    }
    return (current + dir + len) % len;
  }

  // Browser-only wiring; skipped under Node (no document).
  if (typeof document !== 'undefined' && typeof window !== 'undefined') {
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', init, { once: true });
    } else {
      init();
    }
  }

  function init() {
    // - The game's own screenshots, in reading order: hero → gallery → the
    //   editor-frame preview. Dedup by src so odd-count games (editor reuses
    //   the last gallery image) don't list it twice, while even-count games
    //   (last image lives only in the editor frame) still reach it.
    var nodes = document.querySelectorAll('.hero-image img, .gallery img, .editor-preview');
    var shots = [];
    var seen = {};
    for (var i = 0; i < nodes.length; i++) {
      var src = nodes[i].src; // resolved full URL even while lazy + unfetched
      if (!src || seen[src]) {
        continue;
      }
      seen[src] = true;
      shots.push({ src: src, trigger: nodes[i] });
    }
    if (shots.length === 0) {
      return;
    }

    shots.forEach(function (shot, i) {
      var el = shot.trigger;
      el.setAttribute('role', 'button');
      el.setAttribute('tabindex', '0');
      el.setAttribute('aria-label', 'View screenshot ' + (i + 1));
      el.addEventListener('click', function () { open(i); });
      el.addEventListener('keydown', function (e) {
        if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') {
          e.preventDefault(); // Space would otherwise scroll the page
          open(i);
        }
      });
    });

    // - One overlay reused for every open. Built once, lazily.
    var overlay = null;
    var imgEl, counterEl, prevBtn, nextBtn, closeBtn;
    var current = 0;
    var lastTrigger = null;

    function build() {
      overlay = document.createElement('div');
      overlay.className = 'lightbox';
      overlay.setAttribute('role', 'dialog');
      overlay.setAttribute('aria-modal', 'true');
      overlay.setAttribute('aria-label', 'Screenshot viewer');
      overlay.hidden = true;

      imgEl = document.createElement('img');
      imgEl.className = 'lightbox-img';
      imgEl.alt = '';

      prevBtn = button('lightbox-prev', 'Previous screenshot', '‹');
      nextBtn = button('lightbox-next', 'Next screenshot', '›');
      closeBtn = button('lightbox-close', 'Close', '×');
      counterEl = document.createElement('div');
      counterEl.className = 'lightbox-counter';

      // Clicks on the image/controls must not reach the backdrop-close.
      [imgEl, prevBtn, nextBtn, closeBtn, counterEl].forEach(function (n) {
        n.addEventListener('click', function (e) { e.stopPropagation(); });
      });
      overlay.addEventListener('click', close); // backdrop
      closeBtn.addEventListener('click', close);
      prevBtn.addEventListener('click', function () { show(nextIndex(current, shots.length, -1)); });
      nextBtn.addEventListener('click', function () { show(nextIndex(current, shots.length, 1)); });

      overlay.appendChild(prevBtn);
      overlay.appendChild(imgEl);
      overlay.appendChild(nextBtn);
      overlay.appendChild(closeBtn);
      overlay.appendChild(counterEl);
      document.body.appendChild(overlay);
    }

    function button(cls, label, glyph) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = cls;
      b.setAttribute('aria-label', label);
      b.textContent = glyph;
      return b;
    }

    function show(i) {
      current = i;
      imgEl.src = shots[i].src;
      counterEl.textContent = (i + 1) + ' / ' + shots.length;
    }

    function open(i) {
      if (!overlay) {
        build();
      }
      lastTrigger = shots[i].trigger;
      var multi = shots.length > 1;
      prevBtn.hidden = !multi;
      nextBtn.hidden = !multi;
      counterEl.hidden = !multi;
      show(i);
      overlay.hidden = false;
      document.documentElement.classList.add('lightbox-open');
      document.addEventListener('keydown', onKey);
      closeBtn.focus();
    }

    function close() {
      if (!overlay || overlay.hidden) {
        return;
      }
      overlay.hidden = true;
      document.documentElement.classList.remove('lightbox-open');
      document.removeEventListener('keydown', onKey);
      imgEl.src = '';
      if (lastTrigger && document.contains(lastTrigger)) {
        lastTrigger.focus();
      }
    }

    function onKey(e) {
      switch (e.key) {
        case 'Escape':
          close();
          break;
        case 'ArrowRight':
          if (shots.length > 1) { show(nextIndex(current, shots.length, 1)); }
          break;
        case 'ArrowLeft':
          if (shots.length > 1) { show(nextIndex(current, shots.length, -1)); }
          break;
        case 'Tab':
          trapFocus(e);
          break;
      }
    }

    // - Keep Tab within the overlay, over whichever controls are visible.
    // - Single-image games have only the close button, so focus stays on it.
    function trapFocus(e) {
      var focusable = [prevBtn, nextBtn, closeBtn].filter(function (b) { return !b.hidden; });
      var first = focusable[0];
      var last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  var api = { nextIndex: nextIndex };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.nextIndex = nextIndex;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
