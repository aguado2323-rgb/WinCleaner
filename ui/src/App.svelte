<script>
  import { invoke } from '@tauri-apps/api/core';

  let rules = $state([]);
  let selected = $state({});
  let results = $state({});
  let busy = $state(false);

  const fmt = (n) => (n > 1e9 ? (n / 1e9).toFixed(2) + ' GB' : (n / 1e6).toFixed(1) + ' MB');
  let ids = $derived(rules.filter((r) => selected[r.id]).map((r) => r.id));
  let total = $derived(Object.values(results).reduce((a, r) => a + r.bytes, 0));

  async function load() {
    rules = await invoke('list_rules');
    selected = Object.fromEntries(rules.map((r) => [r.id, r.safe]));
  }

  async function analyze() {
    busy = true;
    const res = await invoke('scan', { ids });
    results = Object.fromEntries(res.map((r) => [r.rule_id, r]));
    busy = false;
  }

  async function clean() {
    busy = true;
    await invoke('clean', { ids, dryRun: false });
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
    <button class="primary" onclick={clean} disabled={busy || !total}>Limpiar</button>
  </footer>
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
  button:disabled { opacity: 0.5; cursor: default; }
</style>
