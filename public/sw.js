const SW_VERSION = 'v2';
const CACHE_NAME = `relationship-graph-${SW_VERSION}`;

// 缓存策略说明：
// - 本 SW 仅在"生产构建"下注册（见 src/main.tsx：import.meta.env.PROD 判断）。
//   Vite 生产资源文件名均带内容 hash（如 index-D-wlx8Lh.js），cache-first
//   不会造成更新不生效；hash 变化后旧缓存自然失效并被 activate 阶段清理。
// - 开发环境（vite dev，/src/* 模块无 hash）SW 会被主动注销并清空缓存，
//   因此下方"静态资源 cache-first"不会拦截开发资源，联调可即时看到更新。
// - 若后续调整为开发环境也注册 SW，必须先对无 hash 路径（如 /src/*）
//   改用 network-first，否则代码修改将长期被旧缓存覆盖。

// 静态资源预缓存列表（核心资源）
const PRECACHE_URLS = [
  '/',
  '/index.html',
  '/manifest.json',
];

// 安装事件：预缓存核心资源
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(PRECACHE_URLS))
  );
  self.skipWaiting();
});

// 激活事件：清理旧缓存
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((names) =>
      Promise.all(
        names.filter((name) => name !== CACHE_NAME).map((name) => caches.delete(name))
      )
    )
  );
  self.clients.claim();
});

// 请求拦截：静态资源 cache-first，API network-first
self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // API 请求：network-first，失败时返回离线响应
  if (url.pathname.startsWith('/api/')) {
    event.respondWith(
      fetch(request).catch(() =>
        new Response(JSON.stringify({ error: 'offline' }), {
          headers: { 'Content-Type': 'application/json' },
          status: 503,
        })
      )
    );
    return;
  }

  // 导航请求（页面跳转/刷新）：network-first，失败回退缓存的 /index.html
  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request).catch(() => caches.match('/index.html'))
    );
    return;
  }

  // 静态资源：cache-first
  event.respondWith(
    caches.match(request).then((cached) => {
      if (cached) return cached;
      return fetch(request).then((response) => {
        if (response.ok && request.method === 'GET') {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
        }
        return response;
      });
    })
  );
});
