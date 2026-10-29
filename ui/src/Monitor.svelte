<script module>
  // Las especificaciones no cambian: se piden una vez aunque el monitor se apague y se vuelva a encender.
  let savedSpecs = null;
  let savedDrives = null;
</script>

<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { fmt } from './format.js';
  import { t } from './i18n.svelte.js';

  // `floating`: el panel va encima, a la derecha (en Desinstalar, que necesita todo el ancho). En ventanas
  // estrechas el CSS hace lo mismo por su cuenta.
  // `onclose`: el botón «×» del panel apaga el monitor, igual que el botón de la barra lateral.
  let { floating = false, onclose } = $props();

  const HISTORY = 40;
  let specs = $state(null);
  let drives = $state(null); // null = todavía leyendo
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

  // Si una muestra tarda más que el intervalo no se encolan otras detrás; pasados 15 s se vuelve a
  // intentar igualmente, para que una respuesta perdida no deje el panel parado.
  let busySince = 0;
  async function sample() {
    if (busySince && Date.now() - busySince < 15000) return;
    const mine = (busySince = Date.now());
    try {
      live = await invoke('monitor_sample');
      history = [...history, live.cpu].slice(-HISTORY);
      netHistory = [...netHistory, live.net_down].slice(-HISTORY);
      failed = false;
    } catch {
      failed = true;
    } finally {
      if (busySince === mine) busySince = 0;
    }
  }

  async function loadSpecs() {
    if (savedSpecs) return (specs = savedSpecs);
    try {
      specs = savedSpecs = await invoke('monitor_specs');
    } catch {
      /* sin especificaciones: el resto del panel sigue funcionando */
    }
  }

  async function loadDrives() {
    if (savedDrives) return (drives = savedDrives);
    try {
      drives = savedDrives = await invoke('monitor_drives');
    } catch {
      drives = [];
    }
  }

  onMount(() => {
    // El panel solo existe mientras el monitor está activado; su colocación (al lado o encima) la decide
    // el CSS según el ancho, así que nunca desaparece al redimensionar. Con la ventana minimizada u oculta
    // no se consulta nada: nadie mira el panel y cada muestra cuesta CPU.
    // La primera muestra se pide siempre; después, cada 2 s salvo con la ventana oculta. Si WebView2 dice
    // «oculta» pero la ventana tiene el foco, se cree al foco (el aviso de visibilidad no siempre es fiable).
    const tick = () => {
      if (document.hidden && !document.hasFocus()) return;
      sample();
    };
    sample();
    const timer = setInterval(tick, 2000);
    const wake = () => !document.hidden && sample();
    document.addEventListener('visibilitychange', wake);
    loadSpecs();
    loadDrives();
    return () => {
      clearInterval(timer);
      document.removeEventListener('visibilitychange', wake);
    };
  });

  let netMax = $derived(Math.max(1, ...netHistory));
  const rate = (b) => fmt(b) + '/s';
  const healthy = (h) => /^(healthy|ok)$/i.test(h);
  // Windows da el estado en inglés: se traduce lo conocido y lo demás se muestra tal cual.
  const health = (h) => ({ warning: t('Aviso'), unhealthy: t('Con fallos'), unknown: t('Desconocida') })[String(h).toLowerCase()] ?? (h || t('Desconocida'));
  let ramPct = $derived(live ? pct(live.ram_used, live.ram_total) : 0);
  let cpuPct = $derived(live ? Math.round(live.cpu) : 0);
</script>

<aside class="mon" class:floating aria-label={t('Monitor del equipo')}>
  <div class="head">
    <p class="label">{t('Monitor')}</p>
    <button class="close" onclick={onclose} aria-label={t('Ocultar el monitor')} title={t('Ocultar el monitor')}>×</button>
  </div>

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
      <p class="k">{t('Salud de los discos')}</p>
      {#if !drives}
        <p class="sub num">···</p>
      {:else if drives.length}
        {#each drives as d (d.name)}
          <div class="disk">
            <div class="row"><span>{d.name}</span><b class="state" class:bad={!healthy(d.health)}>{healthy(d.health) ? t('Correcta') : health(d.health)}</b></div>
            <p class="sub num">{[d.media, d.size ? fmt(d.size) : '', d.temp != null ? `${Math.round(d.temp)} °C` : '', d.wear != null ? t('{w}% de vida usada', { w: d.wear }) : ''].filter(Boolean).join(' · ')}</p>
          </div>
        {/each}
      {:else}
        <p class="sub">{t('Windows no informa de la salud de los discos en este equipo.')}</p>
      {/if}
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
          <dt>{t('Encendido')}</dt><dd>{uptime(live.uptime)}</dd>
        </dl>
      </section>
    {/if}
  {:else}
    <p class="sub num">···</p>
  {/if}
</aside>

<style>
  .mon { position: sticky; top: 32px; align-self: flex-start; flex: none; width: 330px; margin: 32px 32px 0 0; padding-left: 28px; border-left: 1px solid var(--rule); display: grid; grid-template-columns: minmax(0, 1fr); gap: 22px; max-height: calc(100vh - 64px); overflow-x: hidden; overflow-y: auto; }
  .head { display: flex; align-items: center; justify-content: space-between; }
  .head .label { margin: 0; }
  .close { border: 0; background: none; padding: 0 4px; font-size: 20px; line-height: 1; color: var(--ink-3); }
  .close:hover { color: var(--ink); }
  /* Ventana estrecha: el panel no cabe al lado y se superpone a la derecha en vez de ocultarse. */
  @media (max-width: 1239px) {
    .mon { position: fixed; top: 0; right: 0; bottom: 0; z-index: 5; width: 300px; max-height: none; margin: 0; padding: 24px 20px 24px 24px; background: var(--paper); box-shadow: -10px 0 30px rgb(0 0 0 / 0.18); }
  }
  .mon.floating { position: fixed; top: 0; right: 0; bottom: 0; z-index: 5; width: 300px; max-height: none; margin: 0; padding: 24px 20px 24px 24px; background: var(--paper); box-shadow: -10px 0 30px rgb(0 0 0 / 0.18); }
  section { display: grid; grid-template-columns: minmax(0, 1fr); gap: 6px; min-width: 0; }
  .row { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; min-width: 0; }
  .row > :last-child { flex: none; }
  .row > :first-child:not(:last-child) { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .k { margin: 0; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--ink-3); }
  b.num.small { font-size: 14px; letter-spacing: 0; }
  .nm { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
  .bad { color: var(--danger, #c4281c); font-weight: 600; }
  .state { font-size: 12.5px; font-weight: 600; color: var(--ink); }
  .state.bad { color: var(--danger, #c4281c); }
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
