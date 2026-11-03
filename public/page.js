// - Shared chrome for every page: language toggle + share button.
// - The server decides the language (src/lib.rs::detect_lang), writes it to <html lang>,
//   and renders the button label; this only handles the click.
// - Reloads the current URL with ?lang= flipped, so everything else in it (search, r18) is kept.
(function () {
  var toggle = document.getElementById('lang-toggle');
  if (toggle) {
    toggle.addEventListener('click', function () {
      var url = new URL(location.href);
      url.searchParams.set('lang', document.documentElement.lang === 'ja' ? 'en' : 'ja');
      location.href = url.toString();
    });
  }

  var share = document.querySelector('.share-btn');
  if (share) {
    share.addEventListener('click', function () {
      navigator.clipboard.writeText(window.location.href);
      share.textContent = share.dataset.copied;
      setTimeout(function () { share.textContent = share.dataset.share; }, 1500);
    });
  }
})();
