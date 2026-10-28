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

  const sections = [
    ['clean', 'Limpieza'],
    ['quarantine', 'Cuarentena'],
    ['space', 'Espacio'],
    ['startup', 'Arranque'],
    ['uninstall', 'Desinstalar'],
    ['debloat', 'Preinstaladas'],
    ['tweaks', 'Ajustes de Windows'],
    ['restore', 'Restauración'],
  ];

  // Iconos de línea de 24×24, uno por sección.
  const icons = {
    clean: 'M12 3v4M12 17v4M3 12h4M17 12h4M6.3 6.3l2.4 2.4M15.3 15.3l2.4 2.4M17.7 6.3l-2.4 2.4M8.7 15.3l-2.4 2.4',
    quarantine: 'M3 7h18v4H3zM5 11v9h14v-9M10 15h4',
    space: 'M12 3a9 9 0 1 0 9 9h-9zM14.5 3.3A9 9 0 0 1 20.7 9.5h-6.2z',
    startup: 'M12 3v9M6.3 6.8a8 8 0 1 0 11.4 0',
    uninstall: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6',
    debloat: 'M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5',
    tweaks: 'M4 7h9M17 7h3M4 17h3M11 17h9M15 5v4M9 15v4',
    restore: 'M4 12a8 8 0 1 0 3-6.2M4 4v5h5M12 8v4l3 2',
  };

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
        <rect width="256" height="256" rx="60" fill="#ff4d00" />
        <path d="M40 92V74a18 18 0 0 1 18-18h108a18 18 0 0 1 18 18v18z" fill="#151512" />
        <path d="M40 92h144v70a18 18 0 0 1-18 18H58a18 18 0 0 1-18-18z" fill="#f3f1ea" />
        <circle cx="62" cy="74" r="6" fill="#f3f1ea" />
        <circle cx="82" cy="74" r="6" fill="#f3f1ea" opacity=".6" />
        <g transform="translate(150 144) scale(1.14) rotate(40) translate(-176 -150)">
          <rect x="167" y="34" width="18" height="84" rx="9" fill="#151512" stroke="#ff4d00" stroke-width="10" />
          <path d="M158 112h36l-3 34h-30z" fill="#151512" stroke="#ff4d00" stroke-width="10" stroke-linejoin="round" />
          <path d="M155 142h42c6 22 4 48-21 78-25-30-27-56-21-78z" fill="#f3f1ea" stroke="#151512" stroke-width="10" stroke-linejoin="round" />
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
  </aside>

  <main>
    {#key view}
      <div class="page" class:wide={view === 'uninstall'}>
        {#if view === 'clean'}
          <Clean {notify} {refreshQuarantine} openQuarantine={() => go('quarantine')} />
        {:else if view === 'space'}
          <Space {notify} {refreshQuarantine} openQuarantine={() => go('quarantine')} />
        {:else if view === 'startup'}
          <Startup {notify} />
        {:else if view === 'uninstall'}
          <Uninstall {notify} {refreshQuarantine} />
        {:else if view === 'debloat'}
          <Debloat {notify} />
        {:else if view === 'tweaks'}
          <Tweaks {notify} />
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
  aside { display: flex; flex-direction: column; padding: 24px 12px 20px; background: var(--side); color: var(--side-ink); }
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
  main { overflow-y: auto; }
  .page { max-width: 780px; padding: 32px 40px 70px; animation: fade 0.18s; }
  .page.wide { max-width: 1060px; }
  .toast { position: fixed; bottom: 20px; left: 244px; display: flex; align-items: center; gap: 18px; background: var(--ink); color: var(--paper); padding: 11px 14px 11px 16px; border-radius: 2px; font-size: 13px; animation: drop 0.18s; }
  .toast button { border: 1px solid var(--paper); background: none; color: var(--paper); padding: 3px 10px; border-radius: 2px; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; }
  .toast button:hover { background: var(--paper); color: var(--ink); }
</style>
