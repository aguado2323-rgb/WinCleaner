<script>
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';

  let { notify } = $props();

  let items = $state([]);
  let loading = $state(true);
  let filter = $state('all');
  let busy = $state({});

  const tint = ['var(--blue)', 'var(--green)', 'var(--violet)', 'var(--yellow)'];
  const sourceTone = { Usuario: 'var(--blue)', Sistema: 'var(--yellow)', 'Carpeta Inicio': 'var(--green)' };

  let active = $derived(items.filter((i) => i.enabled).length);
  let shown = $derived(
    items.filter((i) => filter === 'all' || (filter === 'on' ? i.enabled : !i.enabled))
  );

  const letter = (n) => (n.replace(/[^\p{L}\p{N}]/gu, '')[0] ?? '?').toUpperCase();
  const colorOf = (n) => tint[[...n].reduce((a, c) => a + c.charCodeAt(0), 0) % tint.length];

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

<header>
  <h1>Arranque</h1>
  <p>Programas que se inician con Windows. Desactivar es reversible: el programa sigue instalado y puedes volver a activarlo cuando quieras.</p>
</header>

<section class="summary">
  <div class="big"><b>{active}</b><span>de {items.length} se inician con Windows</span></div>
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={filter === 'all'} class:on={filter === 'all'} onclick={() => (filter = 'all')}>Todos</button>
    <button role="tab" aria-selected={filter === 'on'} class:on={filter === 'on'} onclick={() => (filter = 'on')}>Activos</button>
    <button role="tab" aria-selected={filter === 'off'} class:on={filter === 'off'} onclick={() => (filter = 'off')}>Desactivados</button>
  </div>
</section>

{#if loading}
  <ul>
    {#each [0, 1, 2, 3] as n (n)}<li class="skeleton" style="animation-delay:{n * 90}ms"></li>{/each}
  </ul>
{:else if !shown.length}
  <div class="empty">
    <span><Icon name="power" size={30} /></span>
    <b>{items.length ? 'Nada en esta lista' : 'No hay programas de arranque'}</b>
    <small>{items.length ? 'Prueba con otro filtro.' : 'Windows no ha encontrado entradas que mostrar.'}</small>
  </div>
{:else}
  <ul>
    {#each shown as item, i (item.id)}
      <li style="--c:{colorOf(item.name)}; animation-delay:{i * 30}ms" class:off={!item.enabled}>
        <label>
          <span class="avatar">{letter(item.name)}</span>
          <span class="txt">
            <span class="top">
              <b>{item.name}</b>
              <i class="tag" style="--t:{sourceTone[item.source] ?? 'var(--mute)'}">{item.source}</i>
            </span>
            <small title={item.command}>{item.command || 'Sin comando'}</small>
          </span>
          <input type="checkbox" checked={item.enabled} disabled={busy[item.id]} onchange={() => toggle(item)} aria-label="Iniciar {item.name} con Windows" />
          <span class="sw"></span>
        </label>
      </li>
    {/each}
  </ul>
{/if}

<style>
  header h1 { margin: 0; font-size: 28px; font-weight: 600; letter-spacing: -0.3px; }
  header p { margin: 6px 0 0; color: var(--mute); font-size: 14px; line-height: 1.55; max-width: 580px; }

  .summary { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin: 26px 0 16px; padding: 18px 22px; border: 1px solid var(--hairline); border-radius: 18px; background: linear-gradient(160deg, var(--elevated), var(--surface)); }
  .big { display: flex; align-items: baseline; gap: 12px; }
  .big b { font-size: 38px; font-weight: 600; letter-spacing: -1px; font-variant-numeric: tabular-nums; background: linear-gradient(135deg, var(--blue), var(--green)); -webkit-background-clip: text; background-clip: text; color: transparent; }
  .big span { color: var(--mute); font-size: 13.5px; }
  .tabs { display: flex; gap: 4px; padding: 4px; border-radius: 99px; background: var(--hairline); }
  .tabs button { border: 0; background: none; padding: 7px 14px; border-radius: 99px; color: var(--mute); cursor: pointer; font-size: 13px; transition: all 0.15s; }
  .tabs button.on { background: var(--elevated); color: var(--ink); box-shadow: 0 1px 6px rgba(0, 0, 0, 0.25); }

  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  li { animation: rise 0.4s cubic-bezier(0.2, 0.7, 0.2, 1) both; }
  @keyframes rise { from { opacity: 0; transform: translateY(8px); } }
  label { display: flex; align-items: center; gap: 14px; padding: 13px 18px 13px 14px; border: 1px solid var(--hairline); border-radius: 14px; background: var(--surface); cursor: pointer; transition: border-color 0.15s, background 0.15s, opacity 0.2s; }
  label:hover { border-color: var(--hairline-strong); background: var(--elevated); }
  li.off label { opacity: 0.6; }
  .avatar { display: grid; place-items: center; flex: none; width: 38px; height: 38px; border-radius: 11px; font-weight: 600; color: var(--c); background: color-mix(in srgb, var(--c) 16%, transparent); border: 1px solid color-mix(in srgb, var(--c) 30%, transparent); }
  .txt { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .top { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .top b { font-weight: 500; font-size: 14.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { flex: none; font-style: normal; font-size: 10.5px; letter-spacing: 0.4px; text-transform: uppercase; padding: 2px 8px; border-radius: 99px; color: var(--t); background: color-mix(in srgb, var(--t) 14%, transparent); }
  small { color: var(--ash); font-size: 12px; font-family: 'Cascadia Mono', Consolas, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  input { position: absolute; opacity: 0; pointer-events: none; }
  .sw { flex: none; width: 38px; height: 22px; border-radius: 99px; background: var(--hairline-strong); position: relative; transition: background 0.2s; }
  .sw::after { content: ''; position: absolute; top: 3px; left: 3px; width: 16px; height: 16px; border-radius: 50%; background: #fff; transition: transform 0.2s cubic-bezier(0.3, 1.4, 0.5, 1); }
  input:checked + .sw { background: var(--green); }
  input:checked + .sw::after { transform: translateX(16px); }
  input:focus-visible + .sw { outline: 2px solid var(--green); outline-offset: 2px; }
  input:disabled + .sw { opacity: 0.5; }

  .skeleton { height: 66px; border-radius: 14px; background: var(--surface); border: 1px solid var(--hairline); animation: pulse 1.2s infinite; }
  @keyframes pulse { 50% { opacity: 0.45; } }
  .empty { display: grid; justify-items: center; gap: 6px; padding: 60px 0; color: var(--mute); text-align: center; }
  .empty span { display: grid; place-items: center; width: 64px; height: 64px; border-radius: 20px; color: var(--green); background: color-mix(in srgb, var(--green) 14%, transparent); margin-bottom: 8px; }
  .empty b { color: var(--ink); font-weight: 500; }
</style>
