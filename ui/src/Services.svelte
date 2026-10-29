<script>
  import { t } from './i18n.svelte.js';
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';

  let { notify } = $props();

  let items = $state([]);
  let admin = $state(false);
  let loading = $state(true);
  let busy = $state({});
  let makePoint = $state(true);
  let pointTried = false;

  let appliedCount = $derived(items.filter((i) => i.applied).length);

  async function load() {
    try {
      [items, admin] = await Promise.all([invoke('services_state'), invoke('app_is_admin')]);
    } catch (e) {
      notify(t('No se pudieron leer los servicios: ') + e);
    }
    loading = false;
  }

  async function ensurePoint() {
    if (!makePoint || pointTried || !admin) return;
    pointTried = true;
    try {
      await invoke('restore_create', { reason: 'tweaks' });
    } catch {
      /* ya había uno reciente o la protección está apagada: cada cambio se puede deshacer igualmente */
    }
  }

  async function toggle(s) {
    const enable = !s.applied;
    busy[s.id] = true;
    try {
      if (enable) await ensurePoint();
      await invoke('service_apply', { id: s.id, enable });
      await load();
      notify(`${s.name}: ${enable ? t('pasa a Manual') : t('vuelve a como estaba')}. ${t('Se nota al reiniciar el equipo.')}`);
    } catch (e) {
      notify(String(e));
    }
    busy[s.id] = false;
  }

  load();
</script>

<div class="sticky">
  <p class="label">{t('Servicios')}</p>
  <h1 class="h1">{t('Menos cosas al arrancar.')}</h1>
  <p class="lede">{t('Los servicios son programas de Windows que arrancan solos con el equipo. Pasar uno a «Manual» no lo borra ni lo detiene: solo evita que arranque solo, y Windows lo inicia si algo lo necesita. «Deshacer» devuelve el inicio original.')} <Help text={H.servicesInfo.text} risk={H.servicesInfo.risk} label={t('Cómo funcionan los servicios')} /></p>

  <section class="top">
    <span class="big num">{appliedCount}<small>/{items.length}</small></span>
    <span class="cap">{t('servicios en Manual')}</span>
    <label class="pt">
      <input type="checkbox" bind:checked={makePoint} />
      <span class="box" aria-hidden="true"></span>
      <span>{t('Punto de restauración antes de empezar')}</span>
    </label>
    <Help text={H.tweakPoint.text} risk={H.tweakPoint.risk} label={t('Qué es un punto de restauración')} />
  </section>
</div>
{#if !loading && !admin}
  <p class="note">{t('Sin permisos de administrador no se puede cambiar ningún servicio. Puedes reiniciar como administrador desde «Restauración».')}</p>
{/if}

{#if loading}
  <p class="empty num">···</p>
{:else if !items.length}
  <p class="empty">{t('No hay servicios que cambiar: los del catálogo ya están en Manual o no existen en tu Windows.')}</p>
{:else}
  <ul>
    {#each items as s, n (s.id)}
      <li style="animation-delay:{n * 18}ms" class:on={s.applied}>
        <div class="txt">
          <span class="top2">
            <b>{s.name}</b>
            {#if s.risk === 'care'}<span class="tag label warn">{t('Con cuidado')}</span>{/if}
          </span>
          <small>{s.what} · {t('Ahora')}: {t(s.start)}</small>
        </div>
        <Help text={s.help} risk={s.risk === 'care' ? 'care' : 'reversible'} label={t('Qué hace {n}', { n: s.name })} />
        <span class="state label" class:on={s.applied}>{s.applied ? t('En Manual') : t('Automático')}</span>
        <button class="act" class:undo={s.applied} disabled={busy[s.id] || !admin} onclick={() => toggle(s)}>
          {s.applied ? t('Deshacer') : t('Pasar a Manual')}
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .h1 { margin-top: 10px; }
  .top { display: flex; align-items: baseline; flex-wrap: wrap; gap: 8px 14px; margin: 26px 0 6px; }
  .big { font-size: 64px; font-weight: 500; letter-spacing: -0.06em; line-height: 1; }
  .big small { font-size: 26px; color: var(--ink-3); letter-spacing: -0.03em; }
  .cap { flex: 1; color: var(--ink-2); }
  .pt { display: flex; align-items: center; gap: 10px; cursor: pointer; font-size: 13px; }
  .pt input { position: absolute; opacity: 0; pointer-events: none; }
  .box { flex: none; position: relative; width: 16px; height: 16px; border: 1.5px solid var(--ink); border-radius: 2px; }
  input:checked + .box { background: var(--accent); }
  input:checked + .box::after { content: ''; position: absolute; left: 4px; top: 0; width: 4px; height: 9px; border: solid var(--on-accent); border-width: 0 2px 2px 0; transform: rotate(40deg); }
  input:focus-visible + .box { outline: 2px solid var(--accent); outline-offset: 2px; }
  .note { margin: 8px 0 0; padding: 10px 12px; border-left: 4px solid var(--accent); color: var(--ink-2); font-size: 13px; line-height: 1.5; }

  .group { display: flex; align-items: baseline; justify-content: space-between; margin: 30px 0 4px; padding-bottom: 8px; border-bottom: 1px solid var(--ink); }
  .group h2 { margin: 0; color: var(--ink); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; align-items: center; gap: 14px; padding: 12px 2px; border-bottom: 1px solid var(--rule); animation: line 0.22s both; }
  .txt { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .top2 { display: flex; align-items: baseline; gap: 10px; }
  .top2 b { font-weight: 600; font-size: 14.5px; }
  small { color: var(--ink-3); font-size: 12.5px; }
  .tag { font-size: 10px; }
  .tag.warn { color: var(--accent); }

  .state { flex: none; width: 84px; text-align: right; }
  .state.on { color: var(--accent); font-weight: 600; }
  .state.on::before { content: '● '; }
  .act { flex: none; width: 128px; border: 1px solid var(--ink); border-radius: 2px; background: var(--accent); color: var(--on-accent); padding: 6px 0; font-family: var(--mono); font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; transition: background 0.1s, color 0.1s; }
  .act:hover:not(:disabled) { background: var(--ink); color: var(--paper); }
  .act.undo { background: none; color: var(--ink-2); font-weight: 400; }
  .act:disabled { opacity: 0.45; cursor: default; }
  .empty { padding: 50px 0; color: var(--ink-2); font-size: 15px; }
</style>
