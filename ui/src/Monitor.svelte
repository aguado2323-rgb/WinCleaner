<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { fmt } from './format.js';
  import { t } from './i18n.svelte.js';

  const MIN_WIDTH = 1480; // por debajo no cabe junto a la pantalla: el panel se oculta y no consulta nada
  const HISTORY = 40;

  let visible = $state(false);
  let specs = $state(null);
  let live = $state(null);
  let history = $state([]);
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
    const mq = matchMedia(`(min-width: ${MIN_WIDTH}px)`);
    let timer;
    const apply = () => {
      visible = mq.matches;
      clearInterval(timer);
      if (visible) {
        if (!specs) loadSpecs();
        sample();
        timer = setInterval(sample, 2000);
      }
    };
    apply();
    mq.addEventListener('change', apply);
    return () => {
      clearInterval(timer);
      mq.removeEventListener('change', apply);
    };
  });

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
        <p class="k">{t('Temperaturas')}</p>
        {#if live.temps.length}
          {#each live.temps.slice(0, 6) as tp (tp.label)}
            <div class="row"><span class="sub">{tp.label}</span><b class="num {tp.celsius >= 85 ? 'hot' : tp.celsius >= 70 ? 'warm' : ''}">{Math.round(tp.celsius)} °C</b></div>
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
{/if}

<style>
  .mon { position: sticky; top: 32px; align-self: flex-start; flex: none; width: 290px; margin: 32px 40px 0 0; padding-left: 28px; border-left: 1px solid var(--rule); display: grid; gap: 22px; }
  section { display: grid; gap: 6px; }
  .row { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; }
  .k { margin: 0; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--ink-3); }
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
  dl { display: grid; grid-template-columns: 62px 1fr; gap: 6px 10px; margin: 0; font-size: 12.5px; }
  dt { color: var(--ink-3); }
  dd { margin: 0; word-break: break-word; }
  dd small { display: block; color: var(--ink-3); }
</style>
