<script>
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';
  import { fmt } from './format.js';
  import { squarify } from './treemap.js';

  let { notify } = $props();

  const MAX_TILES = 14;

  let roots = $state([]);
  let current = $state('');
  let report = $state(null);
  let loading = $state(false);
  let tab = $state('folders');
  let hovered = $state(null);
  const cache = new Map();
  let seq = 0;

  let crumbs = $derived(
    current
      .split(/[\\/]+/)
      .filter(Boolean)
      .map((name, i, all) => ({
        name,
        path: all.slice(0, i + 1).join('\\') + (i === 0 && name.endsWith(':') ? '\\' : ''),
      }))
  );

  // La raíz activa es la más específica que contiene la ruta actual (Tu carpeta antes que Disco C:).
  let activeRoot = $derived(
    roots.filter((r) => current.startsWith(r.path)).sort((a, b) => b.path.length - a.path.length)[0]?.path
  );

  let tiles = $derived.by(() => {
    if (!report) return [];
    const kids = report.children.filter((c) => c.bytes > 0);
    const shown = kids.slice(0, MAX_TILES).map((c) => ({ ...c, value: c.bytes }));
    const rest = kids.slice(MAX_TILES);
    if (rest.length) {
      const bytes = rest.reduce((a, c) => a + c.bytes, 0);
      shown.push({ name: `${rest.length} elementos más`, path: '', is_dir: false, bytes, files: 0, value: bytes, other: true });
    }
    return squarify(shown).map((t, i) => ({ ...t, i }));
  });

  async function go(path) {
    current = path;
    hovered = null;
    const id = ++seq;
    if (cache.has(path)) {
      report = cache.get(path);
      return;
    }
    loading = true;
    report = null;
    try {
      const r = await invoke('analyze', { path });
      cache.set(path, r);
      if (id === seq) report = r;
    } catch (e) {
      if (id === seq) notify('No se pudo analizar: ' + e);
    }
    if (id === seq) loading = false;
  }

  async function init() {
    try {
      roots = await invoke('disk_roots');
      if (roots.length) await go(roots[0].path);
    } catch (e) {
      notify('No se pudieron leer los discos: ' + e);
    }
  }

  const reveal = (path) => invoke('reveal', { path }).catch((e) => notify('No se pudo abrir: ' + e));
  // Cada bloque se tiñe con una tinta distinta; el mayor, con el acento.
  const tone = (t) =>
    t.other
      ? 'var(--paper-2)'
      : t.i === 0
        ? 'color-mix(in srgb, var(--accent) 30%, var(--paper))'
        : `color-mix(in srgb, var(--ink) ${[7, 12, 17][t.i % 3]}%, var(--paper))`;
  const pct = (b) => (report?.bytes ? Math.round((b / report.bytes) * 100) : 0);
  const maxChild = $derived(report?.children[0]?.bytes || 1);
  const maxFile = $derived(report?.largest[0]?.bytes || 1);
  const baseName = (p) => p.split(/[\\/]/).pop();
  const dirName = (p) => p.slice(0, Math.max(0, p.lastIndexOf('\\')));

  init();
</script>

<p class="label">Espacio</p>
<h1 class="h1">Qué ocupa tu disco.</h1>
<p class="lede">Solo lectura: aquí no se borra nada. Haz clic en un bloque para entrar. <Help text={H.spaceMap.text} risk={H.spaceMap.risk} label="Cómo leer el mapa" /></p>

<div class="roots tabs">
  {#each roots as r (r.path)}
    <button class:on={activeRoot === r.path} onclick={() => go(r.path)}>{r.label}</button>
  {/each}
</div>

{#if current}
  <nav class="crumbs num" aria-label="Ruta">
    {#each crumbs as c, i (c.path)}
      {#if i}<span>/</span>{/if}
      <button class:last={i === crumbs.length - 1} onclick={() => go(c.path)}>{c.name}</button>
    {/each}
  </nav>
{/if}

<section class="stats num">
  {#if report}
    <span><b>{fmt(report.bytes)}</b> en total</span>
    <span><b>{report.files.toLocaleString('es-ES')}</b> archivos</span>
    <span><b>{report.children.length}</b> elementos</span>
    {#if report.skipped}<span class="warn"><b>{report.skipped}</b> sin acceso <Help text={H.spaceSkipped.text} risk={H.spaceSkipped.risk} label="Qué es sin acceso" /></span>{/if}
  {:else}
    <span>—</span>
  {/if}
</section>

<section class="map" aria-label="Mapa de espacio">
  {#if loading}
    <div class="scanning"><b class="num dots">···</b><span>Analizando. En discos grandes puede tardar.</span></div>
  {:else if report && !tiles.length}
    <div class="scanning"><span>Carpeta vacía.</span></div>
  {/if}
  {#each tiles as t (t.path || t.name)}
    <button
      class="tile"
      class:dim={hovered && hovered !== t.path}
      style="left:{t.x}%;top:{t.y}%;width:{t.w}%;height:{t.h}%;--bg:{tone(t)};animation-delay:{t.i * 20}ms"
      disabled={t.other}
      onmouseenter={() => (hovered = t.path)}
      onmouseleave={() => (hovered = null)}
      onclick={() => (t.is_dir ? go(t.path) : reveal(t.path))}
      title="{t.name} · {fmt(t.bytes)}"
    >
      <span class="in">
        {#if t.w > 9 && t.h > 14}
          <span class="nm">{t.name}{t.is_dir ? '/' : ''}</span>
          <span class="sz num">{fmt(t.bytes)}{#if t.w > 14 && t.h > 24}{' · ' + pct(t.bytes) + '%'}{/if}</span>
        {/if}
      </span>
    </button>
  {/each}
</section>

{#if report}
  <div class="tabs sub" role="tablist">
    <button role="tab" aria-selected={tab === 'folders'} class:on={tab === 'folders'} onclick={() => (tab = 'folders')}>Contenido</button>
    <button role="tab" aria-selected={tab === 'files'} class:on={tab === 'files'} onclick={() => (tab = 'files')}>Archivos más grandes</button>
  </div>

  <ul>
    {#if tab === 'folders'}
      {#each report.children.slice(0, 40) as c, i (c.path)}
        <li style="animation-delay:{i * 14}ms" class:hot={hovered === c.path}>
          <button class="row" onclick={() => (c.is_dir ? go(c.path) : reveal(c.path))}>
            <span class="nm">{c.name}{c.is_dir ? '/' : ''}</span>
            <span class="mini" aria-hidden="true"><i style="width:{Math.max(c.bytes ? 2 : 0, (c.bytes / maxChild) * 100)}%"></i></span>
            <span class="pc num">{pct(c.bytes)}%</span>
            <span class="sz num">{fmt(c.bytes)}</span>
          </button>
        </li>
      {/each}
    {:else}
      {#each report.largest as f, i (f.path)}
        <li style="animation-delay:{i * 14}ms">
          <div class="row">
            <span class="nm">
              {baseName(f.path)}
              <small>{dirName(f.path)}</small>
            </span>
            <span class="mini" aria-hidden="true"><i style="width:{(f.bytes / maxFile) * 100}%"></i></span>
            <span class="sz num">{fmt(f.bytes)}</span>
            <button class="link" onclick={() => reveal(f.path)}>Mostrar ↗</button>
            <Help text={H.spaceLargest.text} risk={H.spaceLargest.risk} label="Qué hace Mostrar" />
          </div>
        </li>
      {/each}
    {/if}
  </ul>
{/if}

<style>
  .h1 { margin-top: 10px; }
  .roots { margin: 26px 0 14px; }
  .crumbs { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px; margin-bottom: 16px; font-size: 12px; color: var(--ink-3); }
  .crumbs button { border: 0; background: none; padding: 1px 2px; color: var(--ink-2); }
  .crumbs button:hover { color: var(--accent); }
  .crumbs button.last { color: var(--ink); font-weight: 600; }
  .stats { display: flex; flex-wrap: wrap; gap: 24px; margin-bottom: 14px; font-size: 12px; color: var(--ink-3); }
  .stats b { color: var(--ink); font-weight: 600; font-size: 15px; margin-right: 3px; }
  .stats .warn b { color: var(--accent); }

  .map { position: relative; height: 330px; background: var(--paper-2); }
  .scanning { position: absolute; inset: 0; display: grid; place-content: center; justify-items: center; gap: 6px; color: var(--ink-2); }
  .dots { font-size: 40px; color: var(--ink-3); animation: blink 1s steps(3) infinite; }
  @keyframes blink { 50% { opacity: 0.3; } }
  .tile { position: absolute; padding: 1.5px; border: 0; background: none; text-align: left; animation: fade 0.3s both; transition: opacity 0.15s; }
  .tile.dim { opacity: 0.5; }
  .tile:disabled { cursor: default; }
  .in { display: flex; flex-direction: column; justify-content: space-between; width: 100%; height: 100%; padding: 8px 10px; background: var(--bg); overflow: hidden; }
  .tile:hover:not(:disabled) .in { box-shadow: inset 0 0 0 2px var(--ink); }
  .tile:focus-visible .in { box-shadow: inset 0 0 0 2px var(--accent); }
  .tile .nm { font-size: 12.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tile .sz { width: auto; text-align: left; font-size: 11.5px; color: var(--ink-2); }

  .sub { margin: 28px 0 4px; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { animation: line 0.22s both; border-bottom: 1px solid var(--rule); }
  .row { display: flex; align-items: center; gap: 18px; width: 100%; padding: 11px 2px; border: 0; background: none; text-align: left; }
  button.row:hover, li.hot .row { background: var(--paper-2); }
  .row .nm { flex: 1; min-width: 0; font-size: 14px; display: grid; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row .nm small { color: var(--ink-3); font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; }
  .mini { flex: none; width: 90px; height: 3px; background: var(--paper-2); }
  .mini i { display: block; height: 100%; background: var(--ink); }
  li:first-child .mini i { background: var(--accent); }
  .pc { width: 38px; text-align: right; font-size: 12px; color: var(--ink-3); }
  .sz { width: 78px; text-align: right; font-size: 13px; }
</style>
