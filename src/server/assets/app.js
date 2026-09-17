(function () {
  'use strict';

  var BOOK = document.body.dataset.bookId;

  function post(url, body) {
    return fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body || {})
    }).then(function (r) {
      if (!r.ok) { throw new Error('request failed: ' + r.status); }
      return r;
    });
  }

  function updateMarkedCount() {
    var el = document.getElementById('marked-count');
    if (!el) return;
    var n = 0;
    document.querySelectorAll('.clipping').forEach(function (card) {
      var box = card.querySelector('.mark');
      if (box && box.checked && !card.classList.contains('ignored')) n++;
    });
    el.textContent = n;
  }

  document.querySelectorAll('.clipping').forEach(function (card) {
    var id = card.dataset.id;

    var mark = card.querySelector('.mark');
    if (mark) mark.addEventListener('change', function (e) {
      post('/api/clipping/' + BOOK + '/' + id + '/mark', { marked: e.target.checked })
        .then(updateMarkedCount)
        .catch(function () { e.target.checked = !e.target.checked; });
    });

    var ignoreBtn = card.querySelector('.ignore-btn');
    if (ignoreBtn) ignoreBtn.addEventListener('click', function () {
      var ignored = !card.classList.contains('ignored');
      post('/api/clipping/' + BOOK + '/' + id + '/ignore', { ignored: ignored })
        .then(function () {
          card.classList.toggle('ignored', ignored);
          ignoreBtn.textContent = ignored ? 'unignore' : 'ignore';
          updateMarkedCount();
        })
        .catch(function () {});
    });

    var ta = card.querySelector('.annotation');
    if (ta) {
      var saved = ta.value;
      ta.addEventListener('blur', function () {
        if (ta.value === saved) return;
        post('/api/clipping/' + BOOK + '/' + id + '/annotate', { text: ta.value })
          .then(function () { saved = ta.value; })
          .catch(function () {});
      });
    }

    var select = card.querySelector('.cluster-select');
    if (select) select.addEventListener('change', function (e) {
      var cid = e.target.value;
      if (!cid) return;
      post('/api/cluster/' + BOOK + '/' + cid + '/assign', { clipping_id: id, member: true })
        .then(function () { window.location.reload(); })
        .catch(function () { e.target.value = ''; });
    });

    var unassign = card.querySelector('.unassign');
    if (unassign) unassign.addEventListener('click', function () {
      var section = card.closest('section.cluster');
      if (!section) return;
      var cid = section.dataset.clusterId;
      post('/api/cluster/' + BOOK + '/' + cid + '/assign', { clipping_id: id, member: false })
        .then(function () { window.location.reload(); })
        .catch(function () {});
    });
  });

  // Drag-and-drop reordering within a cluster. Dropping a card from
  // another cluster pulls it into this one.
  document.querySelectorAll('section.cluster[data-cluster-id]').forEach(function (section) {
    var cid = section.dataset.clusterId;
    section.querySelectorAll('.clipping').forEach(function (card) {
      card.addEventListener('dragstart', function (e) {
        e.dataTransfer.setData('text/plain', card.dataset.id);
        e.dataTransfer.effectAllowed = 'move';
      });
      card.addEventListener('dragover', function (e) { e.preventDefault(); });
      card.addEventListener('drop', function (e) {
        e.preventDefault();
        var dragged = e.dataTransfer.getData('text/plain');
        if (!dragged || dragged === card.dataset.id) return;
        var ids = Array.prototype.map.call(
          section.querySelectorAll('.clipping'),
          function (c) { return c.dataset.id; }
        );
        var from = ids.indexOf(dragged);
        if (from === -1) {
          ids.unshift(dragged);
        } else {
          ids.splice(from, 1);
        }
        var to = ids.indexOf(card.dataset.id);
        ids.splice(to === -1 ? ids.length : to, 0, dragged);
        post('/api/cluster/' + BOOK + '/' + cid + '/order', { clipping_ids: ids })
          .then(function () { window.location.reload(); })
          .catch(function () {});
      });
    });
  });

  var toggle = document.getElementById('toggle-ignored');
  if (toggle) toggle.addEventListener('click', function () {
    var showing = document.body.classList.toggle('show-ignored');
    toggle.textContent = showing ? 'hide ignored' : 'show ignored';
  });

  var create = document.getElementById('create-cluster');
  if (create) create.addEventListener('click', function () {
    var name = window.prompt('Cluster name');
    if (!name || !name.trim()) return;
    post('/api/book/' + BOOK + '/cluster', { name: name.trim() })
      .then(function () { window.location.reload(); })
      .catch(function () {});
  });

  document.querySelectorAll('.rename-cluster').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var name = window.prompt('Rename cluster');
      if (!name || !name.trim()) return;
      post('/api/cluster/' + BOOK + '/' + btn.dataset.id + '/rename', { name: name.trim() })
        .then(function () { window.location.reload(); })
        .catch(function () {});
    });
  });

  document.querySelectorAll('.delete-cluster').forEach(function (btn) {
    btn.addEventListener('click', function () {
      if (!window.confirm('Delete this cluster? Its clippings return to Unclustered.')) return;
      fetch('/api/cluster/' + BOOK + '/' + btn.dataset.id, { method: 'DELETE' })
        .then(function (r) {
          if (!r.ok) throw new Error('request failed');
          window.location.reload();
        })
        .catch(function () {});
    });
  });

  var clear = document.getElementById('clear-marks');
  if (clear) clear.addEventListener('click', function () {
    if (!window.confirm('Clear all export marks?')) return;
    post('/api/marks/clear', {})
      .then(function () { window.location.reload(); })
      .catch(function () {});
  });

  updateMarkedCount();
})();
