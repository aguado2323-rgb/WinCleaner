<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { fmt } from './format.js';
  import { t } from './i18n.svelte.js';

  const MIN_WIDTH = 1500; // por debajo no cabe junto a la pantalla: el panel se oculta y no consulta nada
  const HISTORY = 40;

  let visible = $state(false);
  let specs = $state(null);
  let live = $state(null);
  let history = $state([]);
  let netHistory = $state([]);
  let failed = $state(false);

  const pct = (a, b) => (b ? Math.min(100, Math.round((a / b) * 100)) : 0);
  const tone = (p) => (p >= 90 ? 'hot' : p >= 75 ? 'warm' : '');

  function uptime(s) {
    const d = Math.floor(s / 86400);
    const h = Math.floor((s % 86400) / 3600);
    const m = Math.floor((s % 3600) / 60);
    return d ? `${d} ${t('d')} ${h} h` : h ? `${h} h ${m} min` : `${m} min`;
  }

  async function sample() {
    try {
      live = await invoke('monitor_sample');
      history = [...history, live.cpu].slice(-HISTORY);
      netHistory = [...netHistory, live.net_down].slice(-HISTORY);
      failed = false;
    } catch {
      failed = true;
    }
  }

  async function loadSpecs() {
    try {
      specs = await invoke('monitor_specs');
    } catch {
      /* sin especificaciones: el resto del panel sigue funcionando */
    }
  }

  onMount(() => {
    let timer;
    // Se decide con el ancho real de la ventana en cada cambio de tamaño: los eventos de media query
    // no siempre llegan en WebView2 y el panel se quedaba oculto hasta cambiar de pantalla.
    const sync = () => {
      const want = window.innerWidth >= MIN_WIDTH;
      if (want === visible && (timer || !want)) return;
      visible = want;
      clearInterval(timer);
      timer = undefined;
      if (want) {
        if (!specs) loadSpecs();
        sample();
        timer = setInterval(sample, 2000);
      }
    };
    sync();
    window.addEventListener('resize', sync);
    return () => {
      clearInterval(timer);
      window.removeEventListener('resize', sync);
    };
  });

  let netMax = $derived(Math.max(1, ...netHistory));
  const rate = (b) => fmt(b) + '/s';
  const healthy = (h) => /^(healthy|ok)$/i.test(h);
  let ramPct = $derived(live ? pct(live.ram_used, live.ram_total) : 0);
  let cpuPct = $derived(live ? Math.round(live.cpu) : 0);
</script>

{#if visible}
  <aside class="mon" aria-label={t('Monitor del equipo')}>
    <p class="label">{t('Monitor')}</p>

    {#if failed && !live}
      <p class="note">{t('No se pudo leer el estado del equipo.')}</p>
    {:else if live}
      <section>
        <div class="row"><span class="k">CPU</span><b class="num {tone(cpuPct)}">{cpuPct}%</b></div>
        <div class="spark" role="img" aria-label={t('Uso de CPU en los últimos segundos')}>
          {#each history as h, i (i)}<i style="height:{Math.max(3, h)}%"></i>{/each}
        </div>
      </section>

      <section>
        <div class="row"><span class="k">RAM</span><b class="num {tone(ramPct)}">{ramPct}%</b></div>
        <div class="bar"><i class={tone(ramPct)} style="width:{ramPct}%"></i></div>
        <p class="sub num">{fmt(live.ram_used)} / {fmt(live.ram_total)}</p>
      </section>

      <section>
        <p class="k">{t('Discos')}</p>
        {#each live.disks as d (d.mount)}
          {@const used = d.total - d.free}
          {@const p = pct(used, d.total)}
          <div class="disk">
            <div class="row"><span class="num">{d.mount.replace(/[\\/]+$/, '')}{d.name ? ' · ' + d.name : ''}</span><span class="num sub">{p}%</span></div>
            <div class="bar"><i class={tone(p)} style="width:{p}%"></i></div>
            <p class="sub num">{t('{a} libres de {b}', { a: fmt(d.free), b: fmt(d.total) })}</p>
          </div>
        {/each}
      </section>

      <section>
        <p class="k">{t('Red')}</p>
        <div class="row"><span class="sub">↓ {t('Bajada')}</span><b class="num small">{rate(live.net_down)}</b></div>
        <div class="row"><span class="sub">↑ {t('Subida')}</span><b class="num small">{rate(live.net_up)}</b></div>
        <div class="spark thin" role="img" aria-label={t('Velocidad de bajada en los últimos segundos')}>
          {#each netHistory as h, i (i)}<i style="height:{Math.max(3, (h / netMax) * 100)}%"></i>{/each}
        </div>
      </section>

      {#if live.gpu.length}
        <section>
          <p class="k">GPU</p>
          {#each live.gpu as g (g.name)}
            {@const gp = pct(g.mem_used, g.mem_total)}
            <div class="row"><span class="sub">{g.name}</span><b class="num small {tone(Math.round(g.util))}">{Math.round(g.util)}%</b></div>
            <div class="bar"><i class={tone(Math.round(g.util))} style="width:{Math.round(g.util)}%"></i></div>
            {#if g.mem_total}<p class="sub num">{t('VRAM {a} / {b}', { a: fmt(g.mem_used), b: fmt(g.mem_total) })} ({gp}%)</p>{/if}
          {/each}
        </section>
      {/if}

      <section>
        <p class="k">{t('Programas que más gastan')}</p>
        {#if live.top_cpu.length}
          <p class="sub">CPU</p>
          {#each live.top_cpu as h (h.name)}
            <div class="row"><span class="nm" title={h.name}>{h.name}</span><span class="num sub">{h.cpu.toFixed(h.cpu < 10 ? 1 : 0)}%</span></div>
          {/each}
        {/if}
        <p class="sub">RAM</p>
        {#each live.top_mem as h (h.name)}
          <div class="row"><span class="nm" title={h.name}>{h.name}</span><span class="num sub">{fmt(h.mem)}</span></div>
        {/each}
      </section>

      <section>
        <p class="k">{t('Temperaturas')}</p>
        {#if live.temps.length}
          {#each live.temps.slice(0, 6) as tp (tp.label)}
            <div class="row"><span class="sub">{tp.label}</span><b class="num small {tp.celsius >= 85 ? 'hot' : tp.celsius >= 70 ? 'warm' : ''}">{Math.round(tp.celsius)} °C</b></div>
          {/each}
        {:else}
          <p class="sub">{t('Este equipo no permite leer temperaturas desde Windows.')}</p>
        {/if}
      </section>

      {#if specs}
        <section>
          <p class="k">{t('Equipo')}</p>
          <dl>
            {#if specs.host}<dt>{t('Nombre')}</dt><dd>{specs.host}</dd>{/if}
            {#if specs.os}<dt>{t('Sistema')}</dt><dd>{specs.os}</dd>{/if}
            {#if specs.cpu}<dt>CPU</dt><dd>{specs.cpu}<small>{t('{p} núcleos · {l} hilos', { p: specs.physical_cores, l: specs.logical_cores })}</small></dd>{/if}
            <dt>RAM</dt><dd>{fmt(specs.ram_total)}</dd>
            {#each specs.gpus as g, i (g)}<dt>{i ? '' : 'GPU'}</dt><dd>{g}</dd>{/each}
            {#each specs.drives as d (d.name)}
              <dt>{t('Disco')}</dt>
              <dd>{d.name}<small>{d.media}{d.size ? ' · ' + fmt(d.size) : ''} · <span class:bad={!healthy(d.health)}>{healthy(d.health) ? t('Salud: correcta') : t('Salud') + ': ' + d.health}</span></small></dd>
            {/each}
            <dt>{t('Encendido')}</dt><dd>{uptime(live.uptime)}</dd>
          </dl>
        </section>
      {/if}
    {:else}
      <p class="sub num">···</p>
    {/if}
  </aside>
{/if}

<style>
  .mon { position: sticky; top: 32px; align-self: flex-start; flex: none; width: 330px; margin: 32px 32px 0 0; padding-left: 28px; border-left: 1px solid var(--rule); display: grid; grid-template-columns: minmax(0, 1fr); gap: 22px; max-height: calc(100vh - 64px); overflow-x: hidden; overflow-y: auto; scrollbar-gutter: stable; scrollbar-width: thin; }
  section { display: grid; grid-template-columns: minmax(0, 1fr); gap: 6px; min-width: 0; }
  .row { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; min-width: 0; }
  .row > :last-child { flex: none; }
  .row > :first-child:not(:last-child) { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .k { margin: 0; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--ink-3); }
  b.num.small { font-size: 14px; letter-spacing: 0; }
  .nm { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
  .bad { color: var(--danger, #c4281c); font-weight: 600; }
  .spark.thin { height: 28px; }
  b.num { font-size: 22px; font-weight: 500; letter-spacing: -0.03em; }
  .sub { margin: 0; font-size: 12px; color: var(--ink-3); }
  .note { margin: 0; font-size: 12.5px; color: var(--ink-2); }
  .bar { height: 6px; background: var(--paper-2); border-radius: 1px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width 0.4s; }
  .bar i.warm { background: #d98b00; }
  .bar i.hot, b.hot { background: var(--danger, #c4281c); color: var(--danger, #c4281c); }
  b.hot { background: none; }
  b.warm { color: #b36b00; }
  .spark { display: flex; align-items: flex-end; justify-content: flex-end; gap: 2px; height: 44px; border-bottom: 1px solid var(--ink); }
  .spark i { flex: 0 0 5px; background: var(--accent); opacity: 0.85; transition: height 0.4s; }
  .disk { display: grid; gap: 4px; margin-top: 8px; font-size: 12.5px; }
  dl { display: grid; grid-template-columns: 62px minmax(0, 1fr); gap: 6px 10px; margin: 0; font-size: 12.5px; }
  dt { color: var(--ink-3); }
  dd { margin: 0; word-break: break-word; }
  dd small { display: block; color: var(--ink-3); }
</style>
