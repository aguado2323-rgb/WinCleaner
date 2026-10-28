//! Planes de energía de Windows (`powercfg`): ver cuál está activo, cambiar y añadir
//! «Máximo rendimiento». Útil sobre todo en portátiles y consolas portátiles.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Plan {
    pub guid: String,
    pub name: String,
    pub active: bool,
}

/// GUID del plan oculto «Máximo rendimiento» (Ultimate Performance), que se puede duplicar.
const ULTIMATE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";

fn is_guid(s: &str) -> bool {
    s.len() == 36
        && s.chars().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Lee la salida de `powercfg /list` sin depender del idioma: busca el GUID en cada línea,
/// el nombre entre paréntesis y un `*` final para el plan activo.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_plans(text: &str) -> Vec<Plan> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(guid) = line.split_whitespace().find(|w| is_guid(w)) else { continue };
        let name = match (line.find('('), line.rfind(')')) {
            (Some(a), Some(b)) if b > a => line[a + 1..b].trim().to_string(),
            _ => guid.to_string(),
        };
        out.push(Plan { guid: guid.to_lowercase(), name, active: line.trim_end().ends_with('*') });
    }
    out
}

pub fn power_plans() -> Result<Vec<Plan>, String> {
    imp::list()
}

/// Cambia el plan activo. Solo acepta un GUID que exista ahora mismo en la lista de Windows.
pub fn power_set(guid: &str) -> Result<(), String> {
    if !is_guid(guid) {
        return Err("plan no válido".into());
    }
    let plans = imp::list()?;
    if !plans.iter().any(|p| p.guid == guid.to_lowercase()) {
        return Err("ese plan ya no existe".into());
    }
    imp::set(&guid.to_lowercase())
}

/// Crea el plan «Máximo rendimiento» si no está, y lo deja listo para elegir.
pub fn power_add_ultimate() -> Result<(), String> {
    let plans = imp::list()?;
    if plans.iter().any(|p| p.name.to_lowercase().contains("ultimate") || p.name.to_lowercase().contains("máximo")) {
        return Ok(());
    }
    imp::duplicate(ULTIMATE)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn powercfg(args: &[&str]) -> Result<std::process::Output, String> {
        Command::new("powercfg").args(args).creation_flags(CREATE_NO_WINDOW).output().map_err(|e| e.to_string())
    }

    pub fn list() -> Result<Vec<Plan>, String> {
        let out = powercfg(&["/list"])?;
        Ok(parse_plans(&String::from_utf8_lossy(&out.stdout)))
    }

    pub fn set(guid: &str) -> Result<(), String> {
        let out = powercfg(&["/setactive", guid])?;
        if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
    }

    pub fn duplicate(guid: &str) -> Result<(), String> {
        let out = powercfg(&["/duplicatescheme", guid])?;
        if out.status.success() {
            Ok(())
        } else {
            Err("este equipo no ofrece el plan «Máximo rendimiento»".into())
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn list() -> Result<Vec<Plan>, String> {
        Err("Solo disponible en Windows".into())
    }

    pub fn set(_: &str) -> Result<(), String> {
        Err("Solo disponible en Windows".into())
    }

    pub fn duplicate(_: &str) -> Result<(), String> {
        Err("Solo disponible en Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_english_and_spanish_lists() {
        let en = "Existing Power Schemes (* Active)\n-----------------------------------\n\
Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced) *\n\
Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)\n";
        let p = parse_plans(en);
        assert_eq!(p.len(), 2);
        assert!(p[0].active && p[0].name == "Balanced");
        assert!(!p[1].active);
        let es = "GUID del esquema de energía: 381b4222-f694-41f0-9685-ff5bb260df2e  (Equilibrado) *\n";
        assert_eq!(parse_plans(es)[0].name, "Equilibrado");
    }

    #[test]
    fn guids_are_validated() {
        assert!(is_guid(ULTIMATE));
        assert!(!is_guid("e9a42b02-d5df-448d-aa00-03f14749eb6"));
        assert!(!is_guid("x; calc"));
        assert!(power_set("--help").is_err());
    }
}
