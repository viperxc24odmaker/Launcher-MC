<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { Home, Layers3, Package, Image, Globe2, Server, Users, Settings, Sun, Moon, Play, Plus, Download, Bell, ChevronDown, Search, Cpu, ShieldCheck, Activity, Database, RotateCcw, Sparkles, FolderOpen, Zap, AlertTriangle } from 'lucide-svelte';

  type Page = 'Home' | 'Instances' | 'Mods' | 'Resource Packs' | 'Worlds' | 'Servers' | 'Accounts' | 'Settings';
  let page: Page = 'Home';
  let dark = true;
  let selected = 'Vanilla 1.21.11';
  let launching = false;
  let status = 'Ready to play';
  let toast = '';
  let query = '';

  const nav = [
    ['Home', Home], ['Instances', Layers3], ['Mods', Package], ['Resource Packs', Image],
    ['Worlds', Globe2], ['Servers', Server], ['Accounts', Users], ['Settings', Settings]
  ] as const;

  const instances = [
    { name: 'Vanilla 1.21.11', version: '1.21.11', loader: 'Vanilla', kind: 'Survival', accent: 'grass' },
    { name: 'Fabric Lab', version: '1.21.11', loader: 'Fabric', kind: 'Modded', accent: 'sunset' },
    { name: 'Performance', version: '1.21.10', loader: 'Fabric', kind: 'Sodium + Vulkan', accent: 'sky' }
  ];

  const features = [
    ['Multi-instance isolation', 'Every profile gets its own files, mods, saves and runtime.', Layers3],
    ['Smart dependency resolution', 'Resolve mod dependencies before you press Play.', Package],
    ['Granular Java control', 'Pin Java runtimes and custom JVM argument profiles.', Cpu],
    ['Snapshots & rollback', 'Create a safe restore point before risky changes.', RotateCcw],
    ['Crash auto-analysis', 'Turn ugly logs into a readable diagnosis and next step.', AlertTriangle],
    ['Secure account vault', 'Encrypted local profiles with fast account switching.', ShieldCheck]
  ];

  async function play() {
    launching = true; status = 'Preparing Minecraft…';
    try {
      await invoke('launch_instance', { instance: selected });
      status = 'Minecraft launched';
      toast = `${selected} is launching`;
    } catch (e) {
      status = 'Launch needs setup';
      toast = String(e);
    } finally {
      launching = false;
      setTimeout(() => toast = '', 3500);
    }
  }

  function choose(name: string) { selected = name; page = 'Home'; }
  function notify(message: string) { toast = message; setTimeout(() => toast = '', 2600); }
</script>

<svelte:head><title>BlockPilot</title></svelte:head>
<div class:light={!dark} class="app-shell">
  <aside class="sidebar">
    <div class="brand"><div class="brand-mark">B</div><div><strong>BLOCKPILOT</strong><span>MINECRAFT LAUNCHER</span></div></div>
    <div class="nav-label">WORKSPACE</div>
    <nav>
      {#each nav as item}
        {@const Icon = item[1]}
        <button class:active={page === item[0]} onclick={() => page = item[0]}><Icon size={18}/><span>{item[0]}</span></button>
      {/each}
    </nav>
    <div class="sidebar-bottom">
      <div class="sync-card"><div class="sync-icon"><CloudIcon /></div><div><b>Cloud sync</b><small>Everything up to date</small></div><span class="online"></span></div>
      <div class="account-mini"><div class="avatar">S</div><div><b>Steve</b><small>Microsoft account</small></div><span class="dot"></span></div>
    </div>
  </aside>

  <main class="main">
    <header class="topbar">
      <div class="crumb"><span>Workspace</span><b>/</b><strong>{page}</strong></div>
      <div class="top-actions">
        <label class="search"><Search size={16}/><input bind:value={query} placeholder="Search everything…" /></label>
        <button class="icon-btn" onclick={() => notify('No new notifications')}><Bell size={18}/><i></i></button>
        <button class="icon-btn" onclick={() => dark = !dark}>{#if dark}<Sun size={18}/>{:else}<Moon size={18}/>{/if}</button>
        <div class="profile-pill"><div class="avatar small">S</div><span>Steve</span><ChevronDown size={14}/></div>
      </div>
    </header>

    {#if page === 'Home'}
      <section class="hero">
        <div class="hero-copy"><div class="eyebrow"><span class="live-dot"></span> READY · BLOCKPILOT 0.1</div><h1>Your worlds.<br/><em>Your way.</em></h1><p>A fast, private Minecraft workspace built around isolated instances, smart modding and zero-friction launching.</p><div class="hero-actions"><button class="play" disabled={launching} onclick={play}><Play size={18} fill="currentColor"/>{launching ? 'LAUNCHING…' : 'PLAY'} </button><button class="instance-select"><div class="cube"></div><span><b>{selected}</b><small>1.21.11 · Vanilla</small></span><ChevronDown size={16}/></button></div></div>
        <div class="hero-art"><div class="sun"></div><div class="mountain m1"></div><div class="mountain m2"></div><div class="mountain m3"></div><div class="pixel-ground"></div><div class="hero-fog"></div></div>
      </section>

      <section class="section-head"><div><span class="eyebrow">YOUR WORKSPACE</span><h2>Recent instances</h2></div><button class="ghost" onclick={() => page='Instances'}>View all <span>→</span></button></section>
      <section class="instance-grid">
        {#each instances as inst}
          <button class="instance-card" class:selected-card={selected === inst.name} onclick={() => choose(inst.name)}>
            <div class="instance-cover {inst.accent}"><div class="terrain"></div><span>{inst.loader}</span><div class="mini-play"><Play size={13} fill="currentColor"/></div></div>
            <div class="card-body"><div><b>{inst.name}</b><small>{inst.version} · {inst.kind}</small></div><span class="state">READY</span></div>
          </button>
        {/each}
        <button class="new-card" onclick={() => page='Instances'}><div><Plus size={24}/></div><b>Create instance</b><small>Vanilla · Fabric · Forge</small></button>
      </section>

      <section class="section-head lower"><div><span class="eyebrow">BUILT IN</span><h2>Power without the clutter</h2></div></section>
      <section class="feature-grid">
        {#each features as f}
          {@const Icon = f[2]}
          <div class="feature-card"><div class="feature-icon"><Icon size={18}/></div><div><b>{f[0]}</b><p>{f[1]}</p></div></div>
        {/each}
      </section>
    {:else}
      <section class="page-head"><div><span class="eyebrow">{page.toUpperCase()}</span><h1>{page}</h1><p>Manage your Minecraft workspace from one place.</p></div><button class="play compact" onclick={() => notify('Coming online with the launcher core') }><Plus size={17}/> NEW</button></section>
      <div class="management-grid">
        <div class="panel wide"><div class="panel-head"><div><b>{page} manager</b><small>Search, filter and control every part of your workspace.</small></div><label class="search inner"><Search size={15}/><input placeholder="Filter…" bind:value={query}/></label></div>
          <div class="rows">
            {#each instances as inst}<div class="row"><div class="row-icon"><Layers3 size={17}/></div><div class="row-main"><b>{inst.name}</b><span>{inst.version} · {inst.loader} · {inst.kind}</span></div><span class="state">READY</span><button class="row-action" onclick={() => choose(inst.name)}>OPEN</button></div>{/each}
          </div>
        </div>
        <div class="panel"><div class="panel-title">System health</div><div class="metric"><Activity size={16}/><span>CPU</span><b>14%</b></div><div class="meter"><i style="width:14%"></i></div><div class="metric"><Database size={16}/><span>Memory</span><b>3.2 / 8 GB</b></div><div class="meter"><i style="width:40%"></i></div><div class="metric"><Zap size={16}/><span>Launcher</span><b>Optimal</b></div></div>
      </div>
    {/if}
    <footer><span>BlockPilot</span><span>·</span><span>Secure local-first launcher</span><span class="footer-right">Java <b>21</b> · Sync <b>ON</b></span></footer>
  </main>
</div>
{#if toast}<div class="toast"><Sparkles size={16}/>{toast}</div>{/if}

<script lang="ts">
  import { Cloud as CloudIcon } from 'lucide-svelte';
</script>
