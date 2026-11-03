// Estado de las actualizaciones de WinCleaner, compartido entre la barra lateral y la pantalla
// de Actualizaciones. La comprobación al abrir se puede apagar y la elección se recuerda.
import { invoke } from '@tauri-apps/api/core';

function readAuto() {
  try {
    return localStorage.getItem('wc-autoupdate') !== 'off';
  } catch {
    return true;
  }
}

export const selfUpdate = $state({
  info: null, // { current, latest, newer, installable, notes, page }
  checking: false,
  installing: false,
  error: '',
  auto: readAuto(),
});

export function setAuto(on) {
  selfUpdate.auto = on;
  try {
    localStorage.setItem('wc-autoupdate', on ? 'on' : 'off');
  } catch {
    /* vale para esta sesión */
  }
}

export async function checkSelfUpdate() {
  selfUpdate.checking = true;
  selfUpdate.error = '';
  try {
    selfUpdate.info = await invoke('self_update_check');
  } catch (e) {
    selfUpdate.error = String(e);
  }
  selfUpdate.checking = false;
  return selfUpdate.info;
}

/// Si sale bien, la app se cierra y se abre la versión nueva: no vuelve.
export async function installSelfUpdate() {
  selfUpdate.installing = true;
  selfUpdate.error = '';
  try {
    await invoke('self_update_install');
  } catch (e) {
    selfUpdate.error = String(e);
  }
  selfUpdate.installing = false;
}

export const openReleasePage = () => invoke('self_update_page');
