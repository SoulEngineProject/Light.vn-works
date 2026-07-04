// - Shared chrome for the game + creator pages: language toggle + share button.
// - Both pages set <html lang> to the server-detected language, which the
//   toggle reads as its fallback when there's no ?lang override.
(function () {
  var toggle = document.getElementById('lang-toggle');
  if (toggle) {
    var p = new URLSearchParams(location.search).get('lang');
    var lang = p === 'ja' || p === 'en'
      ? p
      : (document.documentElement.lang === 'ja' ? 'ja' : 'en');
    toggle.textContent = lang === 'ja' ? 'English' : '日本語';
    toggle.addEventListener('click', function () {
      var url = new URL(location.href);
      url.searchParams.set('lang', lang === 'ja' ? 'en' : 'ja');
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
