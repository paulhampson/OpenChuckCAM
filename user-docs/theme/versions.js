// SPDX-License-Identifier: AGPL-3.0-or-later
// The version switch and the note on an older release (decision 0400 section V).
// Without JavaScript, or when versions.json cannot be read (a page opened from disk), the header
// keeps its plain "Version" link to versions.html, and the footer still names the version.
(function () {
  var meta = document.querySelector('meta[name="docs-version"]');
  var link = document.querySelector('.version-link');
  if (!meta || !link || typeof path_to_root === 'undefined' || !window.fetch) return;
  var current = meta.content;
  var site = path_to_root + '../';
  // This page's path inside its version folder: the last (depth + 1) parts of the address.
  var depth = (path_to_root.match(/\.\.\//g) || []).length;
  var parts = location.pathname.split('/');
  var page = parts.slice(parts.length - depth - 1).join('/') || 'index.html';

  function open(path) {
    // Go to the same page in another version, or to that version's start if the page is not there.
    var target = site + path + page;
    fetch(target, { method: 'HEAD' }).then(function (r) {
      location.href = r.ok ? target : site + path;
    }, function () { location.href = site + path; });
  }

  fetch(site + 'versions.json').then(function (r) { return r.ok ? r.json() : null; }).then(function (data) {
    if (!data || !data.versions) return;
    var label = document.createElement('label');
    label.htmlFor = 'version-select';
    label.textContent = 'Version';
    var select = document.createElement('select');
    select.id = 'version-select';
    data.versions.forEach(function (v) {
      var o = document.createElement('option');
      o.value = v.path;
      o.textContent = v.version === 'latest' ? 'latest (nightly build)' : v.version;
      if (v.version === current) o.selected = true;
      select.appendChild(o);
    });
    select.addEventListener('change', function () { open(select.value); });
    var span = link.parentNode;
    span.replaceChild(select, link);
    span.insertBefore(label, select);

    var newest = data.newest_release;
    if (current !== 'latest' && newest && newest !== current) {
      var note = document.createElement('div');
      note.className = 'older-release';
      note.setAttribute('role', 'note');
      note.appendChild(document.createTextNode('This is the documentation for ' + current + '. The newest release is ' + newest + ': '));
      var a = document.createElement('a');
      a.href = site + 'v' + newest + '/';
      a.textContent = 'this page in ' + newest;
      a.addEventListener('click', function (e) { e.preventDefault(); open('v' + newest + '/'); });
      note.appendChild(a);
      note.appendChild(document.createTextNode('.'));
      document.body.insertBefore(note, document.body.firstChild.nextSibling);
    }
  }, function () {});
})();
