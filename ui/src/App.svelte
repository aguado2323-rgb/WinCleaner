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
    ['tweaks', 'Ajustes'],
    ['restore', 'Restauración'],
  ];

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
    <div class="brand">Win<br />Cleaner</div>

    <nav aria-label="Secciones">
      {#each sections as [id, name], i (id)}
        <button class:active={view === id} onclick={() => go(id)}>
          <span class="n">{String(i + 1).padStart(2, '0')}</span>
          <span class="t">{name}</span>
          {#if id === 'quarantine' && q.batches.length}<span class="c">{q.batches.length}</span>{/if}
        </button>
      {/each}
    </nav>

    <p class="promise label">Sin anuncios<br />Sin telemetría<br />Código abierto · MIT</p>
  </aside>

  <main>
    {#key view}
      <div class="page">
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
  .shell { display: grid; grid-template-columns: 188px 1fr; height: 100vh; }
  aside { display: flex; flex-direction: column; padding: 28px 0 24px 28px; border-right: 1px solid var(--rule); }
  .brand { font-family: var(--display); font-size: 22px; font-weight: 700; line-height: 0.95; letter-spacing: -0.04em; margin-bottom: 44px; }
  nav { display: flex; flex-direction: column; }
  nav button { display: flex; align-items: baseline; gap: 12px; border: 0; background: none; padding: 9px 0; text-align: left; color: var(--ink-3); transition: color 0.12s; }
  nav button:hover { color: var(--ink); }
  nav button .n { font-family: var(--mono); font-size: 11px; width: 18px; }
  nav button .t { font-size: 15px; }
  nav button.active { color: var(--ink); }
  nav button.active .n { color: var(--accent); }
  nav button.active .t { font-weight: 600; box-shadow: inset 0 -2px 0 var(--accent); }
  nav button .c { font-family: var(--mono); font-size: 11px; color: var(--accent); }
  .promise { margin: auto 0 0; line-height: 1.9; font-size: 10px; }
  main { overflow-y: auto; }
  .page { max-width: 780px; padding: 32px 40px 70px; animation: fade 0.18s; }
  .toast { position: fixed; bottom: 20px; left: 216px; display: flex; align-items: center; gap: 18px; background: var(--ink); color: var(--paper); padding: 11px 14px 11px 16px; border-radius: 2px; font-size: 13px; animation: drop 0.18s; }
  .toast button { border: 1px solid var(--paper); background: none; color: var(--paper); padding: 3px 10px; border-radius: 2px; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; }
  .toast button:hover { background: var(--paper); color: var(--ink); }
</style>
