<script>
  import { invoke } from '@tauri-apps/api/core';
  import { Tween } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';
  import Icon from './Icon.svelte';
  import { fmt, accents } from './format.js';

  let { notify, refreshQuarantine, openQuarantine } = $props();

  let rules = $state([]);
  let selected = $state({});
  let results = $state({});
  let phase = $state('scanning'); // scanning | ready | cleaning | done
  let confirming = $state(false);
  let freed = $state(0);

  const R = 92;
  const C = 2 * Math.PI * R;
  const counter = new Tween(0, { duration: 700, easing: cubicOut });
  const arc = new Tween(0, { duration: 800, easing: cubicOut });

  let allBytes = $derived(Object.values(results).reduce((a, r) => a + r.bytes, 0));
  let total = $derived(rules.reduce((a, r) => a + (selected[r.id] ? results[r.id]?.bytes ?? 0 : 0), 0));
  let ids = $derived(rules.filter((r) => selected[r.id]).map((r) => r.id));
  let maxBytes = $derived(Math.max(1, ...Object.values(results).map((r) => r.bytes)));
  let groups = $derived(
    Object.entries(
      rules.reduce((g, r) => ((g[r.category] ??= []).push(r), g), {})
    )
  );

  $effect(() => {
    counter.target = phase === 'done' ? freed : total;
    arc.target = phase === 'done' ? 1 : allBytes ? total / allBytes : 0;
  });

  async function scan() {
    phase = 'scanning';
    try {
      const res = await invoke('scan', { ids: rules.map((r) => r.id) });
      results = Object.fromEntries(res.map((r) => [r.rule_id, r]));
    } catch (e) {
      notify('No se pudo analizar: ' + e);
    }
    phase = 'ready';
  }

  async function init() {
    try {
      rules = await invoke('list_rules');
      selected = Object.fromEntries(rules.map((r) => [r.id, r.safe]));
      await scan();
    } catch (e) {
      notify('No se pudieron cargar las reglas: ' + e);
      phase = 'ready';
    }
  }

  async function clean() {
    confirming = false;
    phase = 'cleaning';
    try {
      const out = await invoke('clean', { ids });
      const files = out.reports.reduce((a, r) => a + r.files, 0);
      freed = out.reports.reduce((a, r) => a + r.bytes, 0);
      const skipped = out.reports.reduce((a, r) => a + r.errors.length, 0);
      await refreshQuarantine();
      phase = 'done';
      notify(
        `${files} archivos movidos a la cuarentena` + (skipped ? ` · ${skipped} en uso, omitidos` : ''),
        files ? { label: 'Deshacer', run: () => undo(out.batch) } : null
      );
    } catch (e) {
      notify('No se pudo limpiar: ' + e);
      phase = 'ready';
    }
  }

  async function undo(batch) {
    const out = await invoke('restore', { batch });
    notify(`${out.restored} archivos restaurados`);
    await refreshQuarantine();
    await scan();
  }

  init();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (confirming = false)} />

<header>
  <h1>Limpieza</h1>
  <p>Elige qué quieres liberar. Nada se borra: todo pasa antes por la cuarentena.</p>
</header>

<section class="hero">
  <div class="ring" class:spin={phase === 'scanning' || phase === 'cleaning'}>
    <svg viewBox="0 0 220 220">
      <defs>
        <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="var(--blue)" />
          <stop offset="1" stop-color="var(--green)" />
        </linearGradient>
      </defs>
      <circle cx="110" cy="110" r={R} class="track" />
      {#if phase === 'scanning' || phase === 'cleaning'}
        <circle cx="110" cy="110" r={R} class="arc" stroke-dasharray="{C * 0.22} {C}" />
      {:else}
        <circle cx="110" cy="110" r={R} class="arc" stroke-dasharray="{C * arc.current} {C}" />
      {/if}
    </svg>
    <div class="center">
      {#if phase === 'scanning'}
        <span class="label">Analizando…</span>
      {:else if phase === 'cleaning'}
        <span class="label">Moviendo a cuarentena…</span>
      {:else if phase === 'done'}
        <span class="done"><Icon name="check" size={22} /></span>
        <strong>{fmt(counter.current)}</strong>
        <span class="label">liberados</span>
      {:else}
        <strong>{fmt(counter.current)}</strong>
        <span class="label">{total ? 'se pueden liberar' : 'todo limpio'}</span>
      {/if}
    </div>
  </div>

  <div class="actions">
    {#if phase === 'done'}
      <button class="cta" onclick={scan}>Volver a analizar</button>
      <button class="ghost" onclick={openQuarantine}><Icon name="shield" size={16} /> Ver cuarentena</button>
    {:else}
      <button class="cta" disabled={phase !== 'ready' || !total} onclick={() => (confirming = true)}>
        {total ? `Limpiar ${fmt(total)}` : 'Nada que limpiar'}
      </button>
      <button class="ghost" disabled={phase !== 'ready'} onclick={scan}>Analizar de nuevo</button>
    {/if}
  </div>
</section>

{#each groups as [category, list], gi (category)}
  <h2 style="--c:{accents[category] ?? 'var(--blue)'}"><i></i>{category}</h2>
  <ul>
    {#each list as r, i (r.id)}
      {@const size = results[r.id]?.bytes ?? 0}
      <li style="--c:{accents[category] ?? 'var(--blue)'}; animation-delay:{(gi * 4 + i) * 40}ms">
        <label>
          <input type="checkbox" bind:checked={selected[r.id]} disabled={phase === 'cleaning'} />
          <span class="sw"></span>
          <span class="txt">
            <b>{r.name}</b>
            <small>{r.description}</small>
            <span class="bar"><i style="width:{Math.max(size ? 3 : 0, (size / maxBytes) * 100)}%"></i></span>
          </span>
          <span class="size">{phase === 'scanning' ? '…' : fmt(size)}</span>
        </label>
      </li>
    {/each}
  </ul>
{/each}

{#if confirming}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="overlay" role="presentation" onclick={() => (confirming = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <span class="shield"><Icon name="shield" size={22} /></span>
      <h3>¿Limpiar {fmt(total)}?</h3>
      <p>Los archivos no se borran: se mueven a la cuarentena y puedes devolverlos a su sitio cuando quieras.</p>
      <div class="row">
        <button class="ghost" onclick={() => (confirming = false)}>Cancelar</button>
        <button class="cta small" onclick={clean}>Limpiar</button>
      </div>
    </div>
  </div>
{/if}

<style>
  header h1 { margin: 0; font-size: 28px; font-weight: 600; letter-spacing: -0.3px; }
  header p { margin: 6px 0 0; color: var(--mute); font-size: 14px; }

  .hero { display: flex; align-items: center; gap: 40px; margin: 28px 0 34px; padding: 26px 30px; border: 1px solid var(--hairline); border-radius: 22px; background: linear-gradient(160deg, var(--elevated), var(--surface)); }
  .ring { position: relative; width: 190px; height: 190px; flex: none; }
  .ring svg { width: 100%; height: 100%; transform: rotate(-90deg); }
  .track { fill: none; stroke: var(--hairline); stroke-width: 10; }
  .arc { fill: none; stroke: url(#g); stroke-width: 10; stroke-linecap: round; filter: drop-shadow(0 0 8px var(--glow)); }
  .ring.spin svg { animation: spin 1.1s linear infinite; }
  @keyframes spin { to { transform: rotate(270deg); } }
  .center { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 2px; padding: 0 22px; }
  .center strong { font-size: 32px; font-weight: 600; letter-spacing: -0.8px; font-variant-numeric: tabular-nums; }
  .label { font-size: 12.5px; color: var(--mute); }
  .done { color: var(--green); display: grid; margin-bottom: 4px; }
  .actions { display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }

  .cta { border: 0; border-radius: 99px; padding: 12px 26px; font-weight: 600; font-size: 15px; background: var(--cta-bg); color: var(--cta-fg); cursor: pointer; transition: transform 0.15s, opacity 0.15s; }
  .cta:hover:not(:disabled) { transform: translateY(-1px); }
  .cta:active:not(:disabled) { transform: scale(0.97); }
  .cta:disabled { opacity: 0.35; cursor: default; }
  .cta.small { padding: 9px 20px; font-size: 14px; }
  .ghost { display: inline-flex; align-items: center; gap: 8px; border: 1px solid var(--hairline-strong); background: none; border-radius: 99px; padding: 9px 18px; color: var(--body); cursor: pointer; font-size: 13.5px; transition: background 0.15s; }
  .ghost:hover:not(:disabled) { background: var(--hairline); }
  .ghost:disabled { opacity: 0.4; cursor: default; }

  h2 { display: flex; align-items: center; gap: 9px; margin: 26px 0 10px; font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.9px; color: var(--mute); }
  h2 i { width: 7px; height: 7px; border-radius: 50%; background: var(--c); box-shadow: 0 0 10px var(--c); }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  li { animation: rise 0.45s cubic-bezier(0.2, 0.7, 0.2, 1) both; }
  @keyframes rise { from { opacity: 0; transform: translateY(10px); } }
  label { display: flex; align-items: center; gap: 16px; padding: 14px 18px; border: 1px solid var(--hairline); border-radius: 14px; background: var(--surface); cursor: pointer; transition: border-color 0.15s, background 0.15s; }
  label:hover { border-color: var(--hairline-strong); background: var(--elevated); }
  input { position: absolute; opacity: 0; pointer-events: none; }
  .sw { flex: none; width: 36px; height: 21px; border-radius: 99px; background: var(--hairline-strong); position: relative; transition: background 0.2s; }
  .sw::after { content: ''; position: absolute; top: 3px; left: 3px; width: 15px; height: 15px; border-radius: 50%; background: #fff; transition: transform 0.2s cubic-bezier(0.3, 1.4, 0.5, 1); }
  input:checked + .sw { background: var(--c); }
  input:checked + .sw::after { transform: translateX(15px); }
  input:focus-visible + .sw { outline: 2px solid var(--c); outline-offset: 2px; }
  .txt { flex: 1; display: grid; gap: 2px; min-width: 0; }
  .txt b { font-weight: 500; font-size: 14.5px; }
  .txt small { color: var(--mute); font-size: 12.5px; }
  .bar { height: 3px; border-radius: 3px; background: var(--hairline); margin-top: 7px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--c); border-radius: 3px; transition: width 0.7s cubic-bezier(0.2, 0.7, 0.2, 1); }
  .size { min-width: 64px; text-align: right; font-variant-numeric: tabular-nums; font-size: 13.5px; color: var(--body); }

  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.55); backdrop-filter: blur(6px); display: grid; place-items: center; animation: fade 0.2s; z-index: 10; }
  @keyframes fade { from { opacity: 0; } }
  .dialog { width: 380px; background: var(--elevated); border: 1px solid var(--hairline-strong); border-radius: 20px; padding: 26px; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5); animation: pop 0.28s cubic-bezier(0.2, 0.9, 0.3, 1.2); }
  @keyframes pop { from { opacity: 0; transform: scale(0.94); } }
  .shield { display: inline-grid; place-items: center; width: 42px; height: 42px; border-radius: 12px; color: var(--blue); background: var(--glow); }
  .dialog h3 { margin: 14px 0 6px; font-size: 19px; font-weight: 600; }
  .dialog p { margin: 0 0 22px; color: var(--mute); font-size: 14px; line-height: 1.55; }
  .row { display: flex; justify-content: flex-end; gap: 10px; }
</style>
