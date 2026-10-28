<script>
  import { invoke } from '@tauri-apps/api/core';

  let { notify } = $props();

  let items = $state([]);
  let loading = $state(true);
  let filter = $state('all');
  let busy = $state({});

  let active = $derived(items.filter((i) => i.enabled).length);
  let shown = $derived(
    items.filter((i) => filter === 'all' || (filter === 'on' ? i.enabled : !i.enabled))
  );

  async function load() {
    try {
      items = await invoke('startup_items');
    } catch (e) {
      notify('No se pudo leer el arranque: ' + e);
    }
    loading = false;
  }

  async function toggle(item) {
    const enabled = !item.enabled;
    busy[item.id] = true;
    item.enabled = enabled; // respuesta inmediata; se revierte si falla
    try {
      await invoke('startup_set', { id: item.id, enabled });
      notify(enabled ? `${item.name} volverá a iniciarse con Windows` : `${item.name} ya no se iniciará con Windows`, {
        label: 'Deshacer',
        run: () => toggle(item),
      });
    } catch (e) {
      item.enabled = !enabled;
      notify(item.needs_admin ? 'Esta entrada es del sistema: ejecuta WinCleaner como administrador para cambiarla.' : 'No se pudo cambiar: ' + e);
    }
    busy[item.id] = false;
  }

  load();
</script>

<p class="label">Arranque</p>
<h1 class="h1">Lo que arranca con Windows.</h1>
<p class="lede">Desactivar es reversible: el programa sigue instalado y puedes volver a activarlo cuando quieras.</p>

<section class="summary">
  <span class="big num">{active}<small>/{items.length}</small></span>
  <span class="cap">se inician con Windows</span>
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={filter === 'all'} class:on={filter === 'all'} onclick={() => (filter = 'all')}>Todos</button>
    <button role="tab" aria-selected={filter === 'on'} class:on={filter === 'on'} onclick={() => (filter = 'on')}>Activos</button>
    <button role="tab" aria-selected={filter === 'off'} class:on={filter === 'off'} onclick={() => (filter = 'off')}>Desactivados</button>
  </div>
</section>

{#if loading}
  <p class="empty num">···</p>
{:else if !shown.length}
  <p class="empty">{items.length ? 'Nada en esta lista.' : 'Windows no ha encontrado programas de arranque.'}</p>
{:else}
  <ul>
    {#each shown as item, i (item.id)}
      <li style="animation-delay:{i * 24}ms" class:off={!item.enabled}>
        <div class="txt">
          <span class="top">
            <b>{item.name}</b>
            <span class="tag label">{item.source}</span>
          </span>
          <small title={item.command}>{item.command || 'Sin comando'}</small>
        </div>
        <div class="seg" role="group" aria-label="Iniciar {item.name} con Windows">
          <button class:on={item.enabled} disabled={busy[item.id]} aria-pressed={item.enabled} onclick={() => !item.enabled && toggle(item)}>Sí</button>
          <button class:on={!item.enabled} disabled={busy[item.id]} aria-pressed={!item.enabled} onclick={() => item.enabled && toggle(item)}>No</button>
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .h1 { margin-top: 10px; }
  .summary { display: flex; align-items: baseline; flex-wrap: wrap; gap: 8px 14px; margin: 28px 0 8px; padding-bottom: 0; }
  .big { font-size: 72px; font-weight: 500; letter-spacing: -0.06em; line-height: 1; }
  .big small { font-size: 28px; color: var(--ink-3); letter-spacing: -0.03em; }
  .cap { flex: 1; color: var(--ink-2); }
  .summary .tabs { flex-basis: 100%; margin-top: 18px; }

  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; align-items: center; gap: 18px; padding: 13px 2px; border-bottom: 1px solid var(--rule); animation: line 0.22s both; }
  li.off .txt { opacity: 0.5; }
  .txt { flex: 1; min-width: 0; display: grid; gap: 2px; transition: opacity 0.15s; }
  .top { display: flex; align-items: baseline; gap: 10px; }
  .top b { font-weight: 600; font-size: 14.5px; }
  .tag { font-size: 10px; }
  small { color: var(--ink-3); font-family: var(--mono); font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .seg { display: flex; flex: none; border: 1px solid var(--ink); border-radius: 2px; overflow: hidden; }
  .seg button { border: 0; background: none; padding: 6px 13px; font-family: var(--mono); font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-3); transition: background 0.1s; }
  .seg button + button { border-left: 1px solid var(--ink); }
  .seg button:hover:not(.on):not(:disabled) { background: var(--paper-2); color: var(--ink); }
  .seg button.on { background: var(--ink); color: var(--paper); }
  .seg button.on:first-child { background: var(--accent); color: var(--on-accent); }
  .seg button:disabled { opacity: 0.5; }
  .empty { padding: 50px 0; color: var(--ink-2); font-size: 15px; }
</style>
