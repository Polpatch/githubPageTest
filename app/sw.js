const CACHE = 'training-v1';

// Cache all resources on first load (cache-as-you-go strategy).
// After one full online visit the app works entirely offline.
self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', e => {
  // Delete old caches from previous versions
  e.waitUntil(
    caches.keys().then(keys =>
      Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', e => {
  const req = e.request;
  // Only handle GET requests for same-origin resources
  if (req.method !== 'GET') return;
  if (!req.url.startsWith(self.location.origin)) return;

  e.respondWith(
    caches.open(CACHE).then(cache =>
      cache.match(req).then(cached => {
        const network = fetch(req).then(response => {
          if (response.ok) cache.put(req, response.clone());
          return response;
        }).catch(() => cached); // offline: fall back to cache
        return cached || network;
      })
    )
  );
});
