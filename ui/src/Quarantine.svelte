<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';
  import { fmt, fmtDate } from './format.js';

  let { q, notify, refreshQuarantine } = $props();
  let confirm = $state(null); // lote | 'all'

  let totalBytes = $derived(q.batches.reduce((a, b) => a + b.bytes, 0));

  async function restore(batch) {
    const out = await invoke('restore', { batch });
    notify(t('{n} archivos restaurados', { n: out.restored }) + (out.errors.length ? ' · ' + t('{n} no se pudieron restaurar', { n: out.errors.length }) : ''));
    await refreshQuarantine();
  }

  async function purge(batch) {
    confirm = null;
    try {
      await invoke('purge', { batch });
      notify(t('Borrado definitivamente'));
    } catch (e) {
      notify(t('No se pudo borrar: ') + e);
    }
    await refreshQuarantine();
  }

  const openFolder = () => invoke('open_quarantine').catch((e) => notify(t('No se pudo abrir: ') + e));
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (confirm = null)} />

<p class="label">{t('Cuarentena')}</p>
<h1 class="h1">{t('Lo que limpias se guarda aquí.')}</h1>
<p class="lede">{t('Nada se borra de verdad hasta que lo decides tú. Restaura una limpieza y los archivos vuelven a su sitio original.')}</p>

<section class="where">
  <span class="label">{t('Carpeta')}</span>
  <code>{q.path || '—'}</code>
  <button class="link" onclick={openFolder}>{t('Abrir')} ↗</button>
  <Help text={H.quarantineOpen.text} risk={H.quarantineOpen.risk} label={t('Qué hace Abrir')} />
</section>

{#if q.batches.length}
  <div class="head">
    <span class="num">{q.batches.length} {q.batches.length === 1 ? t('limpieza') : t('limpiezas')} · {fmt(totalBytes)}</span>
    <span class="acts">
      <Help text={H.quarantineAll.text} risk={H.quarantineAll.risk} label={t('Qué hace Vaciar todo')} />
      <button class="btn danger" onclick={() => (confirm = 'all')}>{t('Vaciar todo')}</button>
    </span>
  </div>
  <ul>
    {#each q.batches as b, i (b.batch)}
      <li style="animation-delay:{i * 28}ms">
        <span class="date num">{fmtDate(b.batch)}</span>
        <span class="meta num">{t('{n} archivos', { n: b.files })}</span>
        <span class="sz num">{fmt(b.bytes)}</span>
        <button class="link" onclick={() => restore(b.batch)}>{t('Restaurar')}</button>
        <Help text={H.quarantineRestore.text} risk={H.quarantineRestore.risk} label={t('Qué hace Restaurar')} />
        <button class="link x" onclick={() => (confirm = b.batch)}>{t('Borrar')}</button>
        <Help text={H.quarantineDelete.text} risk={H.quarantineDelete.risk} label={t('Qué hace Borrar')} />
      </li>
    {/each}
  </ul>
{:else}
  <div class="empty">
    <p class="big num">0</p>
    <p>{t('La cuarentena está vacía. Cuando limpies, aparecerá aquí.')}</p>
  </div>
{/if}

{#if confirm}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="overlay" role="presentation" onclick={() => (confirm = null)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <h3>{confirm === 'all' ? t('¿Vaciar toda la cuarentena?') : t('¿Borrar esta limpieza?')}</h3>
      <p>{t('Esta acción es definitiva: los archivos ya no se podrán recuperar.')}</p>
      <div class="row">
        <button class="btn" onclick={() => (confirm = null)}>{t('Cancelar')}</button>
        <button class="btn primary" onclick={() => purge(confirm === 'all' ? null : confirm)}>{t('Borrar definitivamente')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .h1 { margin-top: 10px; }
  .where { display: flex; align-items: baseline; gap: 16px; margin: 28px 0 30px; padding: 12px 0; border-block: 1px solid var(--rule); }
  code { flex: 1; min-width: 0; font-family: var(--mono); font-size: 12.5px; color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; user-select: text; }
  .head { display: flex; align-items: center; justify-content: space-between; padding-bottom: 10px; border-bottom: 1px solid var(--ink); font-size: 13px; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; align-items: baseline; gap: 22px; padding: 14px 2px; border-bottom: 1px solid var(--rule); animation: line 0.25s both; }
  .date { flex: 1; font-size: 13px; }
  .meta { color: var(--ink-3); font-size: 12.5px; }
  .sz { width: 76px; text-align: right; font-size: 13px; }
  .x:hover { color: var(--accent); border-color: var(--accent); }
  .acts { display: flex; align-items: center; gap: 10px; }
  .empty { padding: 70px 0; color: var(--ink-2); }
  .empty .big { margin: 0 0 6px; font-size: 96px; font-weight: 500; letter-spacing: -0.06em; color: var(--ink-3); line-height: 1; }
</style>
