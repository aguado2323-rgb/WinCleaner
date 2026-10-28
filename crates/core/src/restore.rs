//! Puntos de restauración del sistema. Se delega en las herramientas de Windows
//! (PowerShell y `rstrui.exe`); WinCleaner nunca restaura por su cuenta.

#![cfg_attr(not(windows), allow(dead_code))]

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RestorePoint {
    pub sequence: u64,
    pub description: String,
    /// `AAAA-MM-DD HH:MM:SS`, hora local.
    pub created: String,
    pub kind: String,
}

/// Motivos permitidos para crear un punto. La descripción sale de aquí y nunca de
/// texto libre, así que no se puede colar nada en el comando de PowerShell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Manual,
    Cleaning,
    Debloat,
    Uninstall,
}

impl Reason {
    pub fn description(self) -> &'static str {
        match self {
            Reason::Manual => "WinCleaner - punto manual",
            Reason::Cleaning => "WinCleaner - antes de limpiar",
            Reason::Debloat => "WinCleaner - antes de quitar aplicaciones",
            Reason::Uninstall => "WinCleaner - antes de desinstalar",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "manual" => Some(Reason::Manual),
            "cleaning" => Some(Reason::Cleaning),
            "debloat" => Some(Reason::Debloat),
            "uninstall" => Some(Reason::Uninstall),
            _ => None,
        }
    }
}

/// `20261028102233.123456-000` (formato WMI) → `2026-10-28 10:22:33`.
fn wmi_date(raw: &str) -> String {
    let d: String = raw.chars().take(14).collect();
    if d.len() == 14 && d.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{} {}:{}:{}", &d[0..4], &d[4..6], &d[6..8], &d[8..10], &d[10..12], &d[12..14])
    } else {
        raw.to_string()
    }
}

fn kind_name(code: u64) -> &'static str {
    match code {
        0 => "Instalación de aplicación",
        1 => "Desinstalación de aplicación",
        10 => "Instalación de controlador",
        12 => "Cambio de configuración",
        13 => "Operación cancelada",
        _ => "Otro",
    }
}

/// Interpreta la salida JSON de `Get-ComputerRestorePoint | ConvertTo-Json`.
/// PowerShell devuelve un objeto suelto si solo hay un punto, y nada si no hay ninguno.
pub fn parse_points(json: &str) -> Vec<RestorePoint> {
    let Ok(value) = serde_json::from_str::<Value>(json.trim()) else { return Vec::new() };
    let items = match value {
        Value::Array(a) => a,
        v @ Value::Object(_) => vec![v],
        _ => return Vec::new(),
    };
    let mut points: Vec<_> = items
        .iter()
        .filter_map(|o| {
            Some(RestorePoint {
                sequence: o.get("SequenceNumber")?.as_u64()?,
                description: o.get("Description")?.as_str()?.to_string(),
                created: wmi_date(o.get("CreationTime").and_then(Value::as_str).unwrap_or("")),
                kind: kind_name(o.get("RestorePointType").and_then(Value::as_u64).unwrap_or(99)).into(),
            })
        })
        .collect();
    points.sort_by_key(|p| std::cmp::Reverse(p.sequence));
    points
}

/// Traduce el error de PowerShell a algo que una persona entienda.
pub fn friendly_error(stderr: &str) -> String {
    let low = stderr.to_lowercase();
    if low.contains("access") || low.contains("acceso") || low.contains("denied") || low.contains("denegado") {
        "Hace falta ejecutar WinCleaner como administrador.".into()
    } else if low.contains("1440") || low.contains("already been created") || low.contains("ya se ha creado") {
        "Windows solo permite crear un punto de restauración cada 24 horas; ya hay uno reciente.".into()
    } else if low.contains("disabled") || low.contains("deshabilit") || low.contains("turned off") {
        "La protección del sistema está desactivada en este disco. Actívala en «Restaurar sistema».".into()
    } else {
        let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("error desconocido");
        format!("No se pudo crear el punto: {}", first.trim())
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Output};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn powershell(script: &str) -> std::io::Result<Output> {
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    pub fn list() -> Vec<RestorePoint> {
        let script = "Get-ComputerRestorePoint | Select-Object SequenceNumber,Description,CreationTime,RestorePointType | ConvertTo-Json -Compress";
        match powershell(script) {
            Ok(out) => parse_points(&String::from_utf8_lossy(&out.stdout)),
            Err(_) => Vec::new(),
        }
    }

    pub fn is_admin() -> bool {
        let script = "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";
        powershell(script)
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    pub fn create(reason: Reason) -> Result<(), String> {
        // La descripción es una constante de la lista cerrada `Reason`; no contiene comillas.
        let script = format!(
            "Checkpoint-Computer -Description '{}' -RestorePointType 'MODIFY_SETTINGS' -WarningAction Stop -ErrorAction Stop",
            reason.description()
        );
        let out = powershell(&script).map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(friendly_error(&String::from_utf8_lossy(&out.stderr)))
        }
    }

    /// Abre el asistente oficial de Windows para volver a un punto anterior.
    pub fn open_system_restore() -> Result<(), String> {
        Command::new("rstrui.exe").spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    /// Vuelve a lanzar WinCleaner con permisos de administrador (aviso UAC de Windows).
    pub fn relaunch_as_admin() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "Start-Process -FilePath $env:WC_EXE -Verb RunAs"])
            .env("WC_EXE", exe)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn list() -> Vec<RestorePoint> {
        Vec::new()
    }
    pub fn is_admin() -> bool {
        false
    }
    pub fn create(_reason: Reason) -> Result<(), String> {
        Err("solo disponible en Windows".into())
    }
    pub fn open_system_restore() -> Result<(), String> {
        Err("solo disponible en Windows".into())
    }
    pub fn relaunch_as_admin() -> Result<(), String> {
        Err("solo disponible en Windows".into())
    }
}

pub use imp::{create as create_restore_point, is_admin, list as list_restore_points, open_system_restore, relaunch_as_admin};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_and_single_object() {
        let many = r#"[{"SequenceNumber":3,"Description":"B","CreationTime":"20261028102233.123456-000","RestorePointType":12},
                       {"SequenceNumber":2,"Description":"A","CreationTime":"20261027080000.000000-000","RestorePointType":0}]"#;
        let pts = parse_points(many);
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0].sequence, 3);
        assert_eq!(pts[0].created, "2026-10-28 10:22:33");
        assert_eq!(pts[0].kind, "Cambio de configuración");

        let one = r#"{"SequenceNumber":7,"Description":"Solo","CreationTime":"20260101000000.000000-000","RestorePointType":1}"#;
        assert_eq!(parse_points(one).len(), 1);
    }

    #[test]
    fn empty_or_garbage_gives_no_points() {
        assert!(parse_points("").is_empty());
        assert!(parse_points("null").is_empty());
        assert!(parse_points("no es json").is_empty());
    }

    #[test]
    fn reasons_are_a_closed_list_without_quotes() {
        for r in ["manual", "cleaning", "debloat", "uninstall"] {
            let d = Reason::parse(r).unwrap().description();
            assert!(!d.contains('\'') && !d.contains('"') && !d.contains('`') && !d.contains('$'));
        }
        assert!(Reason::parse("'; Remove-Item *").is_none());
    }

    #[test]
    fn errors_are_translated() {
        assert!(friendly_error("Access is denied").contains("administrador"));
        assert!(friendly_error("... within the past 1440 minutes").contains("24 horas"));
        assert!(friendly_error("").starts_with("No se pudo"));
    }
}
