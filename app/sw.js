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

