<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';

  let { notify } = $props();

  let plans = $state([]);
  let loading = $state(true);
  let busy = $state(false);

  async function load() {
    try {
      plans = await invoke('power_list');
    } catch (e) {
      notify(t('No se pudieron leer los planes de energía: ') + e);
    }
    loading = false;
  }

  async function activate(p) {
    busy = true;
    try {
      await invoke('power_activate', { guid: p.guid });
      notify(t('Plan activo: {n}', { n: p.name }));
    } catch (e) {
      notify(String(e));
    }
    await load();
    busy = false;
  }

  async function addUltimate() {
    busy = true;
    try {
      await invoke('power_ultimate');
      notify(t('Plan «Máximo rendimiento» añadido. Actívalo desde la lista.'));
    } catch (e) {
      notify(String(e));
    }
    await load();
    busy = false;
  }

  load();
</script>

<div class="sticky">
  <p class="label">{t('Energía')}</p>
  <h1 class="h1">{t('Rendimiento o batería.')}</h1>
  <p class="lede">{t('Elige el plan de energía de Windows. Cambiar es inmediato y siempre puedes volver al anterior.')} <Help text={H.powerInfo.text} risk={H.powerInfo.risk} label={t('Qué es un plan de energía')} /></p>
</div>

{#if loading}
  <p class="empty num">···</p>
{:else}
  <ul>
    {#each plans as p, n (p.guid)}
      <li class:on={p.active} style="animation-delay:{n * 18}ms">
        <span class="txt"><b>{p.name}</b></span>
        <span class="state label" class:on={p.active}>{p.active ? t('Activo') : ''}</span>
        <button class="act" disabled={busy || p.active} onclick={() => activate(p)}>{p.active ? t('En uso') : t('Activar')}</button>
      </li>
    {/each}
  </ul>
  <div class="more">
    <button class="btn" disabled={busy} onclick={addUltimate}>{t('Añadir «Máximo rendimiento»')}</button>
    <Help text={H.powerUltimate.text} risk={H.powerUltimate.risk} label={t('Qué es Máximo rendimiento')} />
  </div>
{/if}

<style>
  ul { list-style: none; margin: 24px 0 0; padding: 0; border-top: 1px solid var(--ink); max-width: 560px; }
  li { display: flex; align-items: center; gap: 14px; padding: 14px 0; border-bottom: 1px solid var(--line, rgba(0,0,0,.12)); }
  .txt { flex: 1; min-width: 0; }
  .state { width: 60px; text-align: right; color: var(--accent); }
  .act { border: 1px solid var(--ink); background: none; padding: 6px 12px; font-family: var(--mono); font-size: 12px; min-width: 84px; }
  .act:hover:not(:disabled) { background: var(--ink); color: var(--paper); }
  .act:disabled { opacity: 0.4; cursor: default; }
  .more { display: flex; align-items: center; gap: 10px; margin-top: 22px; }
</style>
