<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { Home, Layers3, Package, Image, Users, Settings, Sun, Moon, Play, Plus, Download, Bell, ChevronDown, Search, Cpu, ShieldCheck, Activity, Database, RotateCcw, Sparkles, Zap, AlertTriangle, Cloud, Check, X, Trash2, FolderOpen, Wand2, Shirt, UserPlus, LogIn, Upload, Star, KeyRound } from 'lucide-svelte';

  type Page = 'Home' | 'Instances' | 'Mods' | 'Cosmetics' | 'Resource Packs' | 'Accounts' | 'Settings';
  let page: Page = 'Home';
  let dark = true;
  let selected = 'Vanilla 1.21.11';
  let launching = false;
  let status = 'Ready to play';
  let toast = '';
  let toastType: 'info' | 'success' | 'error' = 'info';
  let query = '';
  let dropdownOpen = false;

  const nav = [['Home',Home],['Instances',Layers3],['Mods',Package],['Cosmetics',Shirt],['Resource Packs',Image],['Accounts',Users],['Settings',Settings]] as const;
  type BackendInstance = { name:string; path:string; has_game:boolean; version:string; loader:string; loader_version?:string|null };
  type UiInstance = { name:string; version:string; loader:string; kind:string; accent:string };
  let instances: UiInstance[] = [];
  let instancesLoaded = false;
  const accentCycle = ['grass','sunset','sky'];
  function toUiInstance(b: BackendInstance, idx: number): UiInstance {
    const loaderDisplay = b.loader.charAt(0).toUpperCase() + b.loader.slice(1);
    return { name: b.name, version: b.version, loader: loaderDisplay, kind: b.loader==='vanilla' ? 'Survival' : 'Modded', accent: accentCycle[idx % accentCycle.length] };
  }
  async function loadInstances(){
    try{
      const raw: BackendInstance[] = await invoke('list_instances');
      instances = raw.map(toUiInstance);
      instancesLoaded = true;
      if(instances.length && !instances.find(i=>i.name===selected)){ selected = instances[0].name; }
    }catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  const features = [
    ['Multi-instance isolation','Every profile gets its own files, mods, saves and runtime.',Layers3],
    ['Smart dependency resolution','Resolve mod dependencies before you press Play.',Package],
    ['Granular Java control','Pin Java runtimes and custom JVM argument profiles.',Cpu],
    ['Snapshots & rollback','Create a safe restore point before risky changes.',RotateCcw],
    ['Crash auto-analysis','Turn ugly logs into a readable diagnosis and next step.',AlertTriangle],
    ['Secure account vault','Encrypted local profiles with fast account switching.',ShieldCheck]
  ] as const;

  // ---- launch ----
  async function play(){
    launching=true; status='Preparing Minecraft…'; toastType='info';
    try{
      const result = await invoke('launch_instance',{instance:selected});
      status='Minecraft launched! ✓'; toastType='success';
      toast=`${selected} is launching - ${result}`;
    }catch(e){
      status='Launch failed'; toastType='error';
      toast=`Error: ${String(e)}`;
    }finally{
      launching=false; setTimeout(()=>toast='',4500);
    }
  }
  function choose(name:string){selected=name;dropdownOpen=false;page='Home'}
  function notify(message:string, type: 'info'|'success'|'error' = 'info'){toast=message;toastType=type;setTimeout(()=>toast='',2800)}

  // ---- instance management ----
  let newInstanceName = '';
  let newInstanceVersion = '';
  let newInstanceLoader: 'vanilla'|'fabric' = 'vanilla';
  const comingSoonLoaders = ['forge','neoforge','quilt'];

  type McVersion = { id:string; kind:string; release_time:string };
  let mcVersions: McVersion[] = [];
  let mcVersionsLoaded = false;
  let includeSnapshots = false;
  async function loadMcVersions(){
    try{
      mcVersions = await invoke('list_mc_versions', {includeSnapshots});
      mcVersionsLoaded = true;
      if(!newInstanceVersion && mcVersions.length){ newInstanceVersion = mcVersions[0].id; }
    }catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }

  async function createInstance(){
    if(!newInstanceName.trim()){ notify('Give the instance a name first','error'); return; }
    if(!newInstanceVersion){ notify('Pick a Minecraft version first','error'); return; }
    try{
      await invoke('create_instance',{name:newInstanceName, version:newInstanceVersion, loader:newInstanceLoader, loaderVersion:null});
      notify(`Instance "${newInstanceName}" created`,'success');
      newInstanceName='';
      await loadInstances();
    }catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  async function deleteInstance(name:string){
    try{
      await invoke('delete_instance',{name});
      notify(`Deleted "${name}"`,'success');
      await loadInstances();
    }catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  async function openFolder(name:string){
    try{ await invoke('open_instance_folder',{instance:name}); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  async function snapshot(name:string){
    try{ const path = await invoke('snapshot_instance',{name}); notify(`Snapshot saved: ${path}`,'success'); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }

  // ---- mods (Modrinth) ----
  type ModResult = { project_id:string; title:string; description:string; downloads:number; icon_url:string|null };
  let modQuery = '';
  let modResults: ModResult[] = [];
  let modLoading = false;
  async function searchMods(){
    modLoading = true;
    try{
      modResults = await invoke('search_mods',{query:modQuery, loader:instances.find(i=>i.name===selected)?.loader?.toLowerCase()||'fabric', version:instances.find(i=>i.name===selected)?.version||'1.21.11'});
    }catch(e){ notify(`Search failed: ${String(e)}`,'error'); modResults=[]; }
    finally{ modLoading=false; }
  }
  async function installMod(projectId:string){
    try{
      const result = await invoke('install_mod',{instance:selected, projectId, gameVersion:instances.find(i=>i.name===selected)?.version||'1.21.11', loader:instances.find(i=>i.name===selected)?.loader?.toLowerCase()||'fabric'});
      notify(String(result),'success');
    }catch(e){ notify(`Install failed: ${String(e)}`,'error'); }
  }

  // ---- cosmetics ----
  type Cosmetic = { id:string; name:string; kind:string; source:string };
  let cosmeticsList: Cosmetic[] = [];
  let selectedCosmetic = '';
  let importPath = '';
  let importKind: 'cape'|'wings' = 'cape';
  async function loadCosmetics(){
    try{ cosmeticsList = await invoke('list_cosmetics'); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  async function applyCosmetic(id:string){
    try{ await invoke('apply_cosmetic',{instance:selected, cosmeticId:id}); selectedCosmetic=id; notify(`Applied to ${selected}`,'success'); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  async function importCosmeticFile(){
    if(!importPath.trim()){ notify('Paste the full PNG file path first','error'); return; }
    try{
      await invoke('import_cosmetic',{kind:importKind, filePath:importPath});
      notify('Cosmetic imported','success'); importPath=''; loadCosmetics();
    }catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }

  // ---- settings / java ----
  let javaRuntimes: {path:string; version:string}[] = [];
  let ramMb = 4096;
  let jvmArgs = '';
  async function detectJava(){
    try{ javaRuntimes = await invoke('list_java_runtimes'); notify(`Found ${javaRuntimes.length} Java runtime(s)`,'success'); }
    catch(e){ notify(String(e),'error'); javaRuntimes=[]; }
  }

  // ---- accounts (offline + ely.by) ----
  type Account = { id:string; kind:'offline'|'elyby'|'microsoft'; username:string; uuid:string; access_token?:string|null; skin_path?:string|null; cape_id?:string|null; active:boolean };
  let accountsList: Account[] = [];
  let accountsLoaded = false;
  let authMode: 'offline'|'elyby'|'microsoft' = 'offline';
  let offlineUsername = '';
  let elybyUsername = '';
  let elybyPassword = '';
  let authBusy = false;
  let skinCanvasEl: HTMLCanvasElement | undefined;
  const capeOptions = [
    { id:'', name:'None' },
    { id:'elyby-classic', name:'Classic ely.by' },
    { id:'elyby-mikuia', name:'Mikuia' },
    { id:'elyby-owlby', name:'Owlby' },
  ];

  async function loadAccounts(){
    try{ accountsList = await invoke('list_accounts'); accountsLoaded = true; drawActiveSkin(); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  $: activeAccount = accountsList.find(a=>a.active);

  async function createOfflineAccount(){
    if(!offlineUsername.trim()){ notify('Enter an offline username','error'); return; }
    authBusy = true;
    try{
      await invoke('add_offline_account',{username:offlineUsername});
      notify(`Offline profile "${offlineUsername}" added`,'success');
      offlineUsername = '';
      await loadAccounts();
    }catch(e){ notify(String(e),'error'); }
    finally{ authBusy = false; }
  }
  async function loginElyBy(){
    if(!elybyUsername.trim() || !elybyPassword){ notify('Enter your ely.by username and password','error'); return; }
    authBusy = true;
    try{
      const acc:Account = await invoke('add_elyby_account',{username:elybyUsername, password:elybyPassword});
      notify(`Signed in as ${acc.username} (ely.by)`,'success');
      elybyUsername=''; elybyPassword='';
      await loadAccounts();
    }catch(e){ notify(String(e),'error'); }
    finally{ authBusy = false; }
  }

  // ---- microsoft ----
  let msClientId = '';
  let msClientIdSaved = false;
  let msSigningIn = false;
  async function loadMsSettings(){
    try{
      const settings:{ms_client_id?:string|null} = await invoke('get_launcher_settings');
      if(settings.ms_client_id){ msClientId = settings.ms_client_id; msClientIdSaved = true; }
    }catch(e){ /* first run, no settings file yet - not an error */ }
  }
  async function saveMsClientId(){
    if(!msClientId.trim()){ notify('Paste your Azure app Client ID first','error'); return; }
    try{ await invoke('set_ms_client_id',{clientId:msClientId}); msClientIdSaved=true; notify('Client ID saved','success'); }
    catch(e){ notify(String(e),'error'); }
  }
  async function signInMicrosoft(){
    msSigningIn = true;
    try{
      const acc:Account = await invoke('start_microsoft_login');
      notify(`Signed in as ${acc.username} (Microsoft)`,'success');
      await loadAccounts();
    }catch(e){ notify(String(e),'error'); }
    finally{ msSigningIn = false; }
  }
  async function switchAccount(id:string){
    try{ await invoke('set_active_account',{id}); await loadAccounts(); notify('Active account switched','success'); }
    catch(e){ notify(String(e),'error'); }
  }
  async function removeAccountFn(id:string){
    try{ await invoke('remove_account',{id}); await loadAccounts(); notify('Account removed','success'); }
    catch(e){ notify(String(e),'error'); }
  }
  let skinPathInput = '';
  async function importSkinFn(id:string){
    if(!skinPathInput.trim()){ notify('Paste the full path to a 64x64 skin PNG','error'); return; }
    try{
      await invoke('import_skin',{id, filePath:skinPathInput});
      notify('Skin imported','success'); skinPathInput=''; await loadAccounts();
    }catch(e){ notify(String(e),'error'); }
  }
  async function pickCape(id:string, capeId:string){
    try{ await invoke('set_cape',{id, capeId: capeId || null}); await loadAccounts(); notify('Cape updated','success'); }
    catch(e){ notify(String(e),'error'); }
  }
  function drawActiveSkin(){
    if(!skinCanvasEl) return;
    const ctx = skinCanvasEl.getContext('2d');
    if(!ctx) return;
    ctx.clearRect(0,0,skinCanvasEl.width,skinCanvasEl.height);
    ctx.imageSmoothingEnabled = false;
    const acc = accountsList.find(a=>a.active);
    const hasSkin = !!acc?.skin_path;
    ctx.fillStyle = hasSkin ? '#5b8a63' : '#3a5d42';
    ctx.fillRect(24,4,32,32);   // head block
    ctx.fillStyle = hasSkin ? '#4a7752' : '#2c4a33';
    ctx.fillRect(30,10,6,6); ctx.fillRect(44,10,6,6); // eyes
    ctx.fillStyle = hasSkin ? '#6b9a73' : '#345f3b';
    ctx.fillRect(24,40,32,48); // torso block
  }
  $: if(page==='Accounts' && !accountsLoaded){ loadAccounts(); loadMsSettings(); }
  $: if(activeAccount && skinCanvasEl){ drawActiveSkin(); }

  // ---- resource packs ----
  let resourcePacksList: string[] = [];
  let rpImportPath = '';
  async function loadResourcePacks(){
    try{ resourcePacksList = await invoke('list_resource_packs', {instance:selected}); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  async function importResourcePackFn(){
    if(!rpImportPath.trim()){ notify('Paste the full path to a .zip resource pack','error'); return; }
    try{ await invoke('import_resource_pack',{instance:selected, filePath:rpImportPath}); notify('Resource pack imported','success'); rpImportPath=''; await loadResourcePacks(); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  async function removeResourcePackFn(filename:string){
    try{ await invoke('remove_resource_pack',{instance:selected, filename}); notify('Removed','success'); await loadResourcePacks(); }
    catch(e){ notify(`Error: ${String(e)}`,'error'); }
  }
  $: if(page==='Resource Packs' && selected){ loadResourcePacks(); }

  onMount(() => {
    loadAccounts();
    loadInstances();
    loadMcVersions();
    let unlistenFn: (() => void) | undefined;
    listen<{instance:string; stage:string; detail:string}>('launch-progress', (e) => {
      status = e.payload.detail;
    }).then(f => { unlistenFn = f; });
    return () => { unlistenFn?.(); };
  });
  $: if(page==='Mods' && modResults.length===0 && !modLoading) { /* lazy, user triggers search */ }
  $: if(page==='Cosmetics' && cosmeticsList.length===0) { loadCosmetics(); }
</script>

<svelte:head><title>BlockPilot</title></svelte:head>

<div class:light={!dark} class="app-shell">
  <aside class="sidebar">
    <div class="brand"><div class="brand-mark">B</div><div><strong>BLOCKPILOT</strong><span>MINECRAFT LAUNCHER</span></div></div>
    <div class="nav-label">WORKSPACE</div>
    <nav>{#each nav as item}{@const Icon=item[1]}<button class:active={page===item[0]} onclick={()=>page=item[0]}><Icon size={18}/><span>{item[0]}</span></button>{/each}</nav>
    <div class="sidebar-bottom">
      <div class="sync-card"><div class="sync-icon"><Cloud size={17}/></div><div><b>Cloud sync</b><small>Everything up to date</small></div><span class="online"></span></div>
      <div class="account-mini"><div class="avatar">{(activeAccount?.username||'S').charAt(0).toUpperCase()}</div><div><b>{activeAccount?.username||'No account'}</b><small>{activeAccount ? (activeAccount.kind==='microsoft'?'Microsoft account':activeAccount.kind==='elyby'?'ely.by account':'Offline profile') : 'Add one in Accounts'}</small></div><span class="dot"></span></div>
    </div>
  </aside>
  <main class="main">
    <header class="topbar">
      <div class="crumb"><span>Workspace</span><b>/</b><strong>{page}</strong></div>
      <div class="top-actions">
        <label class="search"><Search size={16}/><input bind:value={query} placeholder="Search everything…" /></label>
        <button class="icon-btn" onclick={()=>notify('No new notifications')}><Bell size={18}/><i></i></button>
        <button class="icon-btn" onclick={()=>dark=!dark}>{#if dark}<Sun size={18}/>{:else}<Moon size={18}/>{/if}</button>
        <div class="profile-pill"><div class="avatar small">{(activeAccount?.username||'?').charAt(0).toUpperCase()}</div><span>{activeAccount?.username||'No account'}</span><ChevronDown size={14}/></div>
      </div>
    </header>

    {#if page==='Home'}
      <section class="hero">
        <div class="hero-copy">
          <div class="eyebrow"><span class="live-dot"></span> READY · BLOCKPILOT 0.1</div>
          <h1>Your worlds.<br/><em>Your way.</em></h1>
          <p>A fast, private Minecraft workspace built around isolated instances, smart modding and zero-friction launching.</p>
          <div class="hero-actions">
            <button class="play" disabled={launching} onclick={play}><Play size={18} fill="currentColor"/>{launching?'LAUNCHING…':'PLAY'}</button>
            <div class="dropdown-wrapper">
              <button class="instance-select" onclick={()=>dropdownOpen=!dropdownOpen}>
                <div class="cube"></div>
                <span><b>{selected}</b><small>{instances.find(i=>i.name===selected)?.version} · {instances.find(i=>i.name===selected)?.loader}</small></span>
                <ChevronDown size={16} style={dropdownOpen?'transform: rotate(180deg)':'transform: rotate(0deg)'}/>
              </button>
              {#if dropdownOpen}
                <div class="instance-dropdown">
                  {#each instances as inst}
                    <button class="dropdown-item" class:active={selected===inst.name} onclick={()=>choose(inst.name)}>
                      <div class="item-icon"></div>
                      <div><b>{inst.name}</b><small>{inst.version} · {inst.kind}</small></div>
                      {#if selected===inst.name}<Check size={14}/>{/if}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        </div>
        <div class="hero-art"><div class="sun"></div><div class="mountain m1"></div><div class="mountain m2"></div><div class="mountain m3"></div><div class="pixel-ground"></div><div class="hero-fog"></div></div>
      </section>

      <section class="section-head"><div><span class="eyebrow">YOUR WORKSPACE</span><h2>Recent instances</h2></div><button class="ghost" onclick={()=>page='Instances'}>View all <span>→</span></button></section>
      <section class="instance-grid">
        {#each instances as inst}
          <button class="instance-card" class:selected-card={selected===inst.name} onclick={()=>choose(inst.name)}>
            <div class="instance-cover {inst.accent}"><div class="terrain"></div><span>{inst.loader}</span><div class="mini-play"><Play size={13} fill="currentColor"/></div></div>
            <div class="card-body"><div><b>{inst.name}</b><small>{inst.version} · {inst.kind}</small></div><span class="state">READY</span></div>
          </button>
        {/each}
        <button class="new-card" onclick={()=>page='Instances'}><div><Plus size={24}/></div><b>Create instance</b><small>Vanilla · Fabric · Forge</small></button>
      </section>

      <section class="section-head lower"><div><span class="eyebrow">BUILT IN</span><h2>Power without the clutter</h2></div></section>
      <section class="feature-grid">{#each features as f}{@const Icon=f[2]}<div class="feature-card"><div class="feature-icon"><Icon size={18}/></div><div><b>{f[0]}</b><p>{f[1]}</p></div></div>{/each}</section>

    {:else if page==='Instances'}
      <section class="page-head">
        <div><span class="eyebrow">INSTANCES</span><h1>Instances</h1><p>Manage your Minecraft workspace from one place.</p></div>
      </section>
      <div class="management-grid">
        <div class="panel wide">
          <div class="panel-title">Create new instance</div>
          <div class="create-instance-form">
            <label class="search inner"><input placeholder="Instance name…" bind:value={newInstanceName}/></label>
            <select class="select-input" bind:value={newInstanceVersion}>
              {#if !mcVersionsLoaded}<option value="">Loading versions…</option>{/if}
              {#each mcVersions as v}<option value={v.id}>{v.id}{v.kind!=='release'?` (${v.kind})`:''}</option>{/each}
            </select>
            <select class="select-input" bind:value={newInstanceLoader}>
              <option value="vanilla">Vanilla</option>
              <option value="fabric">Fabric</option>
              {#each comingSoonLoaders as l}<option value={l} disabled>{l.charAt(0).toUpperCase()+l.slice(1)} (soon)</option>{/each}
            </select>
            <label class="snapshot-toggle"><input type="checkbox" bind:checked={includeSnapshots} onchange={loadMcVersions}/> Show snapshots</label>
            <button class="row-action solid" onclick={createInstance}><Plus size={14}/> CREATE</button>
          </div>
          <div class="panel-title" style="margin-top:20px">Your instances</div>
          <div class="rows">
            {#each instances as inst}
              <div class="row">
                <div class="row-icon"><Layers3 size={17}/></div>
                <div class="row-main"><b>{inst.name}</b><span>{inst.version} · {inst.loader} · {inst.kind}</span></div>
                <span class="state">READY</span>
                <button class="row-action" onclick={()=>openFolder(inst.name)}><FolderOpen size={13}/></button>
                <button class="row-action" onclick={()=>snapshot(inst.name)}><RotateCcw size={13}/></button>
                <button class="row-action danger" onclick={()=>deleteInstance(inst.name)}><Trash2 size={13}/></button>
                <button class="row-action" onclick={()=>choose(inst.name)}>OPEN</button>
              </div>
            {/each}
            {#if instancesLoaded && instances.length===0}<div class="empty-state"><Layers3 size={28}/><p>No instances yet - create one above to get started</p></div>{/if}
          </div>
        </div>
        <div class="panel">
          <div class="panel-title">System health</div>
          <div class="metric"><Activity size={16}/><span>CPU</span><b>14%</b></div><div class="meter"><i style="width:14%"></i></div>
          <div class="metric"><Database size={16}/><span>Memory</span><b>3.2 / 8 GB</b></div><div class="meter"><i style="width:40%"></i></div>
          <div class="metric"><Zap size={16}/><span>Launcher</span><b>{status}</b></div>
        </div>
      </div>

    {:else if page==='Mods'}
      <section class="page-head">
        <div><span class="eyebrow">MODS</span><h1>Mods</h1><p>Search Modrinth and install straight into <b>{selected}</b>.</p></div>
      </section>
      <div class="panel wide">
        <div class="panel-head">
          <label class="search inner" style="flex:1"><Search size={15}/><input placeholder="Search Modrinth (e.g. sodium, lithium, jei)…" bind:value={modQuery} onkeydown={(e)=>e.key==='Enter' && searchMods()}/></label>
          <button class="row-action solid" onclick={searchMods}>{modLoading?'SEARCHING…':'SEARCH'}</button>
        </div>
        <div class="rows">
          {#each modResults as m}
            <div class="row">
              <div class="row-icon"><Package size={17}/></div>
              <div class="row-main"><b>{m.title}</b><span>{m.description.slice(0,90)}{m.description.length>90?'…':''} · {m.downloads.toLocaleString()} downloads</span></div>
              <button class="row-action solid" onclick={()=>installMod(m.project_id)}><Download size={13}/> INSTALL</button>
            </div>
          {/each}
          {#if modResults.length===0 && !modLoading}<div class="empty-state"><Package size={28}/><p>Search Modrinth to browse mods for {selected}</p></div>{/if}
        </div>
      </div>

    {:else if page==='Cosmetics'}
      <section class="page-head">
        <div><span class="eyebrow">COSMETICS</span><h1>Cosmetics</h1><p>Capes and wings, applied per-instance. Free, offline-first.</p></div>
      </section>
      <div class="management-grid">
        <div class="panel wide">
          <div class="cosmetics-grid">
            {#each cosmeticsList as c}
              <button class="cosmetic-card" class:active={selectedCosmetic===c.id} onclick={()=>applyCosmetic(c.id)}>
                <div class="cosmetic-preview {c.kind}"><Wand2 size={22}/></div>
                <b>{c.name}</b><small>{c.kind} · {c.source}</small>
                {#if selectedCosmetic===c.id}<span class="applied-badge"><Check size={11}/> APPLIED</span>{/if}
              </button>
            {/each}
          </div>
        </div>
        <div class="panel">
          <div class="panel-title">Import cosmetic</div>
          <div class="metric" style="margin-top:14px"><span>Type</span></div>
          <div class="import-kind">
            <button class:active={importKind==='cape'} onclick={()=>importKind='cape'}>Cape</button>
            <button class:active={importKind==='wings'} onclick={()=>importKind='wings'}>Wings</button>
          </div>
          <label class="search inner" style="margin-top:12px;width:100%"><input placeholder="Full path to PNG…" bind:value={importPath}/></label>
          <button class="row-action solid" style="margin-top:10px;width:100%;justify-content:center" onclick={importCosmeticFile}><Plus size={14}/> IMPORT</button>
        </div>
      </div>

    {:else if page==='Settings'}
      <section class="page-head"><div><span class="eyebrow">SETTINGS</span><h1>Settings</h1><p>Java runtime, memory and launch behavior.</p></div></section>
      <div class="management-grid">
        <div class="panel wide">
          <div class="panel-title">Java runtime</div>
          <button class="row-action solid" style="margin-top:12px" onclick={detectJava}><Cpu size={14}/> DETECT JAVA</button>
          <div class="rows">
            {#each javaRuntimes as j}<div class="row"><div class="row-icon"><Cpu size={17}/></div><div class="row-main"><b>{j.path}</b><span>{j.version}</span></div></div>{/each}
            {#if javaRuntimes.length===0}<div class="empty-state"><Cpu size={28}/><p>Click detect to scan for Java 21 on PATH</p></div>{/if}
          </div>
          <div class="panel-title" style="margin-top:20px">Memory allocation</div>
          <input type="range" min="1024" max="16384" step="512" bind:value={ramMb} style="width:100%;margin-top:12px"/>
          <small style="color:var(--muted)">{(ramMb/1024).toFixed(1)} GB</small>
          <div class="panel-title" style="margin-top:20px">Custom JVM arguments</div>
          <label class="search inner" style="margin-top:10px;width:100%"><input placeholder="-XX:+UseG1GC …" bind:value={jvmArgs}/></label>
        </div>
        <div class="panel">
          <div class="panel-title">Launcher</div>
          <div class="metric"><Zap size={16}/><span>Version</span><b>0.1.0</b></div>
          <div class="metric"><ShieldCheck size={16}/><span>Auth</span><b>Offline</b></div>
        </div>
      </div>

    {:else if page==='Accounts'}
      <section class="page-head"><div><span class="eyebrow">ACCOUNTS</span><h1>Accounts</h1><p>Offline profiles for quick single-player, or ely.by for real skins and capes across every instance.</p></div></section>
      <div class="management-grid">
        <div class="panel wide">
          <div class="account-tabs">
            <button class:active={authMode==='offline'} onclick={()=>authMode='offline'}><UserPlus size={14}/> Offline profile</button>
            <button class:active={authMode==='elyby'} onclick={()=>authMode='elyby'}><LogIn size={14}/> ely.by login</button>
            <button class:active={authMode==='microsoft'} onclick={()=>authMode='microsoft'}><ShieldCheck size={14}/> Microsoft</button>
          </div>
          {#if authMode==='offline'}
            <div class="auth-form">
              <p class="auth-hint">No login needed - just a name. Works fully offline, single-player only.</p>
              <label class="search inner wide"><input placeholder="Choose a username…" bind:value={offlineUsername} onkeydown={(e)=>e.key==='Enter' && createOfflineAccount()}/></label>
              <button class="row-action solid" onclick={createOfflineAccount} disabled={authBusy}>{authBusy?'ADDING…':'ADD PROFILE'}</button>
            </div>
          {:else if authMode==='elyby'}
            <div class="auth-form">
              <p class="auth-hint"><KeyRound size={12}/> Signs in via ely.by's Yggdrasil endpoint. Your real skin and cape render in-game through authlib-injector.</p>
              <label class="search inner wide"><input placeholder="ely.by username or email…" bind:value={elybyUsername}/></label>
              <label class="search inner wide"><input type="password" placeholder="Password…" bind:value={elybyPassword} onkeydown={(e)=>e.key==='Enter' && loginElyBy()}/></label>
              <button class="row-action solid" onclick={loginElyBy} disabled={authBusy}>{authBusy?'SIGNING IN…':'SIGN IN'}</button>
            </div>
          {:else if authMode==='microsoft'}
            <div class="auth-form">
              <p class="auth-hint"><ShieldCheck size={12}/> Real Xbox/Minecraft sign-in. Needs a free Azure app Client ID once - opens your browser, signs in, comes right back.</p>
              {#if !msClientIdSaved}
                <label class="search inner wide"><input placeholder="Azure app Client ID…" bind:value={msClientId}/></label>
                <button class="row-action solid" onclick={saveMsClientId}>SAVE CLIENT ID</button>
              {:else}
                <button class="row-action solid" onclick={signInMicrosoft} disabled={msSigningIn}>{msSigningIn?'WAITING FOR BROWSER…':'SIGN IN WITH MICROSOFT'}</button>
                <button class="row-action" style="align-self:flex-start" onclick={()=>msClientIdSaved=false}>Change client ID</button>
              {/if}
            </div>
          {/if}

          <div class="panel-title" style="margin-top:24px">Saved accounts</div>
          <div class="rows">
            {#each accountsList as acc}
              <div class="row">
                <div class="avatar small" style="min-width:26px">{acc.username.charAt(0).toUpperCase()}</div>
                <div class="row-main"><b>{acc.username}</b><span>{acc.kind==='elyby'?'ely.by account':acc.kind==='microsoft'?'Microsoft account':'Offline profile'} {acc.active?'· active':''}</span></div>
                {#if acc.active}<span class="state"><Star size={11}/> ACTIVE</span>{:else}<button class="row-action" onclick={()=>switchAccount(acc.id)}>USE</button>{/if}
                <button class="row-action danger" onclick={()=>removeAccountFn(acc.id)}><Trash2 size={13}/></button>
              </div>
            {/each}
            {#if accountsList.length===0}<div class="empty-state"><Users size={28}/><p>No accounts yet - add an offline profile or sign in with ely.by above</p></div>{/if}
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">Skin & cape</div>
          {#if activeAccount}
            <div class="skin-preview-wrap"><canvas bind:this={skinCanvasEl} width="80" height="88"></canvas></div>
            <small style="color:var(--muted);display:block;text-align:center;margin-top:6px">{activeAccount.username} {activeAccount.skin_path?'· custom skin imported':'· default skin'}</small>
            <label class="search inner" style="margin-top:14px;width:100%"><input placeholder="Full path to skin PNG…" bind:value={skinPathInput}/></label>
            <button class="row-action solid" style="margin-top:8px;width:100%;justify-content:center" onclick={()=>importSkinFn(activeAccount.id)}><Upload size={13}/> IMPORT SKIN</button>
            {#if activeAccount.kind==='elyby'}
              <div class="panel-title" style="margin-top:18px">Cape</div>
              <div class="cape-options">
                {#each capeOptions as c}
                  <button class:active={(activeAccount.cape_id||'')===c.id} onclick={()=>pickCape(activeAccount.id, c.id)}>{c.name}</button>
                {/each}
              </div>
            {:else if activeAccount.kind==='microsoft'}
              <small style="color:var(--muted);display:block;margin-top:12px">Microsoft accounts use your official Mojang skin/cape automatically - manage those at minecraft.net.</small>
            {:else}
              <small style="color:var(--muted);display:block;margin-top:12px">Capes need an ely.by or Microsoft account to render in-game.</small>
            {/if}
          {:else}
            <div class="empty-state"><Shirt size={28}/><p>Add an account to manage skins and capes</p></div>
          {/if}
        </div>
      </div>

    {:else}
      <section class="page-head">
        <div><span class="eyebrow">RESOURCE PACKS</span><h1>Resource Packs</h1><p>Applied to <b>{selected}</b>. Drop in .zip packs and they show here.</p></div>
      </section>
      <div class="management-grid">
        <div class="panel wide">
          <div class="panel-head">
            <div><b>Import a pack</b><small>Paste the full path to a .zip resource pack file.</small></div>
            <label class="search inner wide"><input placeholder="Full path to .zip…" bind:value={rpImportPath}/></label>
            <button class="row-action solid" onclick={importResourcePackFn}><Upload size={13}/> IMPORT</button>
          </div>
          <div class="rows">
            {#each resourcePacksList as pack}
              <div class="row">
                <div class="row-icon"><Image size={17}/></div>
                <div class="row-main"><b>{pack}</b><span>Applied to {selected}</span></div>
                <button class="row-action danger" onclick={()=>removeResourcePackFn(pack)}><Trash2 size={13}/></button>
              </div>
            {/each}
            {#if resourcePacksList.length===0}<div class="empty-state"><Image size={28}/><p>No resource packs yet for {selected}</p></div>{/if}
          </div>
        </div>
        <div class="panel">
          <div class="panel-title">System health</div>
          <div class="metric"><Activity size={16}/><span>CPU</span><b>14%</b></div><div class="meter"><i style="width:14%"></i></div>
          <div class="metric"><Database size={16}/><span>Memory</span><b>3.2 / 8 GB</b></div><div class="meter"><i style="width:40%"></i></div>
          <div class="metric"><Zap size={16}/><span>Launcher</span><b>{status}</b></div>
        </div>
      </div>
    {/if}

    <footer><span>BlockPilot</span><span>·</span><span>Secure local-first launcher</span><span class="footer-right">Java <b>21</b> · Sync <b>ON</b></span></footer>
  </main>
</div>

{#if toast}
  <div class="toast" class:success={toastType==='success'} class:error={toastType==='error'} class:info={toastType==='info'}>
    {#if toastType==='success'}<Check size={16}/>{:else if toastType==='error'}<X size={16}/>{:else}<Sparkles size={16}/>{/if}
    {toast}
  </div>
{/if}
