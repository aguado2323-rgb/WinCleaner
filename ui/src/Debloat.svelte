<script>
  import { invoke } from '@tauri-apps/api/core';

  let { notify } = $props();

  let items = $state([]);
  let loading = $state(true);
  let selected = $state({});
  let confirming = $state(false);
  let makePoint = $state(true);
  let phase = $state('idle'); // idle | working
  let pointError = $state('');
  let open = $state({});

  let chosen = $derived(items.filter((i) => selected[i.id]));
  let cautious = $derived(chosen.filter((i) => i.risk === 'cautious').length);
  const groups = $derived(
    [...new Set(items.map((i) => i.category))].map((c) => [c, items.filter((i) => i.category === c)])
  );

  async function load() {
    loading = true;
    try {
      items = await invoke('debloat_list');
      selected = Object.fromEntries(items.filter((i) => i.risk === 'safe').map((i) => [i.id, false]));
    } catch (e) {
      notify('No se pudo leer las aplicaciones: ' + e);
    }
    loading = false;
  }

  function setSafe() {
    selected = Object.fromEntries(items.map((i) => [i.id, i.risk === 'safe']));
  }

  async function apply(skipPoint = false) {
    phase = 'working';
    pointError = '';
    if (makePoint && !skipPoint) {
      try {
        await invoke('restore_create', { reason: 'debloat' });
      } catch (e) {
        pointError = String(e);
        phase = 'idle';
        return;
      }
    }
    try {
      const res = await invoke('debloat_apply', { ids: chosen.map((i) => i.id) });
      const ok = res.filter((r) => r.ok).length;
      const bad = res.filter((r) => !r.ok);
      notify(bad.length ? `${ok} quitadas, ${bad.length} con error: ${bad[0].error}` : `${ok} aplicaciones quitadas`);
    } catch (e) {
      notify(String(e));
    }
    confirming = false;
    phase = 'idle';
    await load();
  }

  load();
</script>

<p class="label">Preinstaladas</p>
<h1 class="h1">Quita lo que no pediste.</h1>
<p class="lede">Aplicaciones que vienen preinstaladas con Windows o con tu equipo. Solo aparecen las que están instaladas y se pueden quitar sin romper nada. Todas se reinstalan desde Microsoft Store.</p>

<section class="act">
  <button class="btn primary" disabled={!chosen.length || loading} onclick={() => (confirming = true)}>
    Quitar {chosen.length || ''}
  </button>
  <button class="link" onclick={setSafe}>Marcar las seguras</button>
  <button class="link" onclick={() => (selected = {})}>Nada</button>
</section>

{#if loading}
  <p class="empty num">···</p>
{:else if !items.length}
  <p class="empty">No se ha encontrado ninguna aplicación del catálogo. Tu Windows ya está limpio.</p>
{:else}
  {#each groups as [cat, list] (cat)}
    <div class="group"><h2 class="label">{cat}</h2><span class="num">{list.length}</span></div>
    <ul>
      {#each list as i, n (i.id)}
        <li style="animation-delay:{n * 24}ms">
          <label>
            <input type="checkbox" bind:checked={selected[i.id]} />
            <span class="box" aria-hidden="true"></span>
            <span class="txt">
              <b>{i.name}</b>
              <small>{i.what}</small>
            </span>
            <span class="tag label" class:warn={i.risk === 'cautious'}>{i.risk === 'safe' ? 'Segura' : 'Con cuidado'}</span>
          </label>
          <button class="more link" aria-expanded={!!open[i.id]} onclick={() => (open[i.id] = !open[i.id])}>
            {open[i.id] ? 'Ocultar efecto' : '¿Qué pasa si la quito?'}
          </button>
          {#if open[i.id]}<p class="impact">{i.impact}</p>{/if}
        </li>
      {/each}
    </ul>
  {/each}
{/if}

{#if confirming}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="overlay" role="presentation" onclick={() => phase === 'idle' && (confirming = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <h3>¿Quitar {chosen.length} {chosen.length === 1 ? 'aplicación' : 'aplicaciones'}?</h3>
      <p>
        Se quitan solo para tu usuario y puedes reinstalarlas desde Microsoft Store.
        {#if cautious}<br /><b>{cautious} {cautious === 1 ? 'es' : 'son'} «con cuidado»</b>: revisa su efecto antes de seguir.{/if}
      </p>
      <label class="pt">
        <input type="checkbox" bind:checked={makePoint} disabled={phase === 'working'} />
        <span class="box" aria-hidden="true"></span>
        <span>Crear un punto de restauración antes (recomendado)</span>
      </label>
      {#if pointError}
        <p class="err">{pointError}</p>
      {/if}
      <div class="row">
        <button class="btn" disabled={phase === 'working'} onclick={() => (confirming = false)}>Cancelar</button>
        {#if pointError}
          <button class="btn" disabled={phase === 'working'} onclick={() => apply(true)}>Continuar sin punto</button>
        {:else}
          <button class="btn primary" disabled={phase === 'working'} onclick={() => apply()}>{phase === 'working' ? 'Quitando…' : 'Quitar'}</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .h1 { margin-top: 10px; }
  .act { display: flex; align-items: center; gap: 26px; margin: 28px 0 8px; }
  .group { display: flex; align-items: baseline; justify-content: space-between; margin: 30px 0 4px; padding-bottom: 8px; border-bottom: 1px solid var(--ink); }
  .group h2 { margin: 0; color: var(--ink); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { animation: line 0.25s both; border-bottom: 1px solid var(--rule); }
  li label { display: flex; align-items: center; gap: 16px; padding: 13px 2px 4px; cursor: pointer; }
  li label:hover { background: var(--paper-2); }
  input { position: absolute; opacity: 0; pointer-events: none; }
  .box { flex: none; position: relative; width: 16px; height: 16px; border: 1.5px solid var(--ink); border-radius: 2px; }
  input:checked + .box { background: var(--accent); }
  input:checked + .box::after { content: ''; position: absolute; left: 4px; top: 0; width: 4px; height: 9px; border: solid var(--on-accent); border-width: 0 2px 2px 0; transform: rotate(40deg); }
  input:focus-visible + .box { outline: 2px solid var(--accent); outline-offset: 2px; }
  .txt { flex: 1; display: grid; gap: 1px; min-width: 0; }
  .txt b { font-weight: 600; font-size: 14.5px; }
  .txt small { color: var(--ink-3); font-size: 12.5px; }
  .tag { flex: none; font-size: 10px; }
  .tag.warn { color: var(--accent); }
  .more { margin: 0 0 10px 32px; font-size: 11.5px; }
  .impact { margin: 0 0 12px 32px; color: var(--ink-2); font-size: 13px; line-height: 1.5; border-left: 2px solid var(--accent); padding-left: 10px; }
  .empty { padding: 50px 0; color: var(--ink-2); font-size: 15px; }
  .pt { display: flex; align-items: center; gap: 12px; margin: 0 0 18px; cursor: pointer; font-size: 13.5px; }
  .err { color: var(--accent); font-size: 13px; margin: 0 0 16px; }
  .dialog b { color: var(--ink); }
</style>
