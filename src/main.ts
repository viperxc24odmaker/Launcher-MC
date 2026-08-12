import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';

function hideSplash() {
  const splash = document.getElementById('splash');
  if (splash) splash.classList.add('hidden');
}

function showFatalError(message: string) {
  hideSplash();
  const el = document.createElement('div');
  el.style.cssText = 'position:fixed;inset:0;z-index:10000;background:#080b09;color:#e9eee9;font-family:sans-serif;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;padding:24px;text-align:center';
  el.innerHTML = `<div style="font-size:15px;font-weight:700;letter-spacing:.08em">BLOCKPILOT FAILED TO START</div>
    <div style="font-size:12px;color:#87928a;max-width:520px">${message}</div>
    <div style="font-size:10px;color:#5f6a62">Press F12 or right-click and choose Inspect to see the full console error.</div>`;
  document.body.appendChild(el);
}

// safety net: never let the splash get stuck forever, no matter what fails below
const failsafe = setTimeout(() => showFatalError('The app took too long to start. Something failed silently during startup.'), 8000);

window.addEventListener('error', (e) => {
  clearTimeout(failsafe);
  showFatalError(e.message || 'An unexpected error occurred.');
});
window.addEventListener('unhandledrejection', (e) => {
  clearTimeout(failsafe);
  showFatalError(String(e.reason) || 'An unexpected error occurred.');
});

try {
  const target = document.getElementById('app');
  if (!target) throw new Error('BlockPilot mount point is missing from index.html');

  const app = mount(App, { target });

  clearTimeout(failsafe);
  requestAnimationFrame(() => setTimeout(hideSplash, 220));

  // @ts-ignore
  window.__blockpilot = app;
} catch (err) {
  clearTimeout(failsafe);
  showFatalError(err instanceof Error ? err.message : String(err));
}

