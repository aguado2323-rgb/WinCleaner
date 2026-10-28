<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';
  import { Tween } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';
  import { size, fmt } from './format.js';

  let { notify, refreshQuarantine, openQuarantine } = $props();

  let rules = $state([]);
  let selected = $state({});
  let results = $state({});
  let phase = $state('scanning'); // scanning | ready | cleaning | done
  let confirming = $state(false);
  let freed = $state(0);

  const counter = new Tween(0, { duration: 600, easing: cubicOut });

  let total = $derived(rules.reduce((a, r) => a + (selected[r.id] ? results[r.id]?.bytes ?? 0 : 0), 0));
  let ids = $derived(rules.filter((r) => selected[r.id]).map((r) => r.id));
  let maxBytes = $derived(Math.max(1, ...Object.values(results).map((r) => r.bytes)));
  let groups = $derived(Object.entries(rules.reduce((g, r) => ((g[r.category] ??= []).push(r), g), {})));
  let tape = $derived(
    rules
      .filter((r) => selected[r.id] && results[r.id]?.bytes)
      .map((r) => ({ id: r.id, name: r.name, bytes: results[r.id].bytes }))
      .sort((a, b) => b.bytes - a.bytes)
  );
  let shown = $derived(size(counter.current));

  $effect(() => {
    counter.target = phase === 'done' ? freed : total;
  });

  async function scan() {
    phase = 'scanning';
    try {
      const res = await invoke('scan', { ids: rules.map((r) => r.id) });
      results = Object.fromEntries(res.map((r) => [r.rule_id, r]));
    } catch (e) {
      notify(t('No se pudo analizar: ') + e);
    }
    phase = 'ready';
  }

  async function init() {
    try {
      rules = await invoke('list_rules');
      selected = Object.fromEntries(rules.map((r) => [r.id, r.safe]));
      await scan();
    } catch (e) {
      notify(t('No se pudieron cargar las reglas: ') + e);
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
        t('{n} archivos movidos a la cuarentena', { n: files }) + (skipped ? t(' · {n} en uso, omitidos', { n: skipped }) : ''),
        files ? { label: t('Deshacer'), run: () => undo(out.batch) } : null
      );
    } catch (e) {
      notify(t('No se pudo limpiar: ') + e);
      phase = 'ready';
    }
  }

  async function undo(batch) {
    const out = await invoke('restore', { batch });
    notify(t('{n} archivos restaurados', { n: out.restored }));
    await refreshQuarantine();
    await scan();
  }

  let bin = $state(null);
  let binConfirm = $state(false);
  let binBusy = $state(false);

  async function loadBin() {
    try {
      bin = await invoke('recycle_state');
    } catch {
      bin = null;
    }
  }

  async function emptyBin() {
    binBusy = true;
    binConfirm = false;
    try {
      await invoke('recycle_clear');
      notify(t('Papelera vaciada.'));
    } catch (e) {
      notify(t('No se pudo vaciar la Papelera: ') + e);
    }
    await loadBin();
    binBusy = false;
  }

  loadBin();

  const setAll = (list, on) => list.forEach((r) => (selected[r.id] = on));

  init();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (confirming = false)} />

<p class="label">{t('Limpieza')}</p>
<p class="lede">{t('Marca lo que quieras limpiar. Pasa el ratón por el')} <b>?</b> {t('de cada línea para ver qué es y si tiene algún riesgo. Nada se borra: todo va a la cuarentena.')}</p>

<section class="hero">
  <div class="figure" class:busy={phase === 'scanning' || phase === 'cleaning'}>
    {#if phase === 'scanning'}
      <span class="big dots" aria-label={t('Analizando')}>···</span>
    {:else}
      <span class="big num">{shown.n}</span><span class="unit num">{shown.u}</span>
    {/if}
  </div>
  <p class="cap">
    {#if phase === 'scanning'}{t('Analizando tu equipo…')}
    {:else if phase === 'cleaning'}{t('Moviendo a la cuarentena…')}
    {:else if phase === 'done'}{t('liberados · están en la cuarentena por si cambias de idea')}
    {:else if total}{t('recuperables')} · {ids.length} {ids.length === 1 ? t('categoría seleccionada') : t('categorías seleccionadas')}
    {:else}{t('no hay nada seleccionado que limpiar')}{/if}
  </p>

  <div class="tape" aria-hidden="true">
    {#each tape as seg, i (seg.id)}
      <i class:lead={i === 0} style="flex:{seg.bytes}" title="{seg.name} · {fmt(seg.bytes)}"></i>
    {/each}
  </div>

  <div class="act">
    {#if phase === 'done'}
      <button class="btn primary" onclick={scan}>{t('Analizar de nuevo')}</button>
      <button class="link" onclick={openQuarantine}>{t('Ver cuarentena →')}</button>
    {:else}
      <button class="btn primary" disabled={phase !== 'ready' || !total} onclick={() => (confirming = true)}>
        {total ? t('Limpiar {x}', { x: fmt(total) }) : t('Nada que limpiar')}
      </button>
      <Help text={H.cleanButton.text} risk={H.cleanButton.risk} label={t('Qué hace Limpiar')} />
      <button class="link" disabled={phase !== 'ready'} onclick={scan}>{t('Analizar de nuevo')}</button>
      <Help text={H.cleanRescan.text} risk={H.cleanRescan.risk} label={t('Qué hace Analizar')} />
    {/if}
  </div>
</section>

{#each groups as [category, list] (category)}
  <div class="group">
    <h2 class="label">{t(category)}</h2>
    <span class="sel">
      <button class="link" onclick={() => setAll(list, true)}>{t('Todo')}</button>
      <button class="link" onclick={() => setAll(list, false)}>{t('Nada')}</button>
    </span>
  </div>
  <ul>
    {#each list as r, i (r.id)}
      {@const bytes = results[r.id]?.bytes ?? 0}
      <li style="animation-delay:{i * 28}ms">
        <label>
          <input type="checkbox" bind:checked={selected[r.id]} disabled={phase === 'cleaning'} />
          <span class="box" aria-hidden="true"></span>
          <span class="txt">
            <b>{t(r.name)}</b>
            <small>{t(r.description)}</small>
          </span>
          <Help text={r.help} risk={r.safe ? 'reversible' : 'care'} label={t('Qué hace {n}', { n: t(r.name) })} />
          <span class="mini" aria-hidden="true"><i style="width:{(bytes / maxBytes) * 100}%"></i></span>
          <span class="sz num">{phase === 'scanning' ? '—' : fmt(bytes)}</span>
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
      <h3>{t('¿Limpiar {x}?', { x: fmt(total) })}</h3>
      <p>{t('Los archivos no se borran: pasan a la cuarentena y puedes devolverlos a su sitio cuando quieras.')}</p>
      <div class="row">
        <button class="btn" onclick={() => (confirming = false)}>{t('Cancelar')}</button>
        <button class="btn primary" onclick={clean}>{t('Limpiar')}</button>
      </div>
    </div>
  </div>
{/if}

<section class="bin">
  <div class="binTxt">
    <b>{t('Papelera de reciclaje')}</b>
    <small>{bin ? (bin.items ? t('{n} elementos', { n: bin.items }) + ' · ' + fmt(bin.bytes) : t('Está vacía')) : t('Comprobando…')} · {t('se vacía de verdad, sin pasar por la cuarentena')}</small>
  </div>
  <Help text={H.recycle.text} risk={H.recycle.risk} label={t('Qué hace Vaciar la Papelera')} />
  {#if binConfirm}
    <span class="binAsk">{t('¿Seguro?')}</span>
    <button class="btn danger" disabled={binBusy} onclick={emptyBin}>{t('Sí, vaciar')}</button>
    <button class="link" onclick={() => (binConfirm = false)}>{t('Cancelar')}</button>
  {:else}
    <button class="btn" disabled={binBusy || !bin?.items} onclick={() => (binConfirm = true)}>{t('Vaciar Papelera')}</button>
  {/if}
</section>

<style>
  .bin { display: flex; align-items: center; gap: 12px; margin-top: 28px; padding: 16px 0; border-top: 1px solid var(--ink); }
  .binTxt { flex: 1; min-width: 0; }
  .binTxt small { display: block; color: var(--ink-2); font-size: 12px; margin-top: 3px; }
  .binAsk { font-family: var(--mono); font-size: 12px; }

  .hero { padding: 26px 0 30px; }
  .figure { display: flex; align-items: baseline; gap: 10px; line-height: 0.9; }
  .big { font-size: 112px; font-weight: 500; letter-spacing: -0.06em; }
  .unit { font-size: 28px; color: var(--accent); font-weight: 600; }
  .dots { color: var(--ink-3); animation: blink 1s steps(3) infinite; }
  @keyframes blink { 50% { opacity: 0.3; } }
  .cap { margin: 14px 0 22px; color: var(--ink-2); }

  .tape { display: flex; gap: 2px; height: 10px; margin-bottom: 28px; background: var(--paper-2); }
  .tape i { display: block; background: var(--ink); opacity: 0.85; min-width: 2px; transition: flex 0.5s cubic-bezier(0.2, 0.7, 0.2, 1); }
  .tape i:nth-child(even) { opacity: 0.5; }
  .tape i.lead { background: var(--accent); opacity: 1; }
  .act { display: flex; align-items: center; gap: 26px; }

  .group { display: flex; align-items: baseline; justify-content: space-between; margin: 30px 0 4px; padding-bottom: 8px; border-bottom: 1px solid var(--ink); }
  .group h2 { margin: 0; color: var(--ink); }
  .sel { display: flex; gap: 14px; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { animation: line 0.25s both; border-bottom: 1px solid var(--rule); }
  label { display: flex; align-items: center; gap: 16px; padding: 13px 2px; cursor: pointer; transition: background 0.1s; }
  label:hover { background: var(--paper-2); }
  input { position: absolute; opacity: 0; pointer-events: none; }
  .box { flex: none; position: relative; width: 16px; height: 16px; border: 1.5px solid var(--ink); border-radius: 2px; }
  input:checked + .box { background: var(--accent); border-color: var(--ink); }
  input:checked + .box::after { content: ''; position: absolute; left: 4px; top: 0; width: 4px; height: 9px; border: solid var(--on-accent); border-width: 0 2px 2px 0; transform: rotate(40deg); }
  input:focus-visible + .box { outline: 2px solid var(--accent); outline-offset: 2px; }
  .txt { flex: 1; display: grid; gap: 1px; min-width: 0; }
  .txt b { font-weight: 600; font-size: 14.5px; }
  .txt small { color: var(--ink-3); font-size: 12.5px; }
  .mini { flex: none; width: 56px; height: 3px; background: var(--paper-2); }
  .mini i { display: block; height: 100%; background: var(--ink); transition: width 0.5s cubic-bezier(0.2, 0.7, 0.2, 1); }
  .sz { flex: none; width: 76px; text-align: right; font-size: 13px; }
</style>
