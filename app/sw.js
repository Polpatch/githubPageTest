const CACHE = 'training-v1';

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
  const isIndexHtml = url.pathname.endsWith('/')
    || url.pathname.endsWith('/index.html')
    || url.pathname === self.registration.scope;

  if (isIndexHtml) {
    // Network-first for index.html: always fetch fresh so updated asset
    // hashes (injected by Trunk) are picked up immediately.
    e.respondWith(
      fetch(req)
        .then(res => {
          if (res.ok) caches.open(CACHE).then(c => c.put(req, res.clone()));
          return res;
        })
        .catch(() => caches.match(req))
    );
  } else {
    // Cache-first for everything else: Trunk adds content hashes to asset
    // filenames so they are effectively immutable.
    e.respondWith(
      caches.match(req).then(cached => {
        if (cached) return cached;
        return fetch(req).then(res => {
          if (res.ok) caches.open(CACHE).then(c => c.put(req, res.clone()));
          return res;
        }).catch(() => { /* offline, no cache — let it fail */ });
      })
    );
  }
});
