<script>
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import Clean from './Clean.svelte';
  import Quarantine from './Quarantine.svelte';
  import Space from './Space.svelte';
  import Startup from './Startup.svelte';

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

  refreshQuarantine();
</script>

<div class="shell">
  <aside>
    <div class="brand">
      <span class="mark"><Icon name="check" size={16} /></span>
      <span>WinCleaner</span>
    </div>

    <nav>
      <button class:active={view === 'clean'} onclick={() => (view = 'clean')}>
        <Icon name="broom" /> Limpieza
      </button>
      <button class:active={view === 'quarantine'} onclick={() => { view = 'quarantine'; refreshQuarantine(); }}>
        <Icon name="shield" /> Cuarentena
        {#if q.batches.length}<span class="badge">{q.batches.length}</span>{/if}
      </button>
      <button class:active={view === 'space'} onclick={() => (view = 'space')}>
        <Icon name="pie" /> Espacio
      </button>
      <button class:active={view === 'startup'} onclick={() => (view = 'startup')}>
        <Icon name="power" /> Arranque
      </button>
    </nav>

    <p class="promise"><Icon name="lock" size={14} /> Sin anuncios · sin telemetría<br />Código abierto (MIT)</p>
  </aside>

  <main>
    {#key view}
      <div class="page">
        {#if view === 'clean'}
          <Clean {notify} {refreshQuarantine} openQuarantine={() => { view = 'quarantine'; refreshQuarantine(); }} />
        {:else if view === 'space'}
          <Space {notify} />
        {:else if view === 'startup'}
          <Startup {notify} />
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
  .shell { display: grid; grid-template-columns: 232px 1fr; height: 100vh; background:
      radial-gradient(900px 500px at 85% -10%, var(--glow), transparent 60%), var(--canvas); }
  aside { display: flex; flex-direction: column; padding: 22px 14px; border-right: 1px solid var(--hairline); background: color-mix(in srgb, var(--surface) 70%, transparent); }
  .brand { display: flex; align-items: center; gap: 10px; font-weight: 600; letter-spacing: 0.2px; padding: 0 10px 22px; }
  .mark { display: grid; place-items: center; width: 26px; height: 26px; border-radius: 8px; color: #06121a; background: linear-gradient(135deg, var(--blue), var(--green)); }
  nav { display: flex; flex-direction: column; gap: 2px; }
  nav button { display: flex; align-items: center; gap: 11px; border: 0; background: none; padding: 9px 10px; border-radius: 9px; color: var(--mute); cursor: pointer; text-align: left; transition: background 0.15s, color 0.15s; }
  nav button:hover:not(:disabled) { background: var(--hairline); color: var(--ink); }
  nav button.active { background: var(--hairline); color: var(--ink); box-shadow: inset 2px 0 0 var(--blue); }
  nav button:disabled { opacity: 0.5; cursor: default; }
  .badge { margin-left: auto; font-size: 11px; font-weight: 600; background: var(--blue); color: #06121a; border-radius: 99px; padding: 1px 7px; }
  .promise { margin: auto 10px 0; font-size: 11.5px; line-height: 1.6; color: var(--ash); }
  .promise :global(svg) { vertical-align: -2px; margin-right: 4px; }
  main { overflow-y: auto; }
  .page { max-width: 760px; margin: 0 auto; padding: 36px 36px 60px; animation: enter 0.35s cubic-bezier(0.2, 0.7, 0.2, 1); }
  @keyframes enter { from { opacity: 0; transform: translateY(8px); } }
  .toast { position: fixed; bottom: 22px; left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: 16px; background: var(--elevated); border: 1px solid var(--hairline-strong); border-radius: 12px; padding: 11px 14px 11px 18px; font-size: 13.5px; box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45); animation: pop 0.25s cubic-bezier(0.2, 0.9, 0.3, 1.2); }
  .toast button { border: 0; background: var(--cta-bg); color: var(--cta-fg); border-radius: 8px; padding: 5px 12px; font-weight: 600; cursor: pointer; }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, 10px) scale(0.96); } }
</style>
