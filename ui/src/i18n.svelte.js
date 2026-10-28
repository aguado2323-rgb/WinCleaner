// Idioma de la interfaz. El español es el idioma de origen: `t('texto en español')` devuelve el
// texto tal cual y, en inglés, su traducción de `en.js` (o el español si aún no está traducido).
import { en } from './en.js';

const KEY = 'wc-lang';

function initial() {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved === 'es' || saved === 'en') return saved;
  } catch {
    /* sin almacenamiento: se sigue al idioma del sistema */
  }
  return (navigator.language || 'es').toLowerCase().startsWith('es') ? 'es' : 'en';
}

let lang = $state(initial());

export const i18n = {
  get lang() {
    return lang;
  },
  get locale() {
    return lang === 'en' ? 'en-GB' : 'es-ES';
  },
  set(l) {
    lang = l;
    try {
      localStorage.setItem(KEY, l);
    } catch {
      /* vale para esta sesión */
    }
    document.documentElement.lang = l;
  },
};

/** Traduce `es` y rellena los `{nombre}` con `params`. */
export function t(es, params) {
  let s = lang === 'en' ? (en[es] ?? es) : es;
  if (params) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));
  return s;
}
