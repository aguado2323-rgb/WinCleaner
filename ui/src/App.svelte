<script>
  import { invoke } from '@tauri-apps/api/core';

  let rules = $state([]);
  let selected = $state({});
  let results = $state({});
  let busy = $state(false);
  let confirming = $state(false);
  let lastBatch = $state(null);
  let message = $state('');

  const fmt = (n) => (n > 1e9 ? (n / 1e9).toFixed(2) + ' GB' : (n / 1e6).toFixed(1) + ' MB');
  let ids = $derived(rules.filter((r) => selected[r.id]).map((r) => r.id));
  let total = $derived(Object.values(results).reduce((a, r) => a + r.bytes, 0));

  async function load() {
    rules = await invoke('list_rules');
    selected = Object.fromEntries(rules.map((r) => [r.id, r.safe]));
  }

  async function analyze() {
    busy = true;
    try {
      const res = await invoke('scan', { ids });
      results = Object.fromEntries(res.map((r) => [r.rule_id, r]));
    } catch (e) {
      message = 'No se pudo analizar: ' + e;
    }
    busy = false;
  }

  async function clean() {
    confirming = false;
    busy = true;
    try {
      const out = await invoke('clean', { ids });
      const files = out.reports.reduce((a, r) => a + r.files, 0);
      const bytes = out.reports.reduce((a, r) => a + r.bytes, 0);
      const errs = out.reports.reduce((a, r) => a + r.errors.length, 0);
      lastBatch = files ? out.batch : null;
      message = `${files} archivos (${fmt(bytes)}) movidos a la cuarentena.` + (errs ? ` ${errs} no se pudieron mover (en uso o sin permiso).` : '');
    } catch (e) {
      message = 'No se pudo limpiar: ' + e;
    }
    await analyze();
  }

  async function undo() {
    busy = true;
    try {
      const out = await invoke('restore', { batch: lastBatch });
      message = `${out.restored} archivos restaurados.` + (out.errors.length ? ` ${out.errors.length} no se restauraron.` : '');
      lastBatch = null;
    } catch (e) {
      message = 'No se pudo deshacer: ' + e;
    }
    await analyze();
  }

  load();
</script>

<main>
  <header>
    <h1>WinCleaner</h1>
    <p>Sin anuncios, sin telemetría.</p>
  </header>

  <section>
    {#each rules as r (r.id)}
      <label class="row">
        <input type="checkbox" bind:checked={selected[r.id]} />
        <span class="info"><strong>{r.name}</strong><small>{r.category} · {r.description}</small></span>
        <span class="size">{results[r.id] ? fmt(results[r.id].bytes) : ''}</span>
      </label>
    {/each}
  </section>

  <footer>
    <span>{total ? 'Recuperable: ' + fmt(total) : ''}</span>
    <button onclick={analyze} disabled={busy || !ids.length}>Analizar</button>
    {#if lastBatch}<button onclick={undo} disabled={busy}>Deshacer</button>{/if}
    <button class="primary" onclick={() => (confirming = true)} disabled={busy || !total}>Limpiar</button>
  </footer>

  {#if message}<p class="msg">{message}</p>{/if}

  {#if confirming}
    <div class="overlay">
      <div class="dialog">
        <h2>¿Limpiar {fmt(total)}?</h2>
        <p>Los archivos no se borran: se mueven a una cuarentena y puedes deshacerlo ahora mismo con el botón «Deshacer».</p>
        <div class="actions">
          <button onclick={() => (confirming = false)}>Cancelar</button>
          <button class="primary" onclick={clean}>Limpiar</button>
        </div>
      </div>
    </div>
  {/if}
</main>

<style>
  main { max-width: 760px; margin: 0 auto; padding: 24px; }
  h1 { margin: 0; font-size: 1.6rem; }
  header p { color: var(--muted); margin: 4px 0 20px; }
  .row { display: flex; align-items: center; gap: 12px; background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 12px 16px; margin-bottom: 8px; }
  .info { flex: 1; display: flex; flex-direction: column; }
  small { color: var(--muted); }
  .size { font-variant-numeric: tabular-nums; color: var(--muted); }
  footer { display: flex; align-items: center; justify-content: flex-end; gap: 12px; margin-top: 20px; }
  footer span { margin-right: auto; font-weight: 600; }
  button { border: 1px solid var(--border); background: var(--card); color: var(--text); padding: 8px 18px; border-radius: 10px; font: inherit; cursor: pointer; }
  button.primary { background: var(--accent); border-color: var(--accent); color: #fff; }
  .msg { margin-top: 16px; color: var(--muted); }
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); display: grid; place-items: center; }
  .dialog { background: var(--card); border: 1px solid var(--border); border-radius: 14px; padding: 24px; max-width: 420px; }
  .dialog h2 { margin: 0 0 8px; font-size: 1.15rem; }
  .actions { display: flex; justify-content: flex-end; gap: 12px; margin-top: 16px; }
  button:disabled { opacity: 0.5; cursor: default; }
</style>
