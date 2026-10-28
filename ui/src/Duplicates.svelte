<script>
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';
  import { fmt } from './format.js';

  let { notify, refreshQuarantine, openQuarantine } = $props();

  let tab = $state('dups'); // dups | old
  let folders = $state([]);
  let chosen = $state({});
  let minMb = $state(10);
  let days = $state(365);
  let phase = $state('idle'); // idle | scanning | done
  let confirming = $state(false);
  let working = $state(false);

  let groups = $state([]); // { bytes, files:[{path,modified}], keep, on }
  let report = $state(null);
  let old = $state([]);
  let picked = $state({});

  const DAY = (s) => (s ? new Date(s * 1000).toLocaleDateString('es-ES', { day: 'numeric', month: 'short', year: 'numeric' }).replace(/\./g, '') : '—');
  const name = (p) => p.split(/[\\/]/).pop();
  const dir = (p) => p.slice(0, Math.max(0, p.length - name(p).length - 1));

  let active = $derived(groups.filter((g) => g.on));
  let removeCount = $derived(active.reduce((a, g) => a + g.files.length - 1, 0));
  let removeBytes = $derived(active.reduce((a, g) => a + g.bytes * (g.files.length - 1), 0));
  let oldPicked = $derived(old.filter((f) => picked[f.path]));
  let oldBytes = $derived(oldPicked.reduce((a, f) => a + f.bytes, 0));
  let anyFolder = $derived(Object.values(chosen).some(Boolean));

  async function init() {
    try {
      folders = await invoke('dup_folders');
      chosen = Object.fromEntries(folders.filter((f) => ['downloads', 'documents', 'desktop'].includes(f.id)).map((f) => [f.id, true]));
    } catch (e) {
      notify('No se pudieron leer las carpetas: ' + e);
    }
  }

  const ids = () => Object.keys(chosen).filter((k) => chosen[k]);

  async function search() {
    phase = 'scanning';
    try {
      if (tab === 'dups') {
        report = await invoke('dup_scan', { folders: ids(), minMb });
        groups = report.groups.map((g) => ({ ...g, keep: 0, on: true }));
      } else {
        old = await invoke('old_scan', { folders: ids(), minMb, days });
        picked = {};
      }
      phase = 'done';
    } catch (e) {
      notify('No se pudo buscar: ' + e);
      phase = 'idle';
    }
  }

  function switchTab(t) {
    tab = t;
    phase = 'idle';
    minMb = t === 'dups' ? 10 : 500;
  }

  async function moveDuplicates() {
    confirming = false;
    working = true;
    try {
      const selection = active.map((g) => ({
        keep: g.files[g.keep].path,
        remove: g.files.filter((_, i) => i !== g.keep).map((f) => f.path),
      }));
      const rep = await invoke('dup_quarantine', { selection });
      notify(`${rep.files} copias a la cuarentena (${fmt(rep.bytes)})` + (rep.errors.length ? ` · ${rep.errors.length} no se tocaron` : ''), {
        label: 'Ver cuarentena',
        run: openQuarantine,
      });
      await refreshQuarantine();
      await search();
    } catch (e) {
      notify('No se pudo mover: ' + e);
    }
    working = false;
  }

  async function moveOld() {
    confirming = false;
    working = true;
    try {
      const rep = await invoke('space_quarantine', { paths: oldPicked.map((f) => f.path) });
      notify(`${rep.files} archivos a la cuarentena (${fmt(rep.bytes)})` + (rep.errors.length ? ` · ${rep.errors.length} no se tocaron` : ''), {
        label: 'Ver cuarentena',
        run: openQuarantine,
      });
      await refreshQuarantine();
      await search();
    } catch (e) {
      notify('No se pudo mover: ' + e);
    }
    working = false;
  }

  const reveal = (path) => invoke('reveal', { path }).catch((e) => notify('No se pudo abrir: ' + e));

  init();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (confirming = false)} />

<p class="label">Duplicados</p>
<h1 class="h1">{tab === 'dups' ? 'Lo mismo, más de una vez.' : 'Grande y olvidado.'}</h1>
<p class="lede">
  {#if tab === 'dups'}
    Busca archivos idénticos (se comparan por contenido, no por nombre) en tus carpetas. Eliges qué copia se queda y el resto va a la cuarentena.
  {:else}
    Archivos pesados que llevan tiempo sin modificarse: instaladores, ISOs, vídeos viejos. Tú decides cuáles se van a la cuarentena.
  {/if}
  <Help text={tab === 'dups' ? H.dupInfo.text : H.oldInfo.text} risk="safe" label="Cómo funciona" />
</p>

<div class="tabs">
  <button class:on={tab === 'dups'} onclick={() => switchTab('dups')}>Duplicados</button>
  <button class:on={tab === 'old'} onclick={() => switchTab('old')}>Grandes y antiguos</button>
</div>

<section class="setup">
  <div class="folders" role="group" aria-label="Carpetas a revisar">
    {#each folders as f (f.id)}
      <label class="chk">
        <input type="checkbox" bind:checked={chosen[f.id]} />
        <span class="box" aria-hidden="true"></span>
        <span>{f.name}</span>
      </label>
    {/each}
  </div>
  <div class="opts">
    <label>
      Tamaño mínimo
      <select bind:value={minMb}>
        {#if tab === 'dups'}
          <option value={1}>1 MB</option>
          <option value={10}>10 MB</option>
          <option value={100}>100 MB</option>
        {:else}
          <option value={100}>100 MB</option>
          <option value={500}>500 MB</option>
          <option value={1000}>1 GB</option>
        {/if}
      </select>
    </label>
    {#if tab === 'old'}
      <label>
        Sin tocar desde hace
        <select bind:value={days}>
          <option value={180}>6 meses</option>
          <option value={365}>1 año</option>
          <option value={730}>2 años</option>
        </select>
      </label>
    {/if}
    <button class="btn primary" disabled={!anyFolder || phase === 'scanning'} onclick={search}>
      {phase === 'scanning' ? 'Buscando…' : phase === 'done' ? 'Buscar de nuevo' : 'Buscar'}
    </button>
  </div>
</section>

{#if phase === 'scanning'}
  <p class="empty num">···</p>
{:else if phase === 'done' && tab === 'dups'}
  {#if !groups.length}
    <p class="empty">No hay duplicados de ese tamaño en las carpetas elegidas. {report ? `Se revisaron ${report.scanned.toLocaleString('es-ES')} archivos.` : ''}</p>
  {:else}
    <section class="sum">
      <span class="big num">{fmt(removeBytes)}</span>
      <span class="cap">recuperables · {groups.length} grupos · {removeCount} copias a quitar</span>
      <button class="btn primary" disabled={!removeCount || working} onclick={() => (confirming = true)}>Mover a la cuarentena</button>
    </section>
    <ul class="groups">
      {#each groups as g, gi (gi)}
        <li class:off={!g.on}>
          <div class="gh">
            <label class="chk">
              <input type="checkbox" bind:checked={g.on} />
              <span class="box" aria-hidden="true"></span>
              <b>{name(g.files[0].path)}</b>
            </label>
            <span class="num meta">{fmt(g.bytes)} × {g.files.length}</span>
          </div>
          {#each g.files as f, fi (f.path)}
            <div class="file" class:keep={g.keep === fi}>
              <label class="pick">
                <input type="radio" name={'g' + gi} checked={g.keep === fi} onchange={() => (g.keep = fi)} />
                <span class="rb" aria-hidden="true"></span>
                <span class="tag label">{g.keep === fi ? 'Se queda' : 'Se quita'}</span>
              </label>
              <span class="path" title={f.path}>{f.path}</span>
              <span class="num dt">{DAY(f.modified)}</span>
              <button class="link" onclick={() => reveal(f.path)}>Mostrar</button>
            </div>
          {/each}
        </li>
      {/each}
    </ul>
  {/if}
{:else if phase === 'done'}
  {#if !old.length}
    <p class="empty">No hay archivos tan grandes y tan antiguos en las carpetas elegidas.</p>
  {:else}
    <section class="sum">
      <span class="big num">{fmt(oldBytes)}</span>
      <span class="cap">seleccionados · {oldPicked.length} de {old.length}</span>
      <button class="btn primary" disabled={!oldPicked.length || working} onclick={() => (confirming = true)}>Mover a la cuarentena</button>
    </section>
    <ul class="old">
      {#each old as f (f.path)}
        <li>
          <label class="chk">
            <input type="checkbox" checked={!!picked[f.path]} onchange={(e) => (picked[f.path] = e.currentTarget.checked)} />
            <span class="box" aria-hidden="true"></span>
            <span class="txt"><b title={f.path}>{name(f.path)}</b><small>{dir(f.path)}</small></span>
          </label>
          <span class="num dt">{DAY(f.modified)}</span>
          <span class="num sz">{fmt(f.bytes)}</span>
          <button class="link" onclick={() => reveal(f.path)}>Mostrar</button>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

{#if confirming}
  <div class="overlay" role="presentation" onclick={() => (confirming = false)}>
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Confirmar" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <h3>¿Mover a la cuarentena?</h3>
      <p>
        {#if tab === 'dups'}
          {removeCount} copias ({fmt(removeBytes)}) irán a la cuarentena. De cada grupo se conserva la copia marcada «Se queda». Puedes restaurarlas desde Cuarentena.
        {:else}
          {oldPicked.length} archivos ({fmt(oldBytes)}) irán a la cuarentena. Puedes restaurarlos desde Cuarentena.
        {/if}
      </p>
      <div class="row">
        <button class="btn" onclick={() => (confirming = false)}>Cancelar</button>
        <button class="btn primary" onclick={tab === 'dups' ? moveDuplicates : moveOld}>Mover</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tabs { margin-top: 22px; }
  .setup { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 16px 24px; margin: 18px 0 8px; }
  .folders { display: flex; flex-wrap: wrap; gap: 8px 20px; }
  .opts { display: flex; align-items: flex-end; gap: 16px; }
  .opts label { display: grid; gap: 4px; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-3); }
  select { border: 1px solid var(--ink); border-radius: 2px; background: var(--paper); color: var(--ink); padding: 8px 10px; font-family: var(--mono); font-size: 12px; }
  .chk { display: flex; align-items: center; gap: 10px; cursor: pointer; font-size: 13.5px; min-width: 0; }
  .chk input, .pick input { position: absolute; opacity: 0; pointer-events: none; }
  .box { flex: none; position: relative; width: 16px; height: 16px; border: 1.5px solid var(--ink); border-radius: 2px; }
  input:checked + .box { background: var(--accent); }
  input:checked + .box::after { content: ''; position: absolute; left: 4px; top: 0; width: 4px; height: 9px; border: solid var(--on-accent); border-width: 0 2px 2px 0; transform: rotate(40deg); }
  input:focus-visible + .box, input:focus-visible + .rb { outline: 2px solid var(--accent); outline-offset: 2px; }
  .empty { padding: 44px 0; color: var(--ink-2); font-size: 14.5px; line-height: 1.55; max-width: 520px; }

  .sum { display: flex; align-items: baseline; flex-wrap: wrap; gap: 8px 14px; margin: 22px 0 8px; }
  .big { font-size: 44px; font-weight: 500; letter-spacing: -0.05em; line-height: 1; }
  .cap { flex: 1; color: var(--ink-2); min-width: 200px; }

  ul { list-style: none; margin: 0; padding: 0; }
  .groups > li { margin-top: 16px; border-top: 1px solid var(--ink); animation: line 0.2s both; }
  .groups > li.off { opacity: 0.5; }
  .gh { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 2px 6px; }
  .gh b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { color: var(--ink-3); font-size: 12px; flex: none; }
  .file { display: flex; align-items: center; gap: 14px; padding: 7px 2px; border-top: 1px solid var(--rule); font-size: 12.5px; }
  .pick { flex: none; display: flex; align-items: center; gap: 9px; width: 104px; cursor: pointer; }
  .rb { flex: none; position: relative; width: 14px; height: 14px; border: 1.5px solid var(--ink); border-radius: 50%; }
  input:checked + .rb { background: var(--accent); box-shadow: inset 0 0 0 2px var(--paper); }
  .tag { font-size: 10px; }
  .file.keep .tag { color: var(--accent); font-weight: 600; }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-2); direction: rtl; text-align: left; }
  .dt { flex: none; color: var(--ink-3); font-size: 12px; width: 96px; text-align: right; }

  .old > li { display: flex; align-items: center; gap: 14px; padding: 10px 2px; border-bottom: 1px solid var(--rule); animation: line 0.2s both; }
  .old .chk { flex: 1; }
  .txt { display: grid; gap: 1px; min-width: 0; }
  .txt b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .txt small { color: var(--ink-3); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sz { flex: none; width: 82px; text-align: right; }
</style>
