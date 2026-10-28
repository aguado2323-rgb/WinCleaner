<script>
  // Icono «?» con la explicación de una opción: qué hace y cuánto riesgo tiene.
  let { text, risk = '', label = 'Qué es esto' } = $props();

  const RISK = {
    safe: 'Sin riesgo',
    reversible: 'Reversible',
    care: 'Con cuidado',
    final: 'Definitivo',
  };

  // Se monta en <body> para que ninguna animación ni transformación de un contenedor lo desplace.
  function portal(node) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  let open = $state(false);
  let btn;
  let pos = $state({ left: 0, top: 0, up: false });

  function show() {
    const r = btn.getBoundingClientRect();
    const w = 300;
    const left = Math.min(Math.max(12, r.left + r.width / 2 - w / 2), window.innerWidth - w - 12);
    const up = r.bottom + 190 > window.innerHeight && r.top > 200;
    pos = { left, top: up ? r.top - 8 : r.bottom + 8, up };
    open = true;
  }
</script>

<span class="help">
  <button
    bind:this={btn}
    type="button"
    class="q"
    aria-label={label}
    aria-expanded={open}
    onmouseenter={show}
    onmouseleave={() => (open = false)}
    onfocus={show}
    onblur={() => (open = false)}
    onclick={(e) => {
      e.preventDefault();
      e.stopPropagation();
      open ? (open = false) : show();
    }}
  >?</button>
  {#if open}
    <span use:portal class="pop" class:up={pos.up} role="tooltip" style="left:{pos.left}px;top:{pos.top}px">
      {#if risk}<b class="risk {risk}">{RISK[risk]}</b>{/if}
      {text}
    </span>
  {/if}
</span>

<style>
  .help { display: inline-flex; vertical-align: middle; }
  .q { flex: none; width: 17px; height: 17px; padding: 0; border: 1px solid var(--ink-3); border-radius: 50%; background: none; color: var(--ink-3); font-family: var(--mono); font-size: 10.5px; line-height: 1; cursor: help; transition: color 0.12s, border-color 0.12s, background 0.12s; }
  .q:hover, .q:focus-visible, .q[aria-expanded='true'] { background: var(--ink); border-color: var(--ink); color: var(--paper); outline: none; }
  .pop { position: fixed; z-index: 30; width: 300px; padding: 11px 13px; background: var(--paper); color: var(--ink-2); border: 1px solid var(--ink); box-shadow: 4px 4px 0 var(--ink); font-family: var(--font, inherit); font-size: 12.5px; font-weight: 400; line-height: 1.5; letter-spacing: 0; text-transform: none; text-align: left; pointer-events: none; animation: fade 0.12s; }
  .pop.up { transform: translateY(-100%); }
  .risk { display: block; margin-bottom: 4px; font-family: var(--mono); font-size: 10px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: var(--ink); }
  .risk.care, .risk.final { color: var(--accent); }
</style>
