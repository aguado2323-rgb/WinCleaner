<script>
  import { invoke } from '@tauri-apps/api/core';
  import Clean from './Clean.svelte';
  import Quarantine from './Quarantine.svelte';
  import Space from './Space.svelte';
  import Startup from './Startup.svelte';
  import Restore from './Restore.svelte';
  import Debloat from './Debloat.svelte';
  import Uninstall from './Uninstall.svelte';
  import Tweaks from './Tweaks.svelte';
  import Duplicates from './Duplicates.svelte';
  import Services from './Services.svelte';
  import Schedule from './Schedule.svelte';

  const sections = [
    ['clean', 'Limpieza'],
    ['quarantine', 'Cuarentena'],
    ['space', 'Espacio'],
    ['duplicates', 'Duplicados'],
    ['startup', 'Arranque'],
    ['uninstall', 'Desinstalar'],
    ['debloat', 'Preinstaladas'],
    ['tweaks', 'Ajustes de Windows'],
    ['services', 'Servicios'],
    ['schedule', 'Programación'],
    ['restore', 'Restauración'],
  ];

  // Iconos de línea de 24×24, uno por sección.
  const icons = {
    clean: 'M12 3v4M12 17v4M3 12h4M17 12h4M6.3 6.3l2.4 2.4M15.3 15.3l2.4 2.4M17.7 6.3l-2.4 2.4M8.7 15.3l-2.4 2.4',
    quarantine: 'M3 7h18v4H3zM5 11v9h14v-9M10 15h4',
    space: 'M12 3a9 9 0 1 0 9 9h-9zM14.5 3.3A9 9 0 0 1 20.7 9.5h-6.2z',
    duplicates: 'M8 8h11v11H8zM5 15V5h10',
    startup: 'M12 3v9M6.3 6.8a8 8 0 1 0 11.4 0',
    uninstall: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6',
    debloat: 'M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5',
    services: 'M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8',
    schedule: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 7v5l3 2',
    tweaks: 'M4 7h9M17 7h3M4 17h3M11 17h9M15 5v4M9 15v4',
    restore: 'M4 12a8 8 0 1 0 3-6.2M4 4v5h5M12 8v4l3 2',
  };

  // Tema: sigue al sistema hasta que la persona elige; después se recuerda su elección.
  const systemDark = () => matchMedia('(prefers-color-scheme: dark)').matches;
  let theme = $state((() => {
    try {
      const t = localStorage.getItem('wc-theme');
      if (t === 'light' || t === 'dark') return t;
    } catch {
      /* sin almacenamiento: se sigue al sistema */
    }
    return systemDark() ? 'dark' : 'light';
  })());
  $effect(() => {
    document.documentElement.dataset.theme = theme;
  });
  function toggleTheme() {
    theme = theme === 'dark' ? 'light' : 'dark';
    try {
      localStorage.setItem('wc-theme', theme);
    } catch {
      /* no pasa nada: vale para esta sesión */
    }
  }

  let view = $state('clean');
  let q = $state({ path: '', batches: [] });
  let toast = $state(null);
  let toastTimer;

  async function refreshQuarantine() {
    try {
      q = await invoke('quarantine');
    } catch {
      /* sin cuarentena todavía */
    }
  }

  function notify(text, action) {
    clearTimeout(toastTimer);
    toast = { text, action };
    toastTimer = setTimeout(() => (toast = null), action ? 9000 : 4500);
  }

  const go = (v) => {
    view = v;
    if (v === 'quarantine') refreshQuarantine();
  };

  refreshQuarantine();
</script>

<div class="shell">
  <aside>
    <div class="brand">
      <svg class="logo" viewBox="0 0 256 256" aria-hidden="true">
  <rect width="256" height="256" rx="60" fill="#ff4d00"/>
  <path d="M40 92V74a18 18 0 0 1 18-18h108a18 18 0 0 1 18 18v18z" fill="#151512"/>
  <path d="M40 92h144v70a18 18 0 0 1-18 18H58a18 18 0 0 1-18-18z" fill="#f3f1ea"/>
  <circle cx="62" cy="74" r="6" fill="#f3f1ea"/>
  <circle cx="82" cy="74" r="6" fill="#f3f1ea" opacity=".6"/>
  <g transform="translate(156 132) rotate(38) scale(0.98)">
    <path d="M-20 22L20 22L50 108L36 100L26 112L12 101L0 114L-12 101L-26 112L-36 100L-50 108Z" fill="#f3f1ea" stroke="#ff4d00" stroke-width="10" stroke-linejoin="round"/>
    <path d="M-20 22L20 22L50 108L36 100L26 112L12 101L0 114L-12 101L-26 112L-36 100L-50 108Z" fill="#f3f1ea"/>
    <path d="M-9 34L-17 98M0 34V104M9 34L17 98" stroke="#ff4d00" stroke-width="4.5" stroke-linecap="round"/>
    <rect x="-7" y="-112" width="14" height="150" rx="7" fill="#151512" stroke="#ff4d00" stroke-width="8"/>
    <rect x="-25" y="12" width="50" height="22" rx="4" fill="#151512" stroke="#ff4d00" stroke-width="8"/>
    <path d="M-25 23H25" stroke="#f3f1ea" stroke-width="2.5" stroke-dasharray="5 5"/>
  </g>
</svg>
      <span>Win<b>Cleaner</b></span>
    </div>

    <nav aria-label="Secciones">
      {#each sections as [id, name] (id)}
        <button class:active={view === id} onclick={() => go(id)}>
          <svg class="ico" viewBox="0 0 24 24" aria-hidden="true"><path d={icons[id]} /></svg>
          <span class="t">{name}</span>
          {#if id === 'quarantine' && q.batches.length}<span class="c">{q.batches.length}</span>{/if}
        </button>
      {/each}
    </nav>

    <button class="theme" onclick={toggleTheme} aria-label={theme === 'dark' ? 'Cambiar a modo claro' : 'Cambiar a modo oscuro'}>
      <svg class="ico" viewBox="0 0 24 24" aria-hidden="true">
        {#if theme === 'dark'}
          <path d="M12 4V2M12 22v-2M4 12H2M22 12h-2M5.6 5.6 4.2 4.2M19.8 19.8l-1.4-1.4M5.6 18.4l-1.4 1.4M19.8 4.2l-1.4 1.4" /><circle cx="12" cy="12" r="4" />
        {:else}
          <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z" />
        {/if}
      </svg>
      <span>{theme === 'dark' ? 'Modo claro' : 'Modo oscuro'}</span>
    </button>
  </aside>

  <main>
    {#key view}
      <div class="page" class:wide={view === 'uninstall'}>
        {#if view === 'clean'}
          <Clean {notify} {refreshQuarantine} openQuarantine={() => go('quarantine')} />
        {:else if view === 'space'}
          <Space {notify} {refreshQuarantine} openQuarantine={() => go('quarantine')} />
        {:else if view === 'duplicates'}
          <Duplicates {notify} {refreshQuarantine} openQuarantine={() => go('quarantine')} />
        {:else if view === 'startup'}
          <Startup {notify} />
        {:else if view === 'uninstall'}
          <Uninstall {notify} {refreshQuarantine} />
        {:else if view === 'debloat'}
          <Debloat {notify} />
        {:else if view === 'tweaks'}
          <Tweaks {notify} />
        {:else if view === 'services'}
          <Services {notify} />
        {:else if view === 'schedule'}
          <Schedule {notify} {refreshQuarantine} />
        {:else if view === 'restore'}
          <Restore {notify} />
        {:else}
          <Quarantine {q} {notify} {refreshQuarantine} />
        {/if}
      </div>
    {/key}
  </main>

  {#if toast}
    <div class="toast" role="status">
      <span>{toast.text}</span>
      {#if toast.action}<button onclick={() => { toast.action.run(); toast = null; }}>{toast.action.label}</button>{/if}
    </div>
  {/if}
</div>

<style>
  .shell { display: grid; grid-template-columns: 218px 1fr; height: 100vh; }
  aside { display: flex; flex-direction: column; overflow-y: auto; padding: 24px 12px 20px; background: var(--side); color: var(--side-ink); }
  .brand { display: flex; align-items: center; gap: 11px; padding: 0 8px; margin-bottom: 30px; font-family: var(--display); font-size: 20px; letter-spacing: -0.03em; }
  .brand b { font-weight: 700; color: var(--accent); }
  .logo { flex: none; width: 36px; height: 36px; border-radius: 9px; }
  nav { display: flex; flex-direction: column; gap: 2px; }
  nav button { position: relative; display: flex; align-items: center; gap: 12px; border: 0; background: none; padding: 9px 10px; border-radius: 8px; text-align: left; color: var(--side-dim); transition: color 0.12s, background 0.12s; }
  nav button:hover { color: var(--side-ink); background: rgb(255 255 255 / 0.06); }
  nav button .ico { flex: none; width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; }
  nav button .t { flex: 1; font-size: 14px; }
  nav button.active { color: var(--side-ink); background: rgb(255 255 255 / 0.09); }
  nav button.active .t { font-weight: 600; }
  nav button.active .ico { color: var(--accent); }
  nav button.active::before { content: ''; position: absolute; left: -12px; top: 8px; bottom: 8px; width: 4px; border-radius: 0 3px 3px 0; background: var(--accent); }
  nav button .c { font-family: var(--mono); font-size: 11px; color: var(--on-accent); background: var(--accent); border-radius: 9px; padding: 0 6px; }
  .theme { margin-top: auto; display: flex; align-items: center; gap: 12px; border: 1px solid rgb(255 255 255 / 0.14); background: none; border-radius: 8px; padding: 9px 12px; color: var(--side-dim); font-size: 13px; transition: color 0.12s, border-color 0.12s; }
  .theme:hover { color: var(--side-ink); border-color: rgb(255 255 255 / 0.3); }
  .theme .ico { width: 17px; height: 17px; fill: none; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; }
  main { overflow-y: auto; }
  .page { max-width: 780px; padding: 32px 40px 70px; animation: fade 0.18s; }
  .page.wide { max-width: 1060px; }
  .toast { position: fixed; bottom: 20px; left: 244px; display: flex; align-items: center; gap: 18px; background: var(--ink); color: var(--paper); padding: 11px 14px 11px 16px; border-radius: 2px; font-size: 13px; animation: drop 0.18s; }
  .toast button { border: 1px solid var(--paper); background: none; color: var(--paper); padding: 3px 10px; border-radius: 2px; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; }
  .toast button:hover { background: var(--paper); color: var(--ink); }
</style>
