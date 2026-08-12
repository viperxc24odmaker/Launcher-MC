import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('BlockPilot mount point is missing');

const app = mount(App, { target });

const splash = document.getElementById('splash');
if (splash) {
  requestAnimationFrame(() => {
    setTimeout(() => splash.classList.add('hidden'), 220);
  });
}

export default app;
