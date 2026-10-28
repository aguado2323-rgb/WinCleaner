<script>
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';
  import { fmt, fmtDay } from './format.js';

  let { notify, refreshQuarantine } = $props();

  let programs = $state([]);
  let loading = $state(true);
  let query = $state('');
  let target = $state(null); // programa elegido
  let step = $state('confirm'); // confirm | running | scanning | leftovers | done
  let makePoint = $state(true);
  let pointError = $state('');
  let leftovers = $state([]);
  let picked = $state({});
  let result = $state(null);
  let outcome = $state(null);
  let icons = $state({});
  let sortKey = $state('name');
  let sortAsc = $state(true);

  // Orden por columna; los que no tienen dato (fecha vacía) van siempre al final.
  const sorters = {
    name: (p) => p.name.toLowerCase(),
    installed: (p) => p.install_date,
    used: (p) => p.last_used,
    size: (p) => p.size_kb,
  };
  function sortBy(key) {
    if (sortKey === key) sortAsc = !sortAsc;
    else {
      sortKey = key;
      sortAsc = key === 'name';
    }
  }

  let shown = $derived.by(() => {
    const f = sorters[sortKey];
    const list = programs.filter((p) => (p.name + ' ' + p.publisher).toLowerCase().includes(query.trim().toLowerCase()));
    return list.sort((a, b) => {
      const [x, y] = [f(a), f(b)];
      if (x === '' || x === 0) return y === '' || y === 0 ? 0 : 1;
      if (y === '' || y === 0) return -1;
      return (x < y ? -1 : x > y ? 1 : 0) * (sortAsc ? 1 : -1);
    });
  });
  let chosen = $derived(leftovers.filter((l, i) => picked[i]));

  async function load() {
    loading = true;
    try {
      programs = await invoke('programs_list');
    } catch (e) {
      notify('No se pudo leer los programas: ' + e);
    }
    loading = false;
    loadIcons();
  }

  // Los iconos tardan (PowerShell): se piden por tandas y van apareciendo.
  async function loadIcons() {
    const ids = programs.map((p) => p.id).filter((id) => !icons[id]);
    for (let i = 0; i < ids.length; i += 40) {
      try {
        icons = { ...icons, ...(await invoke('programs_icons', { ids: ids.slice(i, i + 40) })) };
      } catch {
        /* sin iconos: se muestra la inicial */
      }
    }
  }

  function ask(p) {
    target = p;
    step = 'confirm';
    pointError = '';
    leftovers = [];
    result = null;
    outcome = null;
  }

  function close() {
    if (step === 'running' || step === 'scanning') return;
    target = null;
  }

  async function start(skipPoint = false) {
    pointError = '';
    if (makePoint && !skipPoint) {
      step = 'running';
      try {
        await invoke('restore_create', { reason: 'uninstall' });
      } catch (e) {
        pointError = String(e);
        step = 'confirm';
        return;
      }
    }
    step = 'running';
    try {
      outcome = await invoke('program_uninstall', { id: target.id });
    } catch (e) {
      outcome = { removed: false, error: String(e) };
    }
    if (outcome.error && !outcome.removed) {
      step = 'done';
      return;
    }
    step = 'scanning';
    try {
      leftovers = await invoke('leftovers_find', { name: target.name, publisher: target.publisher, location: target.location });
    } catch {
      leftovers = [];
    }
    picked = Object.fromEntries(leftovers.map((l, i) => [i, l.selected]));
    step = leftovers.length ? 'leftovers' : 'done';
    await load();
  }

  async function removeLeftovers() {
    step = 'scanning';
    try {
      result = await invoke('leftovers_remove', { items: chosen });
      refreshQuarantine();
    } catch (e) {
      result = { folders: 0, keys: 0, errors: [String(e)], batch: '' };
    }
    step = 'done';
  }

  async function undoRegistry() {
    try {
      const n = await invoke('registry_restore', { batch: result.batch });
      notify(`${n} copias del registro restauradas`);
    } catch (e) {
      notify(String(e));
    }
  }

  load();
</script>

<p class="label">Desinstalar</p>
<h1 class="h1">Fuera, y sin dejar rastro.</h1>
<p class="lede">Usa el desinstalador oficial de cada programa y después busca lo que se haya dejado. Tú revisas los restos antes de quitar nada: las carpetas van a la cuarentena y las claves del registro se guardan en una copia.</p>

<div class="bar">
  <input class="search" type="search" placeholder="Buscar programa…" bind:value={query} aria-label="Buscar programa" />
  <span class="num">{shown.length}<small>/{programs.length}</small></span>
</div>

{#if loading}
  <p class="empty num">···</p>
{:else if !shown.length}
  <p class="empty">{programs.length ? 'Ningún programa coincide.' : 'No se han encontrado programas instalados.'}</p>
{:else}
  <div class="cols label" role="row">
    <span class="ic"></span>
    <button class="col nm" class:on={sortKey === 'name'} onclick={() => sortBy('name')}>Programa {sortKey === 'name' ? (sortAsc ? '↑' : '↓') : ''}</button>
    <button class="col dt" class:on={sortKey === 'installed'} onclick={() => sortBy('installed')}>Instalado {sortKey === 'installed' ? (sortAsc ? '↑' : '↓') : ''}</button>
    <span class="col dt use">
      <button class="col" class:on={sortKey === 'used'} onclick={() => sortBy('used')}>Último uso {sortKey === 'used' ? (sortAsc ? '↑' : '↓') : ''}</button>
      <Help text={H.uninstallDates.text} risk={H.uninstallDates.risk} label="Qué son las fechas" />
    </span>
    <button class="col sz" class:on={sortKey === 'size'} onclick={() => sortBy('size')}>Tamaño {sortKey === 'size' ? (sortAsc ? '↑' : '↓') : ''}</button>
    <span class="act"></span>
  </div>
  <ul>
    {#each shown as p, i (p.id)}
      <li style="animation-delay:{Math.min(i, 20) * 14}ms">
        <span class="ic">
          {#if icons[p.id]}
            <img src="data:image/png;base64,{icons[p.id]}" alt="" width="28" height="28" />
          {:else}
            <span class="ph" aria-hidden="true">{p.name.trim().charAt(0).toUpperCase()}</span>
          {/if}
        </span>
        <div class="txt">
          <b title={p.name}>{p.name}</b>
          <small>{[p.publisher, p.version].filter(Boolean).join(' · ') || '—'}</small>
        </div>
        <span class="dt num">{fmtDay(p.install_date)}</span>
        <span class="dt num">{fmtDay(p.last_used)}</span>
        <span class="sz num">{p.size_kb ? fmt(p.size_kb * 1024) : '—'}</span>
        <span class="act">
          <button class="btn sm" onclick={() => ask(p)}>Desinstalar</button>
          <Help text={H.uninstallButton.text} risk={H.uninstallButton.risk} label="Qué hace Desinstalar" />
        </span>
      </li>
    {/each}
  </ul>
{/if}

{#if target}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="overlay" role="presentation" onclick={close}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="dialog wide" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      {#if step === 'confirm'}
        <h3>¿Desinstalar {target.name}?</h3>
        <p>
          Se abrirá el desinstalador del propio programa.
          {#if target.machine_wide}Está instalado para todos los usuarios: Windows pedirá permisos de administrador.{/if}
        </p>
        <label class="pt">
          <input type="checkbox" bind:checked={makePoint} />
          <span class="box" aria-hidden="true"></span>
          <span>Crear un punto de restauración antes (recomendado)</span>
        </label>
        <Help text={H.uninstallPoint.text} risk={H.uninstallPoint.risk} label="Qué es un punto de restauración" />
        {#if pointError}<p class="err">{pointError}</p>{/if}
        <div class="row">
          <button class="btn" onclick={close}>Cancelar</button>
          {#if pointError}
            <button class="btn" onclick={() => start(true)}>Continuar sin punto</button>
          {:else}
            <button class="btn primary" onclick={() => start()}>Desinstalar</button>
          {/if}
        </div>
      {:else if step === 'running'}
        <h3>Desinstalando…</h3>
        <p>Sigue las instrucciones del desinstalador de {target.name}. WinCleaner espera a que termine.</p>
        <p class="num dots">···</p>
      {:else if step === 'scanning'}
        <h3>Trabajando…</h3>
        <p class="num dots">···</p>
      {:else if step === 'leftovers'}
        <h3>Restos de {target.name}</h3>
        <p>Estas ubicaciones coinciden con el programa. Revisa y marca solo lo que quieras quitar.</p>
        <ul class="lo">
          {#each leftovers as l, i}
            <li>
              <label>
                <input type="checkbox" bind:checked={picked[i]} />
                <span class="box" aria-hidden="true"></span>
                <span class="txt">
                  <b>{l.path}</b>
                  <small>{l.kind === 'folder' ? 'Carpeta · va a la cuarentena' : 'Registro · se guarda copia .reg'}</small>
                </span>
                <Help text={l.kind === 'folder' ? H.leftoverFolder.text : H.leftoverRegistry.text} risk={l.kind === 'folder' ? 'reversible' : 'care'} label="Qué es este resto" />
              </label>
            </li>
          {/each}
        </ul>
        <div class="row">
          <button class="btn" onclick={() => (step = 'done')}>Dejarlos</button>
          <button class="btn primary" disabled={!chosen.length} onclick={removeLeftovers}>Quitar {chosen.length}</button>
        </div>
      {:else}
        <h3>{outcome?.error && !outcome.removed ? 'No se pudo desinstalar' : 'Listo'}</h3>
        {#if outcome?.error && !outcome.removed}
          <p>{outcome.error}</p>
        {:else if result}
          <p>
            {result.folders} {result.folders === 1 ? 'carpeta' : 'carpetas'} a la cuarentena y {result.keys}
            {result.keys === 1 ? 'clave' : 'claves'} del registro quitadas.
          </p>
          {#if result.errors.length}<p class="err">{result.errors.length} con error: {result.errors[0]}</p>{/if}
        {:else}
          <p>{target.name} se ha desinstalado{leftovers.length ? '; los restos se han dejado como estaban' : ' y no se han encontrado restos'}.</p>
        {/if}
        <div class="row">
          {#if result?.keys}<button class="btn" onclick={undoRegistry}>Restaurar registro</button>{/if}
          <button class="btn primary" onclick={() => (target = null)}>Cerrar</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .h1 { margin-top: 10px; }
  .bar { display: flex; align-items: baseline; gap: 18px; margin: 28px 0 6px; padding-bottom: 10px; border-bottom: 1px solid var(--ink); }
  .search { flex: 1; border: 0; background: none; font: inherit; font-size: 15px; color: var(--ink); padding: 4px 0; outline: none; }
  .search::placeholder { color: var(--ink-3); }
  .bar .num { font-size: 13px; }
  .bar small { color: var(--ink-3); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; align-items: center; gap: 16px; padding: 9px 2px; border-bottom: 1px solid var(--rule); animation: line 0.22s both; }
  .txt { flex: 1; min-width: 0; display: grid; gap: 1px; }
  .txt b { font-weight: 600; font-size: 14.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { color: var(--ink-3); font-size: 12.5px; }
  .cols { display: flex; align-items: center; gap: 16px; padding: 10px 2px 6px; font-size: 10px; }
  .col { border: 0; background: none; padding: 0; font: inherit; letter-spacing: inherit; text-transform: inherit; color: var(--ink-3); text-align: left; cursor: pointer; }
  .col:hover, .col.on { color: var(--ink); }
  .cols .nm { flex: 1; min-width: 0; }
  .use { display: flex; align-items: center; gap: 6px; }
  .ic { flex: none; width: 28px; height: 28px; display: grid; place-items: center; }
  .ic img { display: block; image-rendering: auto; }
  .ph { width: 28px; height: 28px; display: grid; place-items: center; background: var(--paper-2); border: 1px solid var(--rule); border-radius: 2px; font-family: var(--mono); font-size: 13px; color: var(--ink-2); }
  .dt { flex: none; width: 112px; white-space: nowrap; font-size: 12.5px; color: var(--ink-2); }
  .sz { flex: none; width: 74px; text-align: right; font-size: 12.5px; }
  .cols .sz { text-align: right; }
  .act { flex: none; width: 134px; display: flex; align-items: center; justify-content: flex-end; gap: 8px; }
  :global(.btn.sm) { padding: 5px 11px; font-size: 11px; }
  .empty { padding: 50px 0; color: var(--ink-2); font-size: 15px; }

  :global(.dialog.wide) { width: 520px; max-width: calc(100vw - 40px); }
  .dots { color: var(--ink-3); font-size: 28px; margin: 0; }
  .pt { display: flex; align-items: center; gap: 12px; margin: 0 0 18px; cursor: pointer; font-size: 13.5px; }
  .err { color: var(--accent); font-size: 13px; margin: 0 0 16px; }
  .lo { max-height: 260px; overflow-y: auto; margin: 0 0 20px; border-top: 1px solid var(--ink); }
  .lo li { padding: 0; }
  .lo label { display: flex; align-items: center; gap: 14px; padding: 10px 2px; cursor: pointer; width: 100%; }
  .lo .txt b { font-family: var(--mono); font-size: 12px; font-weight: 500; white-space: normal; word-break: break-all; }
  input[type='checkbox'] { position: absolute; opacity: 0; pointer-events: none; }
  .box { flex: none; position: relative; width: 16px; height: 16px; border: 1.5px solid var(--ink); border-radius: 2px; }
  input:checked + .box { background: var(--accent); }
  input:checked + .box::after { content: ''; position: absolute; left: 4px; top: 0; width: 4px; height: 9px; border: solid var(--on-accent); border-width: 0 2px 2px 0; transform: rotate(40deg); }
  input:focus-visible + .box { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
