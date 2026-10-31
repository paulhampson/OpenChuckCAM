// SPDX-License-Identifier: AGPL-3.0-or-later
// A minimal search box over Zola's elasticlunr index (Zola writes the index; the page script is ours).
(function () {
  var input = document.getElementById('search'), list = document.getElementById('search-results');
  if (!input || !window.searchIndex || !window.elasticlunr) return;
  var index = elasticlunr.Index.load(window.searchIndex);
  input.addEventListener('input', function () {
    var q = input.value.trim(); list.innerHTML = '';
    if (q.length < 2) { list.classList.add('hidden'); return; }
    var hits = index.search(q, { bool: 'AND', expand: true, fields: { title: { boost: 2 }, body: { boost: 1 } } }).slice(0, 10);
    hits.forEach(function (h) {
      var d = index.documentStore.getDoc(h.ref), li = document.createElement('li'), a = document.createElement('a');
      a.href = h.ref; a.textContent = d.title || h.ref; li.appendChild(a); list.appendChild(li);
    });
    if (!hits.length) { var li = document.createElement('li'); li.textContent = 'No pages match.'; list.appendChild(li); }
    list.classList.remove('hidden');
  });
})();
