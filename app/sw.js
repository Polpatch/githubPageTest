const CACHE = 'training-v2';

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', e => {
  e.waitUntil(
    caches.keys()
      .then(keys => Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

function cacheResponse(req, res) {
  // Clone synchronously — must happen before any async boundary
  // or the response body will already be consumed.
  const clone = res.clone();
  caches.open(CACHE).then(c => c.put(req, clone));
}

self.addEventListener('fetch', e => {
  const req = e.request;
  if (req.method !== 'GET') return;
  if (!req.url.startsWith(self.location.origin)) return;

  const url = new URL(req.url);

  const isNetworkFirst =
    url.pathname.endsWith('/')           ||
    url.pathname.endsWith('/index.html') ||
    url.pathname === self.registration.scope ||
    url.pathname.includes('/schede/');

  if (isNetworkFirst) {
    e.respondWith(
      fetch(req)
        .then(res => {
          if (res.ok) cacheResponse(req, res);
          return res;
        })
        .catch(() => caches.match(req))
    );
  } else {
    e.respondWith(
      caches.match(req).then(cached => {
        if (cached) return cached;
        return fetch(req).then(res => {
          if (res.ok) cacheResponse(req, res);
          return res;
        }).catch(() => { /* offline, no cache */ });
      })
    );
  }
});

// ── Recovery timer notifications ─────────────────────────────────────────────
let scheduledTimeout = null;

self.addEventListener('message', ({ data }) => {
  if (data.action === 'schedule') {
    clearTimeout(scheduledTimeout);
    const delay = data.fire_at - Date.now();
    if (delay <= 0) fireNotification();
    else scheduledTimeout = setTimeout(fireNotification, delay);
  }
  if (data.action === 'cancel') {
    clearTimeout(scheduledTimeout);
  }
});

function fireNotification() {
  self.registration.showNotification('Recupero completato', {
    body: 'Puoi riprendere con il prossimo set',
    tag: 'recovery-timer',
    renotify: false,
  });
}

self.addEventListener('notificationclick', e => {
  e.notification.close();
  e.waitUntil(
    clients.matchAll({ type: 'window', includeUncontrolled: true })
      .then(list => {
        const match = list.find(c => c.url.startsWith(self.registration.scope));
        return match ? match.focus() : clients.openWindow('./');
      })
  );
});
