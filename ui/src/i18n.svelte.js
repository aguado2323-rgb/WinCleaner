// Idioma de la interfaz. El español es el idioma de origen: `t('texto en español')` devuelve el
// texto tal cual y, en inglés, su traducción de `en.js` (o el español si aún no está traducido).
import { en } from './en.js';

// El idioma sigue siempre al de Windows: español si el equipo está en español, inglés si no.
// El selector ES/EN cambia el idioma solo mientras la app está abierta; al volver a abrirla manda Windows.
// Mientras llega la respuesta del sistema se usa el idioma del navegador integrado, que suele coincidir.
const guess = (navigator.language || 'es').toLowerCase().startsWith('es') ? 'es' : 'en';
let lang = $state(guess);
try {
  localStorage.removeItem('wc-lang'); // elección guardada por versiones anteriores: ya no se usa
} catch {
  /* sin almacenamiento */
}

export const i18n = {
  get lang() {
    return lang;
  },
  get locale() {
    return lang === 'en' ? 'en-GB' : 'es-ES';
  },
  set(l) {
    if (l !== 'es' && l !== 'en') return;
    lang = l;
    document.documentElement.lang = l;
  },
};

/** Pone el idioma de Windows. Lo llama la app al arrancar. */
export async function followSystem(invoke) {
  try {
    i18n.set(await invoke('system_lang'));
  } catch {
    /* sin respuesta: se queda el del navegador integrado */
  }
}

/** Traduce `es` y rellena los `{nombre}` con `params`. */
export function t(es, params) {
  let s = lang === 'en' ? (en[es] ?? es) : es;
  if (params) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));
  return s;
}
