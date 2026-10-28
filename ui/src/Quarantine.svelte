<script>
  import { invoke } from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';
  import { fmt, fmtDate } from './format.js';

  let { q, notify, refreshQuarantine } = $props();
  let confirm = $state(null); // { batch? } | 'all'

  let totalBytes = $derived(q.batches.reduce((a, b) => a + b.bytes, 0));

  async function restore(batch) {
    const out = await invoke('restore', { batch });
    notify(`${out.restored} archivos restaurados` + (out.errors.length ? ` · ${out.errors.length} no se pudieron restaurar` : ''));
    await refreshQuarantine();
  }

  async function purge(batch) {
    confirm = null;
    try {
      await invoke('purge', { batch });
      notify('Borrado definitivamente');
    } catch (e) {
      notify('No se pudo borrar: ' + e);
    }
    await refreshQuarantine();
  }

  const openFolder = () => invoke('open_quarantine').catch((e) => notify('No se pudo abrir: ' + e));
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (confirm = null)} />

<header>
  <h1>Cuarentena</h1>
  <p>Aquí se guardan los archivos que limpias, por si cambias de idea. Hasta que los borres aquí, se pueden recuperar.</p>
</header>

<section class="where">
  <span class="ico"><Icon name="folder" size={20} /></span>
  <div>
    <small>Carpeta en tu equipo</small>
    <code>{q.path || '—'}</code>
  </div>
  <button class="ghost" onclick={openFolder}>Abrir carpeta</button>
</section>

{#if q.batches.length}
  <div class="head">
    <span>{q.batches.length} {q.batches.length === 1 ? 'limpieza guardada' : 'limpiezas guardadas'} · {fmt(totalBytes)}</span>
    <button class="danger" onclick={() => (confirm = 'all')}><Icon name="trash" size={15} /> Vaciar todo</button>
  </div>
  <ul>
    {#each q.batches as b, i (b.batch)}
      <li style="animation-delay:{i * 40}ms">
        <div class="info">
          <b>{fmtDate(b.batch)}</b>
          <small>{b.files} archivos · {fmt(b.bytes)}</small>
        </div>
        <button class="ghost" onclick={() => restore(b.batch)}><Icon name="undo" size={15} /> Restaurar</button>
        <button class="icon" title="Borrar definitivamente" aria-label="Borrar definitivamente" onclick={() => (confirm = b.batch)}><Icon name="trash" size={16} /></button>
      </li>
    {/each}
  </ul>
{:else}
  <div class="empty">
    <span><Icon name="shield" size={30} /></span>
    <b>La cuarentena está vacía</b>
    <small>Cuando limpies, los archivos aparecerán aquí.</small>
  </div>
{/if}

{#if confirm}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="overlay" role="presentation" onclick={() => (confirm = null)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <h3>{confirm === 'all' ? '¿Vaciar toda la cuarentena?' : '¿Borrar esta limpieza?'}</h3>
      <p>Esta acción es definitiva: los archivos ya no se podrán recuperar.</p>
      <div class="row">
        <button class="ghost" onclick={() => (confirm = null)}>Cancelar</button>
        <button class="cta" onclick={() => purge(confirm === 'all' ? null : confirm)}>Borrar definitivamente</button>
      </div>
    </div>
  </div>
{/if}

<style>
  header h1 { margin: 0; font-size: 28px; font-weight: 600; letter-spacing: -0.3px; }
  header p { margin: 6px 0 0; color: var(--mute); font-size: 14px; line-height: 1.55; max-width: 560px; }
  .where { display: flex; align-items: center; gap: 14px; margin: 26px 0 22px; padding: 16px 18px; border: 1px solid var(--hairline); border-radius: 16px; background: var(--surface); }
  .where > div { flex: 1; min-width: 0; display: grid; gap: 3px; }
  .where small { color: var(--mute); font-size: 12px; }
  code { font-family: 'Cascadia Mono', Consolas, monospace; font-size: 12.5px; color: var(--body); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; user-select: text; }
  .ico { display: grid; place-items: center; width: 40px; height: 40px; border-radius: 11px; color: var(--blue); background: var(--glow); }
  .head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; font-size: 13px; color: var(--mute); }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  li { display: flex; align-items: center; gap: 10px; padding: 14px 16px 14px 18px; border: 1px solid var(--hairline); border-radius: 14px; background: var(--surface); animation: rise 0.4s cubic-bezier(0.2, 0.7, 0.2, 1) both; }
  @keyframes rise { from { opacity: 0; transform: translateY(8px); } }
  .info { flex: 1; display: grid; gap: 2px; }
  .info b { font-weight: 500; font-size: 14.5px; }
  .info small { color: var(--mute); font-size: 12.5px; }
  .ghost { display: inline-flex; align-items: center; gap: 7px; border: 1px solid var(--hairline-strong); background: none; border-radius: 99px; padding: 8px 16px; color: var(--body); cursor: pointer; font-size: 13px; transition: background 0.15s; }
  .ghost:hover { background: var(--hairline); }
  .icon { display: grid; place-items: center; width: 34px; height: 34px; border: 0; border-radius: 10px; background: none; color: var(--mute); cursor: pointer; transition: all 0.15s; }
  .icon:hover { color: var(--red); background: color-mix(in srgb, var(--red) 14%, transparent); }
  .danger { display: inline-flex; align-items: center; gap: 6px; border: 0; background: none; color: var(--red); cursor: pointer; font-size: 13px; padding: 6px 10px; border-radius: 8px; }
  .danger:hover { background: color-mix(in srgb, var(--red) 14%, transparent); }
  .empty { display: grid; justify-items: center; gap: 6px; padding: 70px 0; color: var(--mute); text-align: center; }
  .empty span { display: grid; place-items: center; width: 64px; height: 64px; border-radius: 20px; color: var(--blue); background: var(--glow); margin-bottom: 8px; }
  .empty b { color: var(--ink); font-weight: 500; }
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.55); backdrop-filter: blur(6px); display: grid; place-items: center; animation: fade 0.2s; z-index: 10; }
  @keyframes fade { from { opacity: 0; } }
  .dialog { width: 380px; background: var(--elevated); border: 1px solid var(--hairline-strong); border-radius: 20px; padding: 26px; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5); animation: pop 0.28s cubic-bezier(0.2, 0.9, 0.3, 1.2); }
  @keyframes pop { from { opacity: 0; transform: scale(0.94); } }
  .dialog h3 { margin: 0 0 6px; font-size: 19px; font-weight: 600; }
  .dialog p { margin: 0 0 22px; color: var(--mute); font-size: 14px; line-height: 1.55; }
  .row { display: flex; justify-content: flex-end; gap: 10px; }
  .cta { border: 0; border-radius: 99px; padding: 9px 20px; font-weight: 600; font-size: 14px; background: var(--red); color: #fff; cursor: pointer; }
</style>
