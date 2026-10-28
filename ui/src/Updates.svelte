<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';

  let { notify } = $props();

  let items = $state([]);
  let selected = $state({});
  let status = $state({}); // id -> 'working' | 'ok' | 'error'
  let loading = $state(true);
  let working = $state(false);
  let failed = $state(false);

  let chosen = $derived(items.filter((i) => selected[i.id]));

  async function load() {
    loading = true;
    failed = false;
    try {
      items = await invoke('updates_scan');
      selected = Object.fromEntries(items.map((i) => [i.id, true]));
      status = {};
    } catch (e) {
      failed = true;
      notify(String(e));
    }
    loading = false;
  }

  async function run() {
    working = true;
    let ok = 0;
    let bad = 0;
    for (const u of chosen) {
      status[u.id] = 'working';
      try {
        await invoke('updates_apply', { id: u.id });
        status[u.id] = 'ok';
        ok++;
      } catch (e) {
        status[u.id] = 'error';
        bad++;
        notify(String(e));
      }
    }
    notify(t('{n} actualizados', { n: ok }) + (bad ? ', ' + t('{n} con error', { n: bad }) : ''));
    working = false;
  }

  load();
</script>

<p class="label">{t('Actualizaciones')}</p>
<h1 class="h1">{t('Programas al día.')}</h1>
<p class="lede">{t('Busca versiones nuevas de tus programas con winget, el gestor oficial de Windows, y las instala. Las versiones nuevas suelen traer arreglos de seguridad.')} <Help text={H.updatesInfo.text} risk={H.updatesInfo.risk} label={t('Cómo funcionan las actualizaciones')} /></p>

<section class="top">
  <span class="big num">{loading ? '···' : items.length}</span>
  <span class="cap">{items.length === 1 ? t('programa con actualización') : t('programas con actualización')}</span>
  <button class="btn primary" disabled={working || !chosen.length} onclick={run}>
    {working ? t('Actualizando…') : t('Actualizar {n}', { n: chosen.length })}
  </button>
  <button class="link" disabled={working || loading} onclick={load}>{t('Buscar de nuevo')}</button>
</section>

{#if loading}
  <p class="empty num">···</p>
{:else if failed}
  <p class="note">{t('No se pudo consultar winget. Viene con «Instalador de aplicación» de la Microsoft Store.')}</p>
{:else if !items.length}
  <p class="empty">{t('Todo está al día.')}</p>
{:else}
  <ul>
    {#each items as u, n (u.id)}
      <li style="animation-delay:{n * 18}ms">
        <label class="row">
          <input type="checkbox" bind:checked={selected[u.id]} disabled={working} />
          <span class="box" aria-hidden="true"></span>
          <span class="txt"><b>{u.name}</b><small>{u.id}</small></span>
        </label>
        <span class="ver num">{u.current} → {u.available}</span>
        <span class="st label" class:ok={status[u.id] === 'ok'} class:err={status[u.id] === 'error'}>
          {status[u.id] === 'working' ? t('Instalando…') : status[u.id] === 'ok' ? t('Hecho') : status[u.id] === 'error' ? t('Error') : ''}
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .top { display: flex; align-items: baseline; gap: 14px; margin-top: 26px; flex-wrap: wrap; }
  .big { font-size: 44px; line-height: 1; }
  .cap { color: var(--ink-2); flex: 1; min-width: 140px; }
  ul { list-style: none; margin: 20px 0 0; padding: 0; border-top: 1px solid var(--ink); }
  li { display: flex; align-items: center; gap: 14px; padding: 12px 0; border-bottom: 1px solid var(--line, rgba(0,0,0,.12)); }
  .row { display: flex; align-items: center; gap: 12px; flex: 1; min-width: 0; cursor: pointer; }
  .row input { position: absolute; opacity: 0; }
  .box { width: 18px; height: 18px; border: 1px solid var(--ink); flex: none; display: grid; place-items: center; }
  .row input:checked + .box { background: var(--accent); }
  .row input:checked + .box::after { content: ''; width: 8px; height: 4px; border: solid var(--on-accent); border-width: 0 0 2px 2px; transform: rotate(-45deg) translate(1px, -1px); }
  .row input:focus-visible + .box { outline: 2px solid var(--accent); outline-offset: 2px; }
  .txt { min-width: 0; }
  .txt b { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .txt small { color: var(--ink-2); font-size: 12px; }
  .ver { font-size: 12px; color: var(--ink-2); white-space: nowrap; }
  .st { width: 80px; text-align: right; }
  .st.ok { color: var(--accent); }
  .st.err { color: var(--danger, #c0392b); }
  .note { margin: 18px 0 0; padding: 10px 12px; border-left: 3px solid var(--accent); font-size: 13px; color: var(--ink-2); }
</style>
