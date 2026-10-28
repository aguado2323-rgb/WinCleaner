export function fmt(n) {
  if (!n) return '0 MB';
  if (n >= 1e9) return (n / 1e9).toFixed(2) + ' GB';
  if (n >= 1e6) return (n / 1e6).toFixed(n >= 1e8 ? 0 : 1) + ' MB';
  if (n >= 1e3) return Math.round(n / 1e3) + ' KB';
  return n + ' B';
}

export function fmtDate(batch) {
  return new Date(Number(batch) * 1000).toLocaleString('es-ES', { dateStyle: 'medium', timeStyle: 'short' });
}

export const accents = { Sistema: 'var(--blue)', Navegadores: 'var(--yellow)' };
