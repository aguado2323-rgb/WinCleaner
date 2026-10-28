//! Quitar aplicaciones preinstaladas (Appx) de la lista curada `rules/debloat.toml`.
//!
//! Seguridad: la interfaz solo envía ids del catálogo; el nombre del paquete que llega a
//! PowerShell sale siempre del catálogo (que solo admite letras, cifras y puntos), nunca del
//! texto de la interfaz. Se quita solo para el usuario actual, así que no se toca la imagen de
//! Windows y cualquier app se reinstala desde Microsoft Store.

#![cfg_attr(not(windows), allow(dead_code))]

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Safe,
    Cautious,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub package: String,
    pub name: String,
    pub category: String,
    pub what: String,
    pub impact: String,
    pub risk: Risk,
}

#[derive(Deserialize)]
struct CatalogFile {
    app: Vec<App>,
}

const CATALOG: &str = include_str!("../rules/debloat.toml");

pub fn catalog() -> Vec<App> {
    toml::from_str::<CatalogFile>(CATALOG).expect("debloat.toml embebido es válido").app
}

/// Una app del catálogo que está instalada en este equipo.
#[derive(Debug, Clone, Serialize)]
pub struct Installed {
    #[serde(flatten)]
    pub app: App,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoveResult {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// Nombres de paquete instalados a partir de `Get-AppxPackage | ConvertTo-Json`.
/// Acepta un objeto suelto, una lista o nada.
pub fn parse_installed(json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(json.trim()) else { return Vec::new() };
    let items = match value {
        Value::Array(a) => a,
        v @ Value::Object(_) => vec![v],
        _ => return Vec::new(),
    };
    items
        .iter()
        .filter_map(|o| Some(o.get("Name")?.as_str()?.to_string()))
        .collect()
}

/// Cruza lo instalado con el catálogo. La comparación no distingue mayúsculas
/// (Windows las trata igual) y es del nombre completo, nunca parcial.
pub fn match_installed(installed: &[String]) -> Vec<Installed> {
    catalog()
        .into_iter()
        .filter(|app| installed.iter().any(|n| n.eq_ignore_ascii_case(&app.package)))
        .map(|app| Installed { app })
        .collect()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn powershell(script: &str) -> std::io::Result<std::process::Output> {
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    pub fn scan() -> Result<Vec<Installed>, String> {
        let out = powershell("Get-AppxPackage | Select-Object Name | ConvertTo-Json -Compress").map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(match_installed(&parse_installed(&String::from_utf8_lossy(&out.stdout))))
    }

    pub fn remove(ids: &[String]) -> Vec<RemoveResult> {
        let cat = catalog();
        ids.iter()
            .map(|id| {
                let Some(app) = cat.iter().find(|a| &a.id == id) else {
                    return RemoveResult { id: id.clone(), ok: false, error: Some("no está en el catálogo".into()) };
                };
                // `package` viene del catálogo (validado por tests: solo letras, cifras y puntos).
                let script = format!(
                    "Get-AppxPackage -Name '{}' | Remove-AppxPackage -ErrorAction Stop",
                    app.package
                );
                match powershell(&script) {
                    Ok(o) if o.status.success() => RemoveResult { id: id.clone(), ok: true, error: None },
                    Ok(o) => {
                        let msg = String::from_utf8_lossy(&o.stderr);
                        let first = msg.lines().find(|l| !l.trim().is_empty()).unwrap_or("error desconocido");
                        RemoveResult { id: id.clone(), ok: false, error: Some(first.trim().to_string()) }
                    }
                    Err(e) => RemoveResult { id: id.clone(), ok: false, error: Some(e.to_string()) },
                }
            })
            .collect()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn scan() -> Result<Vec<Installed>, String> {
        Err("solo disponible en Windows".into())
    }
    pub fn remove(ids: &[String]) -> Vec<RemoveResult> {
        ids.iter()
            .map(|id| RemoveResult { id: id.clone(), ok: false, error: Some("solo disponible en Windows".into()) })
            .collect()
    }
}

pub use imp::{remove as debloat_remove, scan as debloat_scan};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        let apps = catalog();
        assert!(apps.len() > 20);
        let mut ids: Vec<_> = apps.iter().map(|a| a.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), apps.len(), "ids duplicados");
        for a in &apps {
            // Esto es lo que garantiza que el nombre es seguro dentro del comando de PowerShell.
            assert!(
                !a.package.is_empty() && a.package.chars().all(|c| c.is_ascii_alphanumeric() || c == '.'),
                "paquete no válido: {}",
                a.package
            );
            assert!(!a.what.is_empty() && !a.impact.is_empty(), "{} sin explicación", a.id);
        }
    }

    #[test]
    fn catalog_never_lists_system_components() {
        let forbidden = [
            "WindowsStore", "DesktopAppInstaller", "Edge", "WebView", "ShellExperienceHost",
            "StartMenuExperienceHost", "SecHealthUI", "Windows.Search", "VCLibs", "NET.Native",
            "UI.Xaml", "WindowsTerminal", "Photos", "Calculator", "Notepad", "Paint",
            "AAD.BrokerPlugin", "CloudExperienceHost", "LockApp", "Windows.CBS", "XboxIdentityProvider",
        ];
        for a in catalog() {
            for f in forbidden {
                assert!(!a.package.contains(f), "{} parece un componente del sistema", a.package);
            }
        }
    }

    #[test]
    fn parses_list_object_and_empty() {
        let many = r#"[{"Name":"Microsoft.BingNews"},{"Name":"Otra.App"}]"#;
        assert_eq!(parse_installed(many).len(), 2);
        assert_eq!(parse_installed(r#"{"Name":"Microsoft.BingNews"}"#).len(), 1);
        assert!(parse_installed("").is_empty());
        assert!(parse_installed("basura").is_empty());
    }

    #[test]
    fn matches_whole_names_only() {
        let installed = vec!["microsoft.bingnews".to_string(), "Microsoft.BingNewsExtra".into(), "Otra.App".into()];
        let found = match_installed(&installed);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].app.id, "bing-news");
    }
}
