//! Gestor de arranque. Desactivar es reversible: se usa el mismo marcador que el
//! Administrador de tareas de Windows (`StartupApproved`) y nunca se borra ni se
//! edita la entrada original.

// En otros sistemas solo se usan las funciones puras (en los tests).
#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub command: String,
    /// "Usuario", "Sistema" o "Carpeta Inicio".
    pub source: String,
    pub enabled: bool,
    /// Cambiar el estado de las entradas del sistema requiere administrador.
    pub needs_admin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    UserRun,
    MachineRun,
    UserFolder,
}

impl Source {
    fn tag(self) -> &'static str {
        match self {
            Source::UserRun => "hkcu-run",
            Source::MachineRun => "hklm-run",
            Source::UserFolder => "folder",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Source::UserRun => "Usuario",
            Source::MachineRun => "Sistema",
            Source::UserFolder => "Carpeta Inicio",
        }
    }
}

pub fn make_id(source: Source, name: &str) -> String {
    format!("{}|{name}", source.tag())
}

pub fn parse_id(id: &str) -> Option<(Source, &str)> {
    let (tag, name) = id.split_once('|')?;
    let source = [Source::UserRun, Source::MachineRun, Source::UserFolder]
        .into_iter()
        .find(|s| s.tag() == tag)?;
    (!name.is_empty()).then_some((source, name))
}

/// Windows guarda 12 bytes por entrada; si el bit bajo del primer byte está activo, está desactivada.
pub fn approved_enabled(bytes: &[u8]) -> bool {
    bytes.first().is_none_or(|b| b & 1 == 0)
}

pub fn approved_value(enabled: bool) -> [u8; 12] {
    let mut v = [0u8; 12];
    v[0] = if enabled { 2 } else { 3 };
    v
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::fs;
    use winreg::enums::*;
    use winreg::{RegKey, RegValue};

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED_RUN: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    const APPROVED_FOLDER: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";

    fn root(source: Source) -> RegKey {
        match source {
            Source::MachineRun => RegKey::predef(HKEY_LOCAL_MACHINE),
            _ => RegKey::predef(HKEY_CURRENT_USER),
        }
    }

    fn approved_path(source: Source) -> &'static str {
        match source {
            Source::UserFolder => APPROVED_FOLDER,
            _ => APPROVED_RUN,
        }
    }

    fn is_enabled(source: Source, name: &str) -> bool {
        root(source)
            .open_subkey_with_flags(approved_path(source), KEY_READ)
            .and_then(|k| k.get_raw_value(name))
            .map(|v| approved_enabled(&v.bytes))
            .unwrap_or(true)
    }

    fn startup_dir() -> Option<std::path::PathBuf> {
        let appdata = std::env::var_os("APPDATA")?;
        Some(std::path::PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
    }

    pub fn list() -> Vec<StartupItem> {
        let mut out = Vec::new();
        for source in [Source::UserRun, Source::MachineRun] {
            let Ok(key) = root(source).open_subkey_with_flags(RUN, KEY_READ) else { continue };
            for (name, _) in key.enum_values().flatten() {
                let command = key.get_value::<String, _>(&name).unwrap_or_default();
                out.push(StartupItem {
                    id: make_id(source, &name),
                    enabled: is_enabled(source, &name),
                    name,
                    command,
                    source: source.label().into(),
                    needs_admin: source == Source::MachineRun,
                });
            }
        }
        if let Some(dir) = startup_dir() {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.eq_ignore_ascii_case("desktop.ini") {
                    continue;
                }
                out.push(StartupItem {
                    id: make_id(Source::UserFolder, &name),
                    enabled: is_enabled(Source::UserFolder, &name),
                    command: entry.path().display().to_string(),
                    name,
                    source: Source::UserFolder.label().into(),
                    needs_admin: false,
                });
            }
        }
        out.sort_by_key(|i| i.name.to_lowercase());
        out
    }

    pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
        let (source, name) = parse_id(id).ok_or("entrada no válida")?;
        // Solo se toca una entrada que existe de verdad en la lista.
        if !list().iter().any(|i| i.id == id) {
            return Err("la entrada ya no existe".into());
        }
        let (key, _) = root(source)
            .create_subkey_with_flags(approved_path(source), KEY_WRITE)
            .map_err(|e| e.to_string())?;
        let value = RegValue { bytes: approved_value(enabled).to_vec(), vtype: REG_BINARY };
        key.set_raw_value(name, &value).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn list() -> Vec<StartupItem> {
        Vec::new()
    }

    pub fn set_enabled(_id: &str, _enabled: bool) -> Result<(), String> {
        Err("solo disponible en Windows".into())
    }
}

pub use imp::{list, set_enabled};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_roundtrip() {
        for s in [Source::UserRun, Source::MachineRun, Source::UserFolder] {
            let id = make_id(s, "Mi app|con barra");
            assert_eq!(parse_id(&id), Some((s, "Mi app|con barra")));
        }
        assert_eq!(parse_id("nada"), None);
        assert_eq!(parse_id("hkcu-run|"), None);
        assert_eq!(parse_id("otro|x"), None);
    }

    #[test]
    fn approved_flag_matches_windows_convention() {
        assert!(approved_enabled(&approved_value(true)));
        assert!(!approved_enabled(&approved_value(false)));
        assert!(approved_enabled(&[0x06, 0, 0, 0])); // valor "habilitado" alternativo
        assert!(!approved_enabled(&[0x03, 0, 0, 0]));
        assert!(approved_enabled(&[])); // sin marcador = activa
    }
}
