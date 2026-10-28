<script>
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { fmt } from './format.js';
  import { squarify } from './treemap.js';

  let { notify } = $props();

  const palette = ['var(--blue)', 'var(--green)', 'var(--violet)', 'var(--yellow)', 'var(--red)'];
  const MAX_TILES = 12;

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

  let tiles = $derived.by(() => {
    if (!report) return [];
    const kids = report.children.filter((c) => c.bytes > 0);
    const shown = kids.slice(0, MAX_TILES).map((c) => ({ ...c, value: c.bytes }));
    const rest = kids.slice(MAX_TILES);
    if (rest.length) {
      const bytes = rest.reduce((a, c) => a + c.bytes, 0);
      shown.push({ name: `${rest.length} elementos más`, path: '', is_dir: false, bytes, files: 0, value: bytes, other: true });
    }
    return squarify(shown).map((t, i) => ({ ...t, color: t.other ? 'var(--ash)' : palette[i % palette.length], i }));
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
  const pct = (b) => (report?.bytes ? Math.round((b / report.bytes) * 100) : 0);
  const maxChild = $derived(report?.children[0]?.bytes || 1);
  const maxFile = $derived(report?.largest[0]?.bytes || 1);
  const baseName = (p) => p.split(/[\\/]/).pop();
  const dirName = (p) => p.slice(0, Math.max(0, p.lastIndexOf('\\')));

  init();
</script>

<header>
  <h1>Espacio</h1>
  <p>Descubre qué ocupa tu disco. Es solo lectura: aquí no se borra nada.</p>
</header>

<div class="roots">
  {#each roots as r (r.path)}
    <button class:active={current === r.path || current.startsWith(r.path)} onclick={() => go(r.path)}>{r.label}</button>
  {/each}
</div>

{#if current}
  <nav class="crumbs" aria-label="Ruta">
    {#each crumbs as c, i (c.path)}
      {#if i}<Icon name="chevron" size={13} />{/if}
      <button class:last={i === crumbs.length - 1} onclick={() => go(c.path)}>{c.name}</button>
    {/each}
  </nav>
{/if}

<section class="stats">
  {#if report}
    <div><b>{fmt(report.bytes)}</b><small>en total</small></div>
    <div><b>{report.files.toLocaleString('es-ES')}</b><small>archivos</small></div>
    <div><b>{report.children.length}</b><small>elementos</small></div>
    {#if report.skipped}<div class="warn"><b>{report.skipped}</b><small>sin acceso</small></div>{/if}
  {:else}
    <div><b class="sk">0.00 GB</b><small>en total</small></div>
    <div><b class="sk">00000</b><small>archivos</small></div>
  {/if}
</section>

<section class="map" aria-label="Mapa de espacio">
  {#if loading}
    <div class="scanning">
      <span class="spinner"></span>
      <b>Analizando…</b>
      <small>En discos grandes puede tardar un poco</small>
    </div>
  {:else if report && !tiles.length}
    <div class="scanning"><b>Carpeta vacía</b><small>No hay nada que ocupe espacio aquí.</small></div>
  {/if}
  {#each tiles as t (t.path || t.name)}
    <button
      class="tile"
      class:dim={hovered && hovered !== t.path}
      class:other={t.other}
      style="left:{t.x}%;top:{t.y}%;width:{t.w}%;height:{t.h}%;--c:{t.color};animation-delay:{t.i * 25}ms"
      disabled={t.other}
      onmouseenter={() => (hovered = t.path)}
      onmouseleave={() => (hovered = null)}
      onclick={() => (t.is_dir ? go(t.path) : reveal(t.path))}
      title="{t.name} · {fmt(t.bytes)}"
    >
      <span class="in">
        {#if t.w > 9 && t.h > 14}
          <span class="nm">
            <Icon name={t.is_dir ? 'folder' : 'file'} size={14} />
            <span>{t.name}</span>
          </span>
          <span class="sz">{fmt(t.bytes)}{#if t.w > 14 && t.h > 24}{' · ' + pct(t.bytes) + '%'}{/if}</span>
        {/if}
      </span>
    </button>
  {/each}
</section>

{#if report}
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === 'folders'} class:on={tab === 'folders'} onclick={() => (tab = 'folders')}>Contenido</button>
    <button role="tab" aria-selected={tab === 'files'} class:on={tab === 'files'} onclick={() => (tab = 'files')}>Archivos más grandes</button>
  </div>

  <ul>
    {#if tab === 'folders'}
      {#each report.children.slice(0, 40) as c, i (c.path)}
        <li style="animation-delay:{i * 18}ms" class:hot={hovered === c.path}>
          <button class="row" onclick={() => (c.is_dir ? go(c.path) : reveal(c.path))}>
            <span class="ico"><Icon name={c.is_dir ? 'folder' : 'file'} size={17} /></span>
            <span class="txt">
              <b>{c.name}</b>
              <span class="bar"><i style="width:{Math.max(c.bytes ? 2 : 0, (c.bytes / maxChild) * 100)}%"></i></span>
            </span>
            <span class="num">{fmt(c.bytes)}<small>{pct(c.bytes)}%</small></span>
            {#if c.is_dir}<span class="go"><Icon name="chevron" size={15} /></span>{/if}
          </button>
        </li>
      {/each}
    {:else}
      {#each report.largest as f, i (f.path)}
        <li style="animation-delay:{i * 18}ms">
          <div class="row">
            <span class="ico"><Icon name="file" size={17} /></span>
            <span class="txt">
              <b>{baseName(f.path)}</b>
              <small class="path">{dirName(f.path)}</small>
              <span class="bar"><i style="width:{(f.bytes / maxFile) * 100}%"></i></span>
            </span>
            <span class="num">{fmt(f.bytes)}</span>
            <button class="icon" title="Mostrar en el Explorador" aria-label="Mostrar en el Explorador" onclick={() => reveal(f.path)}><Icon name="external" size={15} /></button>
          </div>
        </li>
      {/each}
    {/if}
  </ul>
{/if}

<style>
  header h1 { margin: 0; font-size: 28px; font-weight: 600; letter-spacing: -0.3px; }
  header p { margin: 6px 0 0; color: var(--mute); font-size: 14px; }
  .roots { display: flex; flex-wrap: wrap; gap: 8px; margin: 22px 0 14px; }
  .roots button { border: 1px solid var(--hairline-strong); background: none; border-radius: 99px; padding: 7px 16px; color: var(--body); cursor: pointer; font-size: 13px; transition: all 0.15s; }
  .roots button:hover { background: var(--hairline); }
  .roots button.active { background: var(--cta-bg); color: var(--cta-fg); border-color: transparent; font-weight: 600; }

  .crumbs { display: flex; flex-wrap: wrap; align-items: center; gap: 2px; margin-bottom: 18px; color: var(--ash); font-size: 13px; }
  .crumbs button { border: 0; background: none; color: var(--mute); cursor: pointer; padding: 3px 6px; border-radius: 6px; }
  .crumbs button:hover { background: var(--hairline); color: var(--ink); }
  .crumbs button.last { color: var(--ink); font-weight: 500; }

  .stats { display: flex; gap: 34px; margin-bottom: 16px; }
  .stats div { display: grid; gap: 1px; }
  .stats b { font-size: 22px; font-weight: 600; letter-spacing: -0.4px; font-variant-numeric: tabular-nums; }
  .stats small { color: var(--mute); font-size: 12px; }
  .stats .warn b { color: var(--yellow); }
  .sk { color: transparent; background: var(--hairline); border-radius: 6px; animation: pulse 1.2s infinite; }
  @keyframes pulse { 50% { opacity: 0.45; } }

  .map { position: relative; height: 340px; border-radius: 18px; overflow: hidden; border: 1px solid var(--hairline); background: var(--surface); }
  .scanning { position: absolute; inset: 0; display: grid; place-content: center; justify-items: center; gap: 6px; text-align: center; color: var(--mute); }
  .scanning b { color: var(--ink); font-weight: 500; }
  .scanning small { font-size: 12.5px; }
  .spinner { width: 26px; height: 26px; border-radius: 50%; border: 3px solid var(--hairline-strong); border-top-color: var(--blue); animation: spin 0.8s linear infinite; margin-bottom: 8px; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .tile { position: absolute; padding: 3px; border: 0; background: none; text-align: left; cursor: pointer; animation: tileIn 0.5s cubic-bezier(0.2, 0.7, 0.2, 1) both; transition: opacity 0.2s; }
  @keyframes tileIn { from { opacity: 0; transform: scale(0.92); } }
  .tile.dim { opacity: 0.55; }
  .tile.other { cursor: default; }
  .in { position: relative; display: flex; flex-direction: column; justify-content: space-between; width: 100%; height: 100%; padding: 9px 11px; border-radius: 12px; overflow: hidden; color: var(--ink);
    background: linear-gradient(160deg, color-mix(in srgb, var(--c) 30%, var(--surface)), color-mix(in srgb, var(--c) 12%, var(--surface)));
    border: 1px solid color-mix(in srgb, var(--c) 40%, transparent); transition: transform 0.2s, border-color 0.2s, box-shadow 0.2s; }
  .tile:hover:not(:disabled) .in { transform: translateY(-2px); border-color: var(--c); box-shadow: 0 10px 28px color-mix(in srgb, var(--c) 28%, transparent); }
  .tile:focus-visible .in { outline: 2px solid var(--c); }
  .nm { display: flex; align-items: center; gap: 6px; font-size: 12.5px; font-weight: 500; min-width: 0; }
  .nm span:last-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nm :global(svg) { flex: none; color: var(--c); }
  .sz { font-size: 12px; color: var(--body); font-variant-numeric: tabular-nums; }

  .tabs { display: flex; gap: 4px; margin: 26px 0 10px; padding: 4px; width: fit-content; border-radius: 99px; background: var(--hairline); }
  .tabs button { border: 0; background: none; padding: 7px 16px; border-radius: 99px; color: var(--mute); cursor: pointer; font-size: 13px; transition: all 0.15s; }
  .tabs button.on { background: var(--elevated); color: var(--ink); box-shadow: 0 1px 6px rgba(0, 0, 0, 0.25); }

  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  li { animation: rise 0.4s cubic-bezier(0.2, 0.7, 0.2, 1) both; }
  @keyframes rise { from { opacity: 0; transform: translateY(8px); } }
  .row { display: flex; align-items: center; gap: 14px; width: 100%; padding: 11px 14px; border: 1px solid var(--hairline); border-radius: 12px; background: var(--surface); text-align: left; transition: border-color 0.15s, background 0.15s; }
  button.row { cursor: pointer; }
  .row:hover, li.hot .row { border-color: var(--hairline-strong); background: var(--elevated); }
  .ico { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 9px; color: var(--blue); background: var(--glow); flex: none; }
  .txt { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .txt b { font-weight: 500; font-size: 13.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .path { color: var(--ash); font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bar { height: 3px; border-radius: 3px; background: var(--hairline); margin-top: 5px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: linear-gradient(90deg, var(--blue), var(--green)); border-radius: 3px; }
  .num { display: grid; justify-items: end; min-width: 70px; font-size: 13px; font-variant-numeric: tabular-nums; color: var(--body); }
  .num small { color: var(--ash); font-size: 11px; }
  .go { color: var(--ash); display: grid; }
  .icon { display: grid; place-items: center; width: 32px; height: 32px; border: 0; border-radius: 9px; background: none; color: var(--mute); cursor: pointer; }
  .icon:hover { background: var(--hairline); color: var(--ink); }
</style>
