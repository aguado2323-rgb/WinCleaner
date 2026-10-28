<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { fmt, fmtDate } from './format.js';
  import { invoke } from '@tauri-apps/api/core';

  let { notify } = $props();

  let r = $state(null);
  const KIND = { clean: 'Limpieza', auto: 'Programada', purge: 'Cuarentena vaciada', recycle: 'Papelera' }; // claves en español: se traducen al mostrarlas

  let max = $derived(r ? Math.max(1, ...r.last_30_days) : 1);

  async function load() {
    try {
      r = await invoke('usage_report');
    } catch (e) {
      notify(t('No se pudo leer el informe: ') + e);
    }
  }

  load();
</script>

<div class="sticky">
  <p class="label">{t('Informe')}</p>
  <h1 class="h1">{t('Lo que llevas liberado.')}</h1>
  <p class="lede">{t('Resumen de lo que WinCleaner ha hecho en este equipo. Solo guarda cifras y fechas, nunca nombres de archivos.')} <Help text={H.reportInfo.text} risk={H.reportInfo.risk} label={t('Qué guarda el informe')} /></p>
</div>

{#if !r}
  <p class="empty num">···</p>
{:else if !r.runs && !r.purged_bytes && !r.recycle_bytes}
  <p class="empty">{t('Todavía no hay nada que contar. Haz una limpieza y volverá aquí.')}</p>
{:else}
  <section class="stats">
    <div><span class="num big">{fmt(r.cleaned_bytes)}</span><span class="cap">{t('movidos a la cuarentena')} · {r.runs} {r.runs === 1 ? t('limpieza') : t('limpiezas')}</span></div>
    <div><span class="num big">{fmt(r.purged_bytes + r.recycle_bytes)}</span><span class="cap">{t('borrados de verdad (cuarentena y Papelera)')}</span></div>
  </section>

  <p class="label sub">{t('Últimos 30 días')}</p>
  <div class="chart" role="img" aria-label={t('Limpiado por día en los últimos 30 días')}>
    {#each r.last_30_days as b, i (i)}
      <i class:zero={!b} style="height:{b ? Math.max(6, (b / max) * 100) : 2}%" title={fmt(b)}></i>
    {/each}
  </div>

  <p class="label sub">{t('Últimos movimientos')}</p>
  <ul>
    {#each r.recent as e, n (n)}
      <li><span class="num when">{fmtDate(e.when)}</span><span class="kind">{t(KIND[e.kind] ?? e.kind)}</span><span class="num sz">{fmt(e.bytes)}</span></li>
    {/each}
  </ul>
{/if}

<style>
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 28px; margin-top: 28px; max-width: 640px; }
  .big { display: block; font-size: 40px; line-height: 1.1; }
  .cap { color: var(--ink-2); font-size: 13px; }
  .sub { margin: 34px 0 10px; }
  .chart { display: flex; align-items: flex-end; gap: 3px; height: 90px; max-width: 640px; border-bottom: 1px solid var(--ink); }
  .chart i { flex: 1; background: var(--accent); min-width: 3px; }
  .chart i.zero { background: var(--ink); opacity: 0.2; }
  ul { list-style: none; margin: 0; padding: 0; max-width: 640px; border-top: 1px solid var(--ink); }
  li { display: flex; gap: 14px; padding: 10px 0; border-bottom: 1px solid var(--line, rgba(0,0,0,.12)); font-size: 13px; }
  .when { color: var(--ink-2); width: 170px; }
  .kind { flex: 1; }
  .sz { text-align: right; }
</style>
