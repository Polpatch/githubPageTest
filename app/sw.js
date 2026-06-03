const CACHE = 'training-v2';

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', e => {
  e.waitUntil(
    caches.keys()
      .then(keys => Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', e => {
  const req = e.request;
  if (req.method !== 'GET') return;
  if (!req.url.startsWith(self.location.origin)) return;

  const url = new URL(req.url);

  const isNetworkFirst =
    // index.html — always fresh (Trunk injects new asset hashes on each build)
    url.pathname.endsWith('/')           ||
    url.pathname.endsWith('/index.html') ||
    url.pathname === self.registration.scope ||
    // schede JSON — can be updated in the repo without a full rebuild
    url.pathname.includes('/schede/');

  if (isNetworkFirst) {
    // Network-first: try network, cache result, fall back to cache if offline
    e.respondWith(
      fetch(req)
        .then(res => {
          if (res.ok) caches.open(CACHE).then(c => c.put(req, res.clone()));
          return res;
        })
        .catch(() => caches.match(req))
    );
  } else {
    // Cache-first: Trunk content-hashes all other assets (JS/WASM/CSS/icons)
    // so they are effectively immutable — serve from cache instantly.
    e.respondWith(
      caches.match(req).then(cached => {
        if (cached) return cached;
        return fetch(req).then(res => {
          if (res.ok) caches.open(CACHE).then(c => c.put(req, res.clone()));
          return res;
        }).catch(() => { /* offline, no cache */ });
      })
    );
  }
});
