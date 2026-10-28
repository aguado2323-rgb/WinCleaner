const nf = (d) => new Intl.NumberFormat('es-ES', { minimumFractionDigits: d, maximumFractionDigits: d });

/** Tamaño legible: { n: '3,24', u: 'GB' }. */
export function size(bytes) {
  if (!bytes) return { n: '0', u: 'MB' };
  if (bytes >= 1e9) return { n: nf(2).format(bytes / 1e9), u: 'GB' };
  if (bytes >= 1e6) return { n: nf(bytes >= 1e8 ? 0 : 1).format(bytes / 1e6), u: 'MB' };
  if (bytes >= 1e3) return { n: nf(0).format(bytes / 1e3), u: 'KB' };
  return { n: String(bytes), u: 'B' };
}

export function fmt(bytes) {
  const { n, u } = size(bytes);
  return `${n} ${u}`;
}

export function fmtDate(batch) {
  return new Date(Number(batch) * 1000).toLocaleString('es-ES', { dateStyle: 'medium', timeStyle: 'short' });
}

/** `2026-10-27` → `27 oct 2026`; vacío → `—`. */
export function fmtDay(iso) {
  if (!iso) return '—';
  const d = new Date(iso + 'T00:00:00');
  return isNaN(d) ? '—' : d.toLocaleDateString('es-ES', { day: 'numeric', month: 'short', year: 'numeric' }).replace(/\./g, '');
}
