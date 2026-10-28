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

  let groups = $derived(
    [...new Set(items.map((i) => i.category))].map((c) => [c, items.filter((i) => i.category === c)])
  );
  let appliedCount = $derived(items.filter((i) => i.applied).length);

  const RESTART = {
    explorer: 'Se nota al reiniciar el Explorador o volver a iniciar sesión.',
    reboot: 'Se nota tras reiniciar el equipo.',
  };

  async function load() {
    try {
      [items, admin] = await Promise.all([invoke('tweaks_state'), invoke('app_is_admin')]);
    } catch (e) {
      notify(t('No se pudieron leer los ajustes: ') + e);
    }
    loading = false;
  }

  async function ensurePoint() {
    if (!makePoint || pointTried || !admin) return;
    pointTried = true;
    try {
      await invoke('restore_create', { reason: 'tweaks' });
    } catch {
      /* ya había uno reciente o la protección está apagada: cada ajuste es reversible igualmente */
    }
  }

  async function toggle(tw, enable) {
    if (tw.applied === enable) return;
    busy[tw.id] = true;
    try {
      if (enable) await ensurePoint();
      await invoke('tweak_apply', { id: tw.id, enable });
      tw.applied = enable;
      const note = RESTART[tw.restart];
      notify(
        `${tw.name}: ${enable ? t('aplicado') : t('deshecho')}.${note ? ' ' + t(note) : ''}`,
        tw.restart === 'explorer' ? { label: t('Reiniciar Explorador'), run: () => invoke('restart_explorer') } : undefined
      );
    } catch (e) {
      notify(String(e));
    }
    busy[tw.id] = false;
  }

  load();
</script>

<p class="label">{t('Ajustes de Windows')}</p>
<h1 class="h1">{t('Windows a tu manera.')}</h1>
<p class="lede">{t('Cambios opcionales en el propio Windows (no en WinCleaner): privacidad, rendimiento e interfaz. Pulsa «Aplicar» para activar el cambio y «Deshacer» para volver a como estaba.')} <Help text={H.tweakToggle.text} risk={H.tweakToggle.risk} label={t('Cómo funcionan los ajustes')} /></p>

<section class="top">
  <span class="big num">{appliedCount}<small>/{items.length}</small></span>
  <span class="cap">{t('cambios aplicados en Windows')}</span>
  <label class="pt">
    <input type="checkbox" bind:checked={makePoint} />
    <span class="box" aria-hidden="true"></span>
    <span>{t('Punto de restauración antes de empezar')}</span>
  </label>
  <Help text={H.tweakPoint.text} risk={H.tweakPoint.risk} label={t('Qué es un punto de restauración')} />
</section>
{#if !loading && !admin}
  <p class="note">{t('Sin permisos de administrador: los ajustes marcados como «Admin» no se pueden cambiar. Puedes reiniciar como administrador desde «Restauración».')}</p>
{/if}

{#if loading}
  <p class="empty num">···</p>
{:else}
  {#each groups as [cat, list] (cat)}
    <div class="group"><h2 class="label">{cat}</h2><span class="num">{list.filter((tw) => tw.applied).length}/{list.length}</span></div>
    <ul>
      {#each list as tw, n (tw.id)}
        <li style="animation-delay:{n * 18}ms" class:on={tw.applied}>
          <div class="txt">
            <span class="top2">
              <b>{tw.name}</b>
              {#if tw.risk === 'care'}<span class="tag label warn">{t('Con cuidado')}</span>{/if}
              {#if tw.needs_admin}<span class="tag label">{t('Admin')}</span>{/if}
            </span>
            <small>{tw.what}</small>
          </div>
          <Help text={tw.help + (RESTART[tw.restart] ? ' ' + t(RESTART[tw.restart]) : '')} risk={tw.risk === 'care' ? 'care' : 'reversible'} label={t('Qué hace {n}', { n: tw.name })} />
          <span class="state label" class:on={tw.applied}>{tw.applied ? t('Aplicado') : t('Sin aplicar')}</span>
          <button class="act" class:undo={tw.applied} disabled={busy[tw.id] || (tw.needs_admin && !admin)} onclick={() => toggle(tw, !tw.applied)}>
            {tw.applied ? t('Deshacer') : t('Aplicar')}
          </button>
        </li>
      {/each}
    </ul>
  {/each}
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
  .act { flex: none; width: 92px; border: 1px solid var(--ink); border-radius: 2px; background: var(--accent); color: var(--on-accent); padding: 6px 0; font-family: var(--mono); font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; transition: background 0.1s, color 0.1s; }
  .act:hover:not(:disabled) { background: var(--ink); color: var(--paper); }
  .act.undo { background: none; color: var(--ink-2); font-weight: 400; }
  .act:disabled { opacity: 0.45; cursor: default; }
  .empty { padding: 50px 0; color: var(--ink-2); font-size: 15px; }
</style>
