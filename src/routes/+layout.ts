// Disable SSR for Tauri — load functions only run in the WebView,
// which has access to Tauri APIs. SSR would run at build time without them.
export const ssr = false;
export const prerender = false;
