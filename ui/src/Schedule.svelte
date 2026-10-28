<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { fmt, fmtDate } from './format.js';
  import { invoke } from '@tauri-apps/api/core';

  let { notify, refreshQuarantine } = $props();

  let cfg = $state(null);
  let task = $state(false);
  let exe = $state('');
  let last = $state(null);
  let busy = $state(false);

  const FREQ = [['daily', 'Cada día'], ['weekly', 'Cada semana'], ['monthly', 'Cada mes']]; // se traducen al mostrarlas
  const MODE = [['clean', 'Limpiar'], ['notify', 'Solo avisar']];
  const PURGE = [[0, 'Nunca'], [7, '7 días'], [30, '30 días'], [90, '90 días']];

  // Una carpeta de descargas o temporal movible hace que la tarea apunte a un sitio que puede desaparecer.
  let risky = $derived(/\\(Temp|Downloads|Descargas)\\/i.test(exe));

  async function load() {
    try {
      const st = await invoke('schedule_state');
      cfg = st.config;
      task = st.task;
      exe = st.exe;
      last = st.last;
    } catch (e) {
      notify(t('No se pudo leer la programación: ') + e);
    }
  }

  async function save() {
    busy = true;
    try {
      await invoke('schedule_apply', { config: cfg });
      task = cfg.enabled;
      notify(cfg.enabled ? t('Limpieza programada.') : t('Programación desactivada.'));
    } catch (e) {
      notify(String(e));
      await load();
    }
    busy = false;
  }

  async function runNow() {
    busy = true;
    try {
      last = await invoke('schedule_now', { config: cfg });
      refreshQuarantine?.();
      notify(cfg.mode === 'clean' ? t('Hecho: {b} a la cuarentena.', { b: fmt(last.bytes) }) : t('Se podrían recuperar {b}.', { b: fmt(last.bytes) }));
    } catch (e) {
      notify(String(e));
    }
    busy = false;
  }

  load();
</script>

<div class="sticky">
  <p class="label">{t('Programación')}</p>
  <h1 class="h1">{t('Que se limpie solo.')}</h1>
  <p class="lede">{t('WinCleaner puede hacer una limpieza segura a la hora que elijas, sin abrir ventana, y avisarte cuando acabe. Todo va a la cuarentena.')} <Help text={H.schedAuto.text} risk={H.schedAuto.risk} label={t('Cómo funciona la programación')} /></p>
</div>

{#if !cfg}
  <p class="empty num">···</p>
{:else}
  <section class="card">
    <label class="row">
      <input type="checkbox" bind:checked={cfg.enabled} onchange={save} disabled={busy} />
      <span class="box" aria-hidden="true"></span>
      <span><b>{t('Limpiar automáticamente')}</b><small>{task ? t('Tarea activa en Windows') : t('Desactivado')}</small></span>
    </label>

    <div class="grid" class:off={!cfg.enabled}>
      <div class="f">
        <span class="label">{t('Frecuencia')}</span>
        <div class="seg">
          {#each FREQ as [v, l]}
            <button class:on={cfg.freq === v} disabled={busy || !cfg.enabled} onclick={() => { cfg.freq = v; save(); }}>{t(l)}</button>
          {/each}
        </div>
      </div>
      <div class="f">
        <span class="label">{t('Hora')}</span>
        <input class="time num" type="time" bind:value={cfg.time} disabled={busy || !cfg.enabled} onchange={save} />
      </div>
      <div class="f">
        <span class="label">{t('Qué hace')} <Help text={H.schedMode.text} risk={H.schedMode.risk} label={t('Limpiar o solo avisar')} /></span>
        <div class="seg">
          {#each MODE as [v, l]}
            <button class:on={cfg.mode === v} disabled={busy || !cfg.enabled} onclick={() => { cfg.mode = v; save(); }}>{t(l)}</button>
          {/each}
        </div>
      </div>
      <div class="f">
        <span class="label">{t('Vaciar cuarentena de más de')} <Help text={H.schedPurge.text} risk={H.schedPurge.risk} label={t('Vaciado automático')} /></span>
        <div class="seg">
          {#each PURGE as [v, l]}
            <button class:on={cfg.purge_days === v} disabled={busy || !cfg.enabled} onclick={() => { cfg.purge_days = v; save(); }}>{t(l)}</button>
          {/each}
        </div>
      </div>
    </div>
    {#if cfg.purge_days > 0 && cfg.enabled}
      <p class="note">{t('Los lotes de la cuarentena con más de {n} días se borrarán definitivamente.', { n: cfg.purge_days })}</p>
    {/if}
    {#if risky && cfg.enabled}
      <p class="note">{t('WinCleaner está en una carpeta temporal o de descargas ({exe}). Muévelo a un sitio fijo y guarda de nuevo, o la tarea dejará de encontrarlo.', { exe })}</p>
    {/if}
  </section>

  <section class="card last">
    <span class="label">{t('Última pasada')}</span>
    {#if last}
      <p class="num sum">{fmt(last.bytes)} <small>· {t('{n} archivos', { n: last.files })} · {last.mode === 'notify' ? t('solo aviso') : t('limpieza')}{last.purged ? ' · ' + t('{n} lotes vaciados', { n: last.purged }) : ''}</small></p>
      <p class="when">{fmtDate(last.when)}</p>
    {:else}
      <p class="when">{t('Todavía no se ha ejecutado.')}</p>
    {/if}
    <button class="btn" disabled={busy} onclick={runNow}>{t('Probar ahora')}</button>
  </section>
{/if}

<style>
  .card { border: 1px solid var(--ink); background: var(--card, transparent); padding: 18px 20px; margin-top: 22px; max-width: 640px; }
  .row { display: flex; gap: 12px; align-items: center; cursor: pointer; }
  .row input { position: absolute; opacity: 0; }
  .row small { display: block; color: var(--ink-2); font-size: 12px; margin-top: 2px; }
  .box { width: 18px; height: 18px; border: 1px solid var(--ink); flex: none; display: grid; place-items: center; }
  .row input:checked + .box { background: var(--accent); }
  .row input:checked + .box::after { content: ''; width: 8px; height: 4px; border: solid var(--on-accent); border-width: 0 0 2px 2px; transform: rotate(-45deg) translate(1px, -1px); }
  .row input:focus-visible + .box { outline: 2px solid var(--accent); outline-offset: 2px; }
  .grid { display: grid; gap: 18px; margin-top: 20px; }
  .grid.off { opacity: 0.45; }
  .f { display: grid; gap: 8px; }
  .seg { display: flex; flex-wrap: wrap; }
  .seg button { border: 1px solid var(--ink); margin-left: -1px; background: none; padding: 7px 12px; font-family: var(--mono); font-size: 12px; }
  .seg button:first-child { margin-left: 0; }
  .seg button.on { background: var(--ink); color: var(--paper); }
  .seg button:disabled { cursor: default; }
  .time { width: 110px; border: 1px solid var(--ink); background: none; color: var(--ink); padding: 7px 10px; font-size: 14px; }
  .note { margin: 14px 0 0; padding: 10px 12px; border-left: 3px solid var(--accent); font-size: 13px; color: var(--ink-2); line-height: 1.5; }
  .last { display: grid; gap: 6px; justify-items: start; }
  .sum { margin: 4px 0 0; font-size: 22px; }
  .sum small { font-size: 12px; color: var(--ink-2); }
  .when { margin: 0 0 8px; color: var(--ink-2); font-size: 13px; }
</style>
