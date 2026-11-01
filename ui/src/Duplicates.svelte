<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { fmt } from './format.js';
  import { i18n } from './i18n.svelte.js';

  let { notify, refreshQuarantine, openQuarantine } = $props();

  let tab = $state('dups'); // dups | old
  let folders = $state([]);
  let chosen = $state({});
  let extra = $state([]); // carpetas elegidas a mano
  let keepRule = $state('oldest'); // oldest | newest | shortest
  let scanned = $state([]); // rutas que se recorrieron en la última búsqueda
  let minMb = $state(10);
  let days = $state(365);
  let phase = $state('idle'); // idle | scanning | done
  let confirming = $state(false);
  let working = $state(false);

  let groups = $state([]); // { bytes, files:[{path,modified}], keep, on }
  let report = $state(null);
  let old = $state([]);
  let picked = $state({});

  const DAY = (s) => (s ? new Date(s * 1000).toLocaleDateString(i18n.locale, { day: 'numeric', month: 'short', year: 'numeric' }).replace(/\./g, '') : '—');
  const name = (p) => p.split(/[\\/]/).pop();
  const dir = (p) => p.slice(0, Math.max(0, p.length - name(p).length - 1));

  let active = $derived(groups.filter((g) => g.on));
  let removeCount = $derived(active.reduce((a, g) => a + g.files.length - 1, 0));
  let removeBytes = $derived(active.reduce((a, g) => a + g.bytes * (g.files.length - 1), 0));
  let oldPicked = $derived(old.filter((f) => picked[f.path]));
  let oldBytes = $derived(oldPicked.reduce((a, f) => a + f.bytes, 0));
  let userFolders = $derived(folders.filter((f) => f.kind !== 'drive'));
  let drives = $derived(folders.filter((f) => f.kind === 'drive'));
  let targets = $derived([...folders.filter((f) => chosen[f.id]).map((f) => f.path), ...extra]);
  let anyFolder = $derived(targets.length > 0);
  let anyDrive = $derived(drives.some((d) => chosen[d.id]));

  async function init() {
    try {
      folders = await invoke('dup_folders');
      chosen = Object.fromEntries(folders.filter((f) => ['downloads', 'documents', 'desktop'].includes(f.id)).map((f) => [f.id, true]));
    } catch (e) {
      notify(t('No se pudieron leer las carpetas: ') + e);
    }
  }

  const ids = () => Object.keys(chosen).filter((k) => chosen[k]);

  async function addFolder() {
    try {
      const path = await open({ directory: true, multiple: false, title: t('Elige una carpeta donde buscar') });
      if (typeof path === 'string' && !extra.includes(path)) extra = [...extra, path];
    } catch (e) {
      notify(t('No se pudo abrir el selector de carpetas: ') + e);
    }
  }

  const removeFolder = (path) => (extra = extra.filter((p) => p !== path));

  // Qué copia de cada grupo se conserva según la regla elegida.
  function pickKeep(files) {
    let best = 0;
    files.forEach((f, i) => {
      const b = files[best];
      if (keepRule === 'oldest' ? f.modified < b.modified : keepRule === 'newest' ? f.modified > b.modified : f.path.length < b.path.length) best = i;
    });
    return best;
  }

  function applyRule() {
    for (const g of groups) g.keep = pickKeep(g.files);
  }

  async function search() {
    phase = 'scanning';
    scanned = targets;
    try {
      if (tab === 'dups') {
        report = await invoke('dup_scan', { folders: ids(), extra: [...extra], minMb });
        scanned = report.roots;
        groups = report.groups.map((g) => ({ ...g, keep: pickKeep(g.files), on: true }));
      } else {
        old = await invoke('old_scan', { folders: ids(), extra: [...extra], minMb, days });
        picked = {};
      }
      phase = 'done';
    } catch (e) {
      notify(t('No se pudo buscar: ') + e);
      phase = 'idle';
    }
  }

  function switchTab(which) {
    tab = which;
    phase = 'idle';
    minMb = which === 'dups' ? 10 : 500;
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
      notify(t('{n} copias a la cuarentena ({b})', { n: rep.files, b: fmt(rep.bytes) }) + (rep.errors.length ? ' · ' + t('{n} no se tocaron', { n: rep.errors.length }) : ''), {
        label: t('Ver cuarentena'),
        run: openQuarantine,
      });
      await refreshQuarantine();
      await search();
    } catch (e) {
      notify(t('No se pudo mover: ') + e);
    }
    working = false;
  }

  async function moveOld() {
    confirming = false;
    working = true;
    try {
      const rep = await invoke('space_quarantine', { paths: oldPicked.map((f) => f.path) });
      notify(t('{n} archivos a la cuarentena ({b})', { n: rep.files, b: fmt(rep.bytes) }) + (rep.errors.length ? ' · ' + t('{n} no se tocaron', { n: rep.errors.length }) : ''), {
        label: t('Ver cuarentena'),
        run: openQuarantine,
      });
      await refreshQuarantine();
      await search();
    } catch (e) {
      notify(t('No se pudo mover: ') + e);
    }
    working = false;
  }

  const reveal = (path) => invoke('reveal', { path }).catch((e) => notify(t('No se pudo abrir: ') + e));

  init();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (confirming = false)} />

<div class="sticky">
  <p class="label">{t('Duplicados')}</p>
  <h1 class="h1">{tab === 'dups' ? t('Lo mismo, más de una vez.') : t('Grande y olvidado.')}</h1>
  <p class="lede">
    {#if tab === 'dups'}
      {t('Busca archivos idénticos (se comparan por contenido, no por nombre) en tus carpetas. Eliges qué copia se queda y el resto va a la cuarentena.')}
    {:else}
      {t('Archivos pesados que llevan tiempo sin modificarse: instaladores, ISOs, vídeos viejos. Tú decides cuáles se van a la cuarentena.')}
    {/if}
    <Help text={tab === 'dups' ? H.dupInfo.text : H.oldInfo.text} risk="safe" label={t('Cómo funciona')} />
  </p>

  <div class="tabs">
    <button class:on={tab === 'dups'} onclick={() => switchTab('dups')}>{t('Duplicados')}</button>
    <button class:on={tab === 'old'} onclick={() => switchTab('old')}>{t('Grandes y antiguos')}</button>
  </div>

  <section class="setup">
    <div class="where">
      <div class="folders" role="group" aria-label={t('Carpetas a revisar')}>
        <span class="label grp">{t('Carpetas')}</span>
        {#each userFolders as f (f.id)}
          <label class="chk">
            <input type="checkbox" bind:checked={chosen[f.id]} />
            <span class="box" aria-hidden="true"></span>
            <span>{t(f.name)}</span>
          </label>
        {/each}
      </div>
      {#if drives.length}
        <div class="folders" role="group" aria-label={t('Unidades a revisar')}>
          <span class="label grp">{t('Unidades')}</span>
          {#each drives as d (d.id)}
            <label class="chk">
              <input type="checkbox" bind:checked={chosen[d.id]} />
              <span class="box" aria-hidden="true"></span>
              <span>{t('Disco')} {d.path.slice(0, 2)}</span>
            </label>
          {/each}
        </div>
      {/if}
      <div class="folders" role="group" aria-label={t('Otras carpetas')}>
        <span class="label grp">{t('Otras')}</span>
        {#each extra as p (p)}
          <span class="extra" title={p}>
            <span class="num">{p}</span>
            <button class="link" aria-label={t('Quitar')} onclick={() => removeFolder(p)}>×</button>
          </span>
        {/each}
        <button class="link" onclick={addFolder}>{t('+ Elegir carpeta…')}</button>
      </div>
    </div>
    <div class="opts">
      <label>
        {t('Tamaño mínimo')}
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
          {t('Sin tocar desde hace')}
          <select bind:value={days}>
            <option value={180}>{t('6 meses')}</option>
            <option value={365}>{t('1 año')}</option>
            <option value={730}>{t('2 años')}</option>
          </select>
        </label>
      {/if}
      <button class="btn primary" disabled={!anyFolder || phase === 'scanning'} onclick={search}>
        {phase === 'scanning' ? t('Buscando…') : phase === 'done' ? t('Buscar de nuevo') : t('Buscar')}
      </button>
    </div>
  </section>

  <p class="target" class:none={!anyFolder}>
    {#if phase === 'scanning'}
      <span class="label">{t('Buscando en')}</span>
    {:else if phase === 'done'}
      <span class="label">{t('Resultados de')}</span>
    {:else}
      <span class="label">{t('Se buscará en')}</span>
    {/if}
    {#if (phase === 'idle' ? targets : scanned).length}
      {#each phase === 'idle' ? targets : scanned as p (p)}<span class="num where-path">{p}</span>{/each}
    {:else}
      <span>{t('Elige al menos una carpeta o unidad.')}</span>
    {/if}
  </p>
  {#if anyDrive && phase !== 'done'}
    <p class="note">{t('Revisar una unidad entera puede tardar varios minutos. Windows, Archivos de programa y ProgramData nunca se revisan.')}</p>
  {/if}
</div>

{#if phase === 'scanning'}
  <p class="empty num">···</p>
{:else if phase === 'done' && tab === 'dups'}
  {#if !groups.length}
    <p class="empty">{t('No hay duplicados de ese tamaño en las carpetas elegidas.')} {report ? t('Se revisaron {n} archivos.', { n: report.scanned.toLocaleString(i18n.locale) }) : ''}</p>
  {:else}
    <section class="sum">
      <span class="big num">{fmt(removeBytes)}</span>
      <span class="cap">{t('recuperables · {g} grupos · {c} copias a quitar', { g: groups.length, c: removeCount })}</span>
      <button class="btn primary" disabled={!removeCount || working} onclick={() => (confirming = true)}>{t('Mover a la cuarentena')}</button>
    </section>
    <section class="rule">
      <label>
        {t('En cada grupo conservar')}
        <select bind:value={keepRule} onchange={applyRule}>
          <option value="oldest">{t('la copia más antigua')}</option>
          <option value="newest">{t('la copia más reciente')}</option>
          <option value="shortest">{t('la de ruta más corta')}</option>
        </select>
      </label>
      <p class="hint">
        <span class="badge keep">{t('Se conserva')}</span> {t('se queda donde está.')}
        <span class="badge out">{t('A la cuarentena')}</span> {t('se mueve a la cuarentena (se puede restaurar).')}
        {t('Pulsa en otra copia para conservar esa.')}
      </p>
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
            <span class="num meta">
              {fmt(g.bytes)} × {g.files.length} ·
              {g.on ? t('se conserva 1, {n} a la cuarentena', { n: g.files.length - 1 }) : t('no se toca nada')}
            </span>
          </div>
          {#each g.files as f, fi (f.path)}
            {@const kept = g.keep === fi}
            <div class="file" class:keep={kept} class:out={!kept && g.on}>
              <label class="pick" title={kept ? t('Esta copia se conserva') : t('Conservar esta copia en lugar de la otra')}>
                <input type="radio" name={'g' + gi} checked={kept} onchange={() => (g.keep = fi)} />
                <span class="rb" aria-hidden="true"></span>
                <span class="badge" class:keep={kept} class:out={!kept && g.on}>
                  {kept ? t('Se conserva') : g.on ? t('A la cuarentena') : t('No se toca')}
                </span>
              </label>
              <span class="path" title={f.path}><b>{name(f.path)}</b> <small>{dir(f.path)}</small></span>
              <span class="num dt">{DAY(f.modified)}</span>
              <button class="link" onclick={() => reveal(f.path)}>{t('Mostrar')}</button>
            </div>
          {/each}
        </li>
      {/each}
    </ul>
  {/if}
{:else if phase === 'done'}
  {#if !old.length}
    <p class="empty">{t('No hay archivos tan grandes y tan antiguos en las carpetas elegidas.')}</p>
  {:else}
    <section class="sum">
      <span class="big num">{fmt(oldBytes)}</span>
      <span class="cap">{t('seleccionados · {a} de {b}', { a: oldPicked.length, b: old.length })}</span>
      <button class="btn primary" disabled={!oldPicked.length || working} onclick={() => (confirming = true)}>{t('Mover a la cuarentena')}</button>
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
          <button class="link" onclick={() => reveal(f.path)}>{t('Mostrar')}</button>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

{#if confirming}
  <div class="overlay" role="presentation" onclick={() => (confirming = false)}>
    <div class="dialog" role="dialog" aria-modal="true" aria-label={t('Confirmar')} tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <h3>{t('¿Mover a la cuarentena?')}</h3>
      <p>
        {#if tab === 'dups'}
          {t('{n} copias ({b}) irán a la cuarentena. De cada grupo se conserva la copia marcada «Se conserva». Puedes restaurarlas desde Cuarentena.', { n: removeCount, b: fmt(removeBytes) })}
        {:else}
          {t('{n} archivos ({b}) irán a la cuarentena. Puedes restaurarlos desde Cuarentena.', { n: oldPicked.length, b: fmt(oldBytes) })}
        {/if}
      </p>
      <div class="row">
        <button class="btn" onclick={() => (confirming = false)}>{t('Cancelar')}</button>
        <button class="btn primary" onclick={tab === 'dups' ? moveDuplicates : moveOld}>{t('Mover')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tabs { margin-top: 22px; }
  .setup { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 16px 24px; margin: 18px 0 8px; }
  .where { display: grid; gap: 10px; min-width: 0; }
  .folders { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 20px; }
  .grp { width: 72px; flex: none; font-size: 10px; color: var(--ink-3); }
  .extra { display: inline-flex; align-items: center; gap: 6px; max-width: 340px; padding: 3px 4px 3px 8px; border: 1px solid var(--rule); border-radius: 2px; font-size: 12px; }
  .extra .num { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .target { display: flex; flex-wrap: wrap; align-items: baseline; gap: 6px 10px; margin: 6px 0 0; font-size: 12.5px; color: var(--ink-2); }
  .target .label { font-size: 10px; color: var(--ink-3); }
  .target.none { color: var(--accent); }
  .where-path { padding: 1px 6px; background: var(--paper-2); border-radius: 2px; color: var(--ink); }
  .note { margin: 6px 0 0; font-size: 12px; color: var(--ink-3); }
  .rule { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 10px 24px; margin: 6px 0 4px; }
  .rule label { display: grid; gap: 4px; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-3); }
  .hint { flex: 1; min-width: 260px; margin: 0; font-size: 12.5px; color: var(--ink-2); line-height: 1.9; }
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
  .pick { flex: none; display: flex; align-items: center; gap: 9px; width: 150px; cursor: pointer; }
  .rb { flex: none; position: relative; width: 14px; height: 14px; border: 1.5px solid var(--ink); border-radius: 50%; }
  input:checked + .rb { background: var(--accent); box-shadow: inset 0 0 0 2px var(--paper); }
  .badge { display: inline-block; padding: 2px 6px; border: 1px solid var(--rule); border-radius: 2px; font-family: var(--mono); font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-3); white-space: nowrap; }
  .badge.keep { background: var(--accent); border-color: var(--accent); color: var(--on-accent); font-weight: 600; }
  .badge.out { border-color: var(--ink-3); color: var(--ink-2); }
  .file.keep { box-shadow: inset 3px 0 0 var(--accent); padding-left: 8px; }
  .file.out .path { opacity: 0.65; }
  .file.out .path b { text-decoration: line-through; text-decoration-color: var(--ink-3); }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-2); }
  .path b { font-weight: 600; color: var(--ink); }
  .path small { font-size: 12px; color: var(--ink-3); }
  .dt { flex: none; color: var(--ink-3); font-size: 12px; width: 96px; text-align: right; }

  .old > li { display: flex; align-items: center; gap: 14px; padding: 10px 2px; border-bottom: 1px solid var(--rule); animation: line 0.2s both; }
  .old .chk { flex: 1; }
  .txt { display: grid; gap: 1px; min-width: 0; }
  .txt b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .txt small { color: var(--ink-3); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sz { flex: none; width: 82px; text-align: right; }
</style>
