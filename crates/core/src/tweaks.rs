//! Ajustes de Windows (privacidad, rendimiento e interfaz). Cada ajuste es un conjunto de valores
//! de registro de un catálogo cerrado (`rules/tweaks.toml`). Antes de cambiar un valor se guarda el
//! original, así que cada ajuste se puede deshacer exactamente. No se ejecuta ningún comando.

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegEntry {
    pub hive: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tweak {
    pub id: String,
    pub name: String,
    pub category: String,
    pub what: String,
    pub help: String,
    pub risk: Risk,
    pub restart: String,
    #[serde(default, skip_serializing)]
    pub reg: Vec<RegEntry>,
}

#[derive(Deserialize)]
struct CatalogFile {
    tweak: Vec<Tweak>,
}

const CATALOG: &str = include_str!("../rules/tweaks.toml");

pub fn catalog() -> Vec<Tweak> {
    toml::from_str::<CatalogFile>(CATALOG).expect("tweaks.toml embebido es válido").tweak
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Val {
    Dword(u32),
    Str(String),
}

impl RegEntry {
    /// Valor que escribe el ajuste. `None` si el catálogo está mal formado (lo vigilan los tests).
    pub fn target(&self) -> Option<Val> {
        match self.kind.as_str() {
            "dword" => self.value.parse().ok().map(Val::Dword),
            "string" => Some(Val::Str(self.value.clone())),
            _ => None,
        }
    }
}

/// Acceso al registro. En Windows es el registro real; en los tests, uno en memoria.
pub trait Registry {
    fn get(&self, e: &RegEntry) -> Result<Option<Val>, String>;
    fn set(&mut self, e: &RegEntry, v: &Val) -> Result<(), String>;
    fn delete(&mut self, e: &RegEntry) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    entry: RegEntry,
    original: Option<Val>,
}

type State = BTreeMap<String, Vec<Saved>>;

fn state_file(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("tweaks-state.json")
}

fn load_state(data_dir: &Path) -> State {
    std::fs::read(state_file(data_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_state(data_dir: &Path, st: &State) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(st).map_err(|e| e.to_string())?;
    std::fs::write(state_file(data_dir), bytes).map_err(|e| e.to_string())
}

pub fn needs_admin(t: &Tweak) -> bool {
    t.reg.iter().any(|e| e.hive == "HKLM")
}

/// ¿Todos los valores del ajuste están ya en su valor objetivo?
pub fn is_applied(reg: &dyn Registry, t: &Tweak) -> bool {
    t.reg.iter().all(|e| e.target().is_some() && reg.get(e).ok().flatten() == e.target())
}

/// Aplica el ajuste guardando antes los valores originales. Si algo falla a medias, deshace lo hecho.
pub fn apply(reg: &mut dyn Registry, t: &Tweak, data_dir: &Path) -> Result<(), String> {
    let mut state = load_state(data_dir);
    // No se pisan los originales ya guardados (si se aplica dos veces, el original sigue siendo el primero).
    if !state.contains_key(&t.id) {
        let mut saved = Vec::new();
        for e in &t.reg {
            saved.push(Saved { entry: e.clone(), original: reg.get(e)? });
        }
        state.insert(t.id.clone(), saved);
        save_state(data_dir, &state)?;
    }
    let saved = state[&t.id].clone();
    for (done, e) in t.reg.iter().enumerate() {
        let target = e.target().ok_or("ajuste mal definido")?;
        if let Err(err) = reg.set(e, &target) {
            // Vuelve atrás lo ya escrito para no dejar el ajuste a medias.
            for s in saved.iter().take(done) {
                let _ = restore_one(reg, s);
            }
            state.remove(&t.id);
            let _ = save_state(data_dir, &state);
            return Err(err);
        }
    }
    Ok(())
}

fn restore_one(reg: &mut dyn Registry, s: &Saved) -> Result<(), String> {
    match &s.original {
        Some(v) => reg.set(&s.entry, v),
        None => reg.delete(&s.entry),
    }
}

/// Devuelve cada valor a como estaba antes de aplicar el ajuste. Si no hay copia guardada
/// (se aplicó desde fuera), se quitan los valores del ajuste para volver al comportamiento de Windows.
pub fn revert(reg: &mut dyn Registry, t: &Tweak, data_dir: &Path) -> Result<(), String> {
    let mut state = load_state(data_dir);
    let saved = state
        .remove(&t.id)
        .unwrap_or_else(|| t.reg.iter().map(|e| Saved { entry: e.clone(), original: None }).collect());
    let mut first_err = None;
    for s in &saved {
        // Solo se restauran valores que pertenecen al ajuste, aunque el archivo de estado se haya editado.
        if !t.reg.contains(&s.entry) {
            continue;
        }
        if let Err(e) = restore_one(reg, s) {
            first_err.get_or_insert(e);
        }
    }
    save_state(data_dir, &state)?;
    first_err.map_or(Ok(()), Err)
}

#[derive(Debug, Clone, Serialize)]
pub struct TweakInfo {
    #[serde(flatten)]
    pub tweak: Tweak,
    pub applied: bool,
    pub needs_admin: bool,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use winreg::enums::*;
    use winreg::{RegKey, RegValue};

    pub struct WinReg;

    fn root(e: &RegEntry) -> RegKey {
        RegKey::predef(if e.hive == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
    }

    fn friendly(e: std::io::Error) -> String {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            "Windows no permite este cambio: ejecuta WinCleaner como administrador (o tu versión de Windows protege este ajuste).".into()
        } else {
            e.to_string()
        }
    }

    impl Registry for WinReg {
        fn get(&self, e: &RegEntry) -> Result<Option<Val>, String> {
            let Ok(key) = root(e).open_subkey_with_flags(&e.path, KEY_READ) else { return Ok(None) };
            let Ok(raw) = key.get_raw_value(&e.name) else { return Ok(None) };
            match raw.vtype {
                REG_DWORD if raw.bytes.len() == 4 => {
                    Ok(Some(Val::Dword(u32::from_le_bytes([raw.bytes[0], raw.bytes[1], raw.bytes[2], raw.bytes[3]]))))
                }
                REG_SZ => key.get_value::<String, _>(&e.name).map(|s| Some(Val::Str(s))).map_err(friendly),
                // Un tipo distinto del esperado no se sobrescribe: sería perder datos que no sabemos restaurar.
                _ => Err(format!("{}: el valor actual tiene un tipo inesperado", e.name)),
            }
        }

        fn set(&mut self, e: &RegEntry, v: &Val) -> Result<(), String> {
            let (key, _) = root(e).create_subkey_with_flags(&e.path, KEY_WRITE).map_err(friendly)?;
            match v {
                Val::Dword(d) => key.set_value(&e.name, d),
                Val::Str(s) => key.set_raw_value(
                    &e.name,
                    &RegValue {
                        bytes: s.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect(),
                        vtype: REG_SZ,
                    },
                ),
            }
            .map_err(friendly)
        }

        fn delete(&mut self, e: &RegEntry) -> Result<(), String> {
            let Ok(key) = root(e).open_subkey_with_flags(&e.path, KEY_WRITE) else { return Ok(()) };
            match key.delete_value(&e.name) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(friendly(err)),
            }
        }
    }
}

#[cfg(windows)]
fn real() -> impl Registry {
    imp::WinReg
}

#[cfg(not(windows))]
fn real() -> impl Registry {
    struct Unsupported;
    impl Registry for Unsupported {
        fn get(&self, _: &RegEntry) -> Result<Option<Val>, String> {
            Ok(None)
        }
        fn set(&mut self, _: &RegEntry, _: &Val) -> Result<(), String> {
            Err("solo disponible en Windows".into())
        }
        fn delete(&mut self, _: &RegEntry) -> Result<(), String> {
            Err("solo disponible en Windows".into())
        }
    }
    Unsupported
}

pub fn tweaks_list() -> Vec<TweakInfo> {
    let reg = real();
    catalog()
        .into_iter()
        .map(|t| TweakInfo { applied: is_applied(&reg, &t), needs_admin: needs_admin(&t), tweak: t })
        .collect()
}

/// Aplica (`enable`) o deshace un ajuste del catálogo por id.
pub fn tweak_set(id: &str, enable: bool, data_dir: &Path) -> Result<(), String> {
    let t = catalog().into_iter().find(|t| t.id == id).ok_or("ajuste desconocido")?;
    let mut reg = real();
    if enable {
        apply(&mut reg, &t, data_dir)
    } else {
        revert(&mut reg, &t, data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake {
        map: HashMap<(String, String, String), Val>,
        fail_on: Option<String>,
    }
    fn k(e: &RegEntry) -> (String, String, String) {
        (e.hive.clone(), e.path.to_lowercase(), e.name.to_lowercase())
    }
    impl Registry for Fake {
        fn get(&self, e: &RegEntry) -> Result<Option<Val>, String> {
            Ok(self.map.get(&k(e)).cloned())
        }
        fn set(&mut self, e: &RegEntry, v: &Val) -> Result<(), String> {
            if self.fail_on.as_deref() == Some(e.name.as_str()) {
                return Err("denegado".into());
            }
            self.map.insert(k(e), v.clone());
            Ok(())
        }
        fn delete(&mut self, e: &RegEntry) -> Result<(), String> {
            self.map.remove(&k(e));
            Ok(())
        }
    }

    fn two_value_tweak() -> Tweak {
        let e = |n: &str, v: &str| RegEntry { hive: "HKCU".into(), path: r"Software\X".into(), name: n.into(), kind: "dword".into(), value: v.into() };
        Tweak { id: "t".into(), name: "T".into(), category: "c".into(), what: "w".into(), help: "h".into(), risk: Risk::Safe, restart: "none".into(), reg: vec![e("A", "0"), e("B", "1")] }
    }

    #[test]
    fn catalog_is_well_formed_and_safe() {
        let cat = catalog();
        assert!(cat.len() >= 20);
        let mut ids: Vec<_> = cat.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), cat.len(), "ids duplicados");
        let banned = ["\\run", "runonce", "\\services\\", "winlogon", "image file execution", "defender", "\\lsa", "cryptography", "securityproviders", "appinit", "\\safer"];
        for t in &cat {
            assert!(t.help.len() > 40 && !t.what.is_empty(), "{} sin explicación", t.id);
            assert!(!t.reg.is_empty());
            assert!(["none", "explorer", "reboot"].contains(&t.restart.as_str()), "{}", t.id);
            for e in &t.reg {
                assert!(e.target().is_some(), "{}: valor no válido", t.id);
                let p = e.path.to_lowercase();
                let ok = match e.hive.as_str() {
                    "HKCU" => p.starts_with("software\\") || p.starts_with("control panel\\") || p == "system\\gameconfigstore",
                    "HKLM" => p.starts_with("software\\") || p == "system\\currentcontrolset\\control\\filesystem",
                    _ => false,
                };
                assert!(ok, "{}: ruta no permitida {} {}", t.id, e.hive, e.path);
                assert!(!e.name.is_empty() && !e.name.contains('\\'), "{}", t.id);
                for b in banned {
                    assert!(!p.contains(b), "{}: ruta sensible {}", t.id, e.path);
                }
            }
        }
    }

    #[test]
    fn apply_then_revert_restores_original_values() {
        let tmp = tempfile::tempdir().unwrap();
        let t = two_value_tweak();
        let mut reg = Fake::default();
        // A ya existía con otro valor; B no existía.
        reg.map.insert(k(&t.reg[0]), Val::Dword(7));

        assert!(!is_applied(&reg, &t));
        apply(&mut reg, &t, tmp.path()).unwrap();
        assert!(is_applied(&reg, &t));
        // Aplicar otra vez no pierde el original.
        apply(&mut reg, &t, tmp.path()).unwrap();

        revert(&mut reg, &t, tmp.path()).unwrap();
        assert_eq!(reg.get(&t.reg[0]).unwrap(), Some(Val::Dword(7)), "recupera el valor previo");
        assert_eq!(reg.get(&t.reg[1]).unwrap(), None, "quita lo que no existía");
        assert!(!is_applied(&reg, &t));
    }

    #[test]
    fn failed_apply_rolls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let t = two_value_tweak();
        let mut reg = Fake { fail_on: Some("B".into()), ..Default::default() };
        assert!(apply(&mut reg, &t, tmp.path()).is_err());
        assert_eq!(reg.get(&t.reg[0]).unwrap(), None, "A se deshace si B falla");
        assert!(!load_state(tmp.path()).contains_key("t"));
    }

    #[test]
    fn revert_without_saved_state_removes_values() {
        let tmp = tempfile::tempdir().unwrap();
        let t = two_value_tweak();
        let mut reg = Fake::default();
        for e in &t.reg {
            reg.set(e, &e.target().unwrap()).unwrap();
        }
        revert(&mut reg, &t, tmp.path()).unwrap();
        assert!(!is_applied(&reg, &t));
    }

    #[test]
    fn revert_ignores_tampered_state() {
        let tmp = tempfile::tempdir().unwrap();
        let t = two_value_tweak();
        let mut reg = Fake::default();
        let evil = RegEntry { hive: "HKLM".into(), path: r"SOFTWARE\Otro".into(), name: "X".into(), kind: "dword".into(), value: "1".into() };
        reg.map.insert(k(&evil), Val::Dword(5));
        let mut st = State::new();
        st.insert("t".into(), vec![Saved { entry: evil.clone(), original: None }]);
        save_state(tmp.path(), &st).unwrap();
        revert(&mut reg, &t, tmp.path()).unwrap();
        assert_eq!(reg.get(&evil).unwrap(), Some(Val::Dword(5)), "no toca valores ajenos al ajuste");
    }
}
