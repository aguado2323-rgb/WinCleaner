<script>
  import Help from './Help.svelte';
  import { H } from './help.js';
  import { invoke } from '@tauri-apps/api/core';

  let { notify } = $props();

  let admin = $state(false);
  let points = $state([]);
  let loading = $state(true);
  let busy = $state(false);

  async function load() {
    loading = true;
    try {
      const s = await invoke('restore_state');
      admin = s.admin;
      points = s.points;
    } catch (e) {
      notify('No se pudo leer la protección del sistema: ' + e);
    }
    loading = false;
  }

  async function create() {
    busy = true;
    try {
      await invoke('restore_create', { reason: 'manual' });
      notify('Punto de restauración creado');
      await load();
    } catch (e) {
      notify(String(e));
    }
    busy = false;
  }

  const openRestore = () => invoke('restore_open').catch((e) => notify('No se pudo abrir: ' + e));
  const elevate = () => invoke('restart_as_admin').catch((e) => notify(String(e)));

  load();
</script>

<p class="label">Restauración</p>
<h1 class="h1">Vuelve atrás si algo falla.</h1>
<p class="lede">Un punto de restauración es una foto de la configuración de Windows. Si un cambio sale mal, puedes volver a ese momento. Crea uno antes de tocar nada importante.</p>

{#if !loading && !admin}
  <section class="admin">
    <p><b>Modo administrador.</b> Windows exige permisos de administrador para crear puntos de restauración.</p>
    <span class="acts"><button class="btn" onclick={elevate}>Reiniciar como administrador</button>
    <Help text={H.restoreAdmin.text} risk={H.restoreAdmin.risk} label="Qué hace Reiniciar como administrador" /></span>
  </section>
{/if}

<section class="act">
  <button class="btn primary" disabled={busy || loading || !admin} onclick={create}>
    {busy ? 'Creando…' : 'Crear punto ahora'}
  </button>
  <Help text={H.restoreCreate.text} risk={H.restoreCreate.risk} label="Qué hace Crear punto" />
  <button class="link" onclick={openRestore}>Abrir Restaurar sistema ↗</button>
  <Help text={H.restoreOpen.text} risk={H.restoreOpen.risk} label="Qué hace Abrir Restaurar sistema" />
</section>
<p class="note">Windows solo permite un punto nuevo cada 24 horas. Para volver a uno anterior se usa el asistente oficial de Windows, no WinCleaner.</p>

<div class="head">
  <h2 class="label">Puntos guardados</h2>
  <span class="num">{points.length}</span>
</div>

{#if loading}
  <p class="empty num">···</p>
{:else if !points.length}
  <p class="empty">No hay puntos de restauración. Si la protección del sistema está desactivada, actívala desde «Abrir Restaurar sistema».</p>
{:else}
  <ul>
    {#each points as p, i (p.sequence)}
      <li style="animation-delay:{i * 24}ms">
        <span class="date num">{p.created}</span>
        <span class="desc">{p.description}</span>
        <span class="kind label">{p.kind}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .h1 { margin-top: 10px; }
  .admin { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin: 26px 0 0; padding: 14px 16px; border: 1px solid var(--ink); border-left: 6px solid var(--accent); }
  .admin p { margin: 0; color: var(--ink-2); line-height: 1.5; }
  .admin b { color: var(--ink); }
  .act { display: flex; align-items: center; gap: 26px; margin: 30px 0 14px; }
  .acts { display: flex; align-items: center; gap: 10px; flex: none; }
  .note { margin: 0 0 8px; color: var(--ink-3); font-size: 12.5px; max-width: 520px; line-height: 1.5; }
  .head { display: flex; align-items: baseline; justify-content: space-between; margin: 34px 0 4px; padding-bottom: 8px; border-bottom: 1px solid var(--ink); }
  .head h2 { margin: 0; color: var(--ink); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; align-items: baseline; gap: 20px; padding: 13px 2px; border-bottom: 1px solid var(--rule); animation: line 0.22s both; }
  .date { width: 150px; flex: none; font-size: 12.5px; }
  .desc { flex: 1; min-width: 0; font-weight: 500; }
  .kind { font-size: 10px; }
  .empty { padding: 40px 0; color: var(--ink-2); max-width: 480px; line-height: 1.55; }
</style>
