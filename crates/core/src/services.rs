//! Servicios de Windows que pasan de «Automático» a «Manual» (catálogo cerrado en `rules/services.toml`).
//! Antes de cambiar el tipo de inicio se guarda el original, así que cada cambio se deshace exactamente.
//! El servicio sigue instalado y no se detiene: «Manual» solo evita que arranque solo al encender.

#![cfg_attr(not(windows), allow(dead_code))]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Safe,
    Care,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub service: String,
    pub name: String,
    pub what: String,
    pub help: String,
    pub risk: Risk,
}

#[derive(Deserialize)]
struct CatalogFile {
    service: Vec<Service>,
}

const CATALOG: &str = include_str!("../rules/services.toml");

pub fn catalog() -> Vec<Service> {
    toml::from_str::<CatalogFile>(CATALOG).expect("services.toml embebido es válido").service
}

/// Tipo de inicio de un servicio. `Other` es arranque/sistema (0 y 1): no se toca nunca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Start {
    Auto,
    AutoDelayed,
    Manual,
    Disabled,
    Other,
}

impl Start {
    pub fn label(self) -> &'static str {
        match self {
            Start::Auto => "Automático",
            Start::AutoDelayed => "Automático (retrasado)",
            Start::Manual => "Manual",
            Start::Disabled => "Desactivado",
            Start::Other => "Sistema",
        }
    }

    /// Valor `Start` del registro del servicio y marca de inicio retrasado.
    pub fn from_registry(start: u32, delayed: bool) -> Start {
        match start {
            2 if delayed => Start::AutoDelayed,
            2 => Start::Auto,
            3 => Start::Manual,
            4 => Start::Disabled,
            _ => Start::Other,
        }
    }
}

/// Control de servicios. En Windows usa el registro (lectura) y `sc.exe` (cambio); en los tests, uno en memoria.
pub trait Services {
    fn get(&self, name: &str) -> Option<Start>;
    fn set(&mut self, name: &str, start: Start) -> Result<(), String>;
}

type State = BTreeMap<String, Start>;

fn state_file(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("services-state.json")
}

fn load_state(data_dir: &Path) -> State {
    std::fs::read(state_file(data_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_state(data_dir: &Path, st: &State) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(st).map_err(|e| e.to_string())?;
    std::fs::write(state_file(data_dir), bytes).map_err(|e| e.to_string())
}

/// Pasa el servicio a «Manual» guardando antes su tipo de inicio. Solo cambia servicios que ahora
/// mismo arrancan solos; uno ya en «Manual» o desactivado se deja como está.
pub fn apply(ctl: &mut dyn Services, s: &Service, data_dir: &Path) -> Result<(), String> {
    let current = ctl.get(&s.service).ok_or("este servicio no existe en tu Windows")?;
    if !matches!(current, Start::Auto | Start::AutoDelayed) {
        return Ok(());
    }
    let mut state = load_state(data_dir);
    // No se pisa el original ya guardado.
    state.entry(s.id.clone()).or_insert(current);
    save_state(data_dir, &state)?;
    if let Err(e) = ctl.set(&s.service, Start::Manual) {
        state.remove(&s.id);
        let _ = save_state(data_dir, &state);
        return Err(e);
    }
    Ok(())
}

/// Devuelve el servicio a su tipo de inicio original. Sin copia guardada no hay nada que deshacer
/// (WinCleaner no lo cambió).
pub fn revert(ctl: &mut dyn Services, s: &Service, data_dir: &Path) -> Result<(), String> {
    let mut state = load_state(data_dir);
    let Some(original) = state.get(&s.id).copied() else { return Ok(()) };
    // Un valor guardado raro (archivo editado) nunca es «sistema»: se vuelve a Automático.
    let target = if original == Start::Other { Start::Auto } else { original };
    ctl.set(&s.service, target)?;
    state.remove(&s.id);
    save_state(data_dir, &state)
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceInfo {
    #[serde(flatten)]
    pub service: Service,
    /// Tipo de inicio actual, en texto.
    pub start: String,
    /// ¿Está ya en «Manual» por haberlo cambiado WinCleaner?
    pub applied: bool,
}

/// Servicios del catálogo que existen y arrancan solos, o que WinCleaner ya pasó a «Manual».
pub fn list(ctl: &dyn Services, data_dir: &Path) -> Vec<ServiceInfo> {
    let state = load_state(data_dir);
    catalog()
        .into_iter()
        .filter_map(|s| {
            let cur = ctl.get(&s.service)?;
            let changed = state.contains_key(&s.id);
            if matches!(cur, Start::Auto | Start::AutoDelayed) || changed {
                Some(ServiceInfo { start: cur.label().into(), applied: changed && cur == Start::Manual, service: s })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use winreg::enums::*;
    use winreg::RegKey;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub struct Sc;

    impl Services for Sc {
        fn get(&self, name: &str) -> Option<Start> {
            let key = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(format!(r"SYSTEM\CurrentControlSet\Services\{name}"), KEY_READ)
                .ok()?;
            let start: u32 = key.get_value("Start").ok()?;
            let delayed = key.get_value::<u32, _>("DelayedAutostart").unwrap_or(0) == 1;
            Some(Start::from_registry(start, delayed))
        }

        fn set(&mut self, name: &str, start: Start) -> Result<(), String> {
            let mode = match start {
                Start::Auto => "auto",
                Start::AutoDelayed => "delayed-auto",
                Start::Manual => "demand",
                Start::Disabled => "disabled",
                Start::Other => return Err("tipo de inicio no permitido".into()),
            };
            // `name` sale siempre del catálogo (solo letras y cifras) y `mode` de una lista cerrada.
            let out = Command::new("sc.exe")
                .args(["config", name, "start=", mode])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(())
            } else if out.status.code() == Some(5) {
                Err("Windows no permite este cambio: ejecuta WinCleaner como administrador.".into())
            } else {
                Err(format!("no se pudo cambiar el servicio ({})", String::from_utf8_lossy(&out.stdout).trim()))
            }
        }
    }
}

#[cfg(windows)]
fn real() -> impl Services {
    imp::Sc
}

#[cfg(not(windows))]
fn real() -> impl Services {
    struct Unsupported;
    impl Services for Unsupported {
        fn get(&self, _: &str) -> Option<Start> {
            None
        }
        fn set(&mut self, _: &str, _: Start) -> Result<(), String> {
            Err("solo disponible en Windows".into())
        }
    }
    Unsupported
}

pub fn services_list(data_dir: &Path) -> Vec<ServiceInfo> {
    list(&real(), data_dir)
}

/// Pasa a «Manual» (`enable`) o deshace el cambio de un servicio del catálogo por id.
pub fn service_set(id: &str, enable: bool, data_dir: &Path) -> Result<(), String> {
    let s = catalog().into_iter().find(|s| s.id == id).ok_or("servicio desconocido")?;
    let mut ctl = real();
    if enable {
        apply(&mut ctl, &s, data_dir)
    } else {
        revert(&mut ctl, &s, data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake(HashMap<String, Start>, bool);
    impl Services for Fake {
        fn get(&self, name: &str) -> Option<Start> {
            self.0.get(name).copied()
        }
        fn set(&mut self, name: &str, start: Start) -> Result<(), String> {
            if self.1 {
                return Err("denegado".into());
            }
            self.0.insert(name.into(), start);
            Ok(())
        }
    }

    fn svc(id: &str, name: &str) -> Service {
        Service { id: id.into(), service: name.into(), name: "n".into(), what: "w".into(), help: "h".into(), risk: Risk::Safe }
    }

    #[test]
    fn catalog_is_closed_and_safe() {
        let cat = catalog();
        assert!(cat.len() >= 6);
        let mut ids: Vec<_> = cat.iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), cat.len(), "ids duplicados");
        let never = [
            "wuauserv", "bits", "windefend", "wdnissvc", "sense", "mpssvc", "securityhealthservice", "wscsvc", "rpcss",
            "dcomlaunch", "rpceptmapper", "dhcp", "dnscache", "eventlog", "lanmanserver", "lanmanworkstation", "audiosrv",
            "msiserver", "trustedinstaller", "cryptsvc", "gpsvc", "profsvc", "schedule", "wlansvc", "netlogon", "samss",
            "bfe", "keyiso", "power", "plugplay", "usosvc", "waasmedicsvc", "appidsvc", "winmgmt",
        ];
        for s in &cat {
            assert!(!s.service.is_empty() && s.service.chars().all(|c| c.is_ascii_alphanumeric()), "{}", s.service);
            assert!(!never.contains(&s.service.to_lowercase().as_str()), "{} es un servicio crítico", s.service);
            assert!(s.help.len() > 40 && !s.what.is_empty() && !s.name.is_empty(), "{} sin explicación", s.id);
        }
    }

    #[test]
    fn registry_values_map_to_start_types() {
        assert_eq!(Start::from_registry(2, false), Start::Auto);
        assert_eq!(Start::from_registry(2, true), Start::AutoDelayed);
        assert_eq!(Start::from_registry(3, false), Start::Manual);
        assert_eq!(Start::from_registry(4, false), Start::Disabled);
        assert_eq!(Start::from_registry(0, false), Start::Other);
        assert_eq!(Start::from_registry(1, false), Start::Other);
    }

    #[test]
    fn apply_then_revert_restores_original_start() {
        let tmp = tempfile::tempdir().unwrap();
        let s = svc("a", "Svc");
        let mut ctl = Fake::default();
        ctl.0.insert("Svc".into(), Start::AutoDelayed);

        apply(&mut ctl, &s, tmp.path()).unwrap();
        assert_eq!(ctl.get("Svc"), Some(Start::Manual));
        // Aplicar otra vez no pierde el original.
        apply(&mut ctl, &s, tmp.path()).unwrap();
        revert(&mut ctl, &s, tmp.path()).unwrap();
        assert_eq!(ctl.get("Svc"), Some(Start::AutoDelayed));
        // Sin copia guardada, deshacer no cambia nada.
        revert(&mut ctl, &s, tmp.path()).unwrap();
        assert_eq!(ctl.get("Svc"), Some(Start::AutoDelayed));
    }

    #[test]
    fn only_automatic_services_are_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let s = svc("a", "Svc");
        for start in [Start::Manual, Start::Disabled, Start::Other] {
            let mut ctl = Fake::default();
            ctl.0.insert("Svc".into(), start);
            apply(&mut ctl, &s, tmp.path()).unwrap();
            assert_eq!(ctl.get("Svc"), Some(start));
        }
        let mut ctl = Fake::default();
        assert!(apply(&mut ctl, &s, tmp.path()).is_err(), "servicio inexistente");
    }

    #[test]
    fn failed_change_leaves_no_saved_state_and_tampered_state_is_safe() {
        let tmp = tempfile::tempdir().unwrap();
        let s = svc("a", "Svc");
        let mut ctl = Fake::default();
        ctl.0.insert("Svc".into(), Start::Auto);
        ctl.1 = true;
        assert!(apply(&mut ctl, &s, tmp.path()).is_err());
        assert!(load_state(tmp.path()).is_empty());

        // Estado manipulado con un tipo «sistema»: se vuelve a Automático, nunca a Sistema.
        let mut st = State::new();
        st.insert("a".into(), Start::Other);
        save_state(tmp.path(), &st).unwrap();
        ctl.1 = false;
        ctl.0.insert("Svc".into(), Start::Manual);
        revert(&mut ctl, &s, tmp.path()).unwrap();
        assert_eq!(ctl.get("Svc"), Some(Start::Auto));
    }

    #[test]
    fn list_shows_only_automatic_or_changed_services() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctl = Fake::default();
        ctl.0.insert("DiagTrack".into(), Start::Auto);
        ctl.0.insert("SysMain".into(), Start::Manual);
        ctl.0.insert("Spooler".into(), Start::Auto);
        let cat = catalog();
        let spooler = cat.iter().find(|s| s.service == "Spooler").unwrap();
        apply(&mut ctl, spooler, tmp.path()).unwrap();

        let l = list(&ctl, tmp.path());
        let by = |n: &str| l.iter().find(|i| i.service.service == n);
        assert!(by("DiagTrack").is_some_and(|i| !i.applied));
        assert!(by("SysMain").is_none(), "ya en Manual de fábrica: no hay nada que ofrecer");
        assert!(by("Spooler").is_some_and(|i| i.applied));
        assert!(by("MapsBroker").is_none(), "no existe en este Windows");
    }
}
