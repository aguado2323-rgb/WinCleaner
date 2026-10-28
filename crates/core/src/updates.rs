//! Actualizar programas con winget (el gestor de paquetes oficial de Windows).
//! WinCleaner no descarga nada por su cuenta: delega en `winget`, que verifica los paquetes.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Update {
    pub name: String,
    pub id: String,
    pub current: String,
    pub available: String,
}

/// Lee la tabla que imprime `winget upgrade`. Las columnas se localizan por la posición de su
/// cabecera (que depende del idioma), no por su texto. Orden esperado: Nombre, Id, Versión,
/// Disponible, Origen.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_upgrades(text: &str) -> Vec<Update> {
    // winget pinta un indicador de progreso con `\r` sin salto de línea: se queda con el último tramo.
    let lines: Vec<&str> = text.lines().map(|l| l.rsplit('\r').find(|s| !s.is_empty()).unwrap_or("")).collect();
    let Some(sep) = lines.iter().position(|l| l.len() >= 10 && l.chars().all(|c| c == '-')) else {
        return Vec::new();
    };
    if sep == 0 {
        return Vec::new();
    }
    let header: Vec<char> = lines[sep - 1].chars().collect();
    // Inicio de cada columna: una palabra precedida de dos o más espacios (o el principio).
    let mut starts = Vec::new();
    let mut i = 0;
    while i < header.len() {
        if header[i] != ' ' && (i == 0 || (header[i - 1] == ' ' && i >= 2 && header[i - 2] == ' ')) {
            starts.push(i);
        }
        i += 1;
    }
    if starts.len() < 4 {
        return Vec::new();
    }
    let cell = |row: &[char], c: usize| -> String {
        let from = starts[c].min(row.len());
        let to = starts.get(c + 1).copied().unwrap_or(row.len()).min(row.len());
        row[from..to].iter().collect::<String>().trim().to_string()
    };
    let mut out = Vec::new();
    for line in &lines[sep + 1..] {
        if line.trim().is_empty() {
            break;
        }
        let row: Vec<char> = line.chars().collect();
        let u = Update { name: cell(&row, 0), id: cell(&row, 1), current: cell(&row, 2), available: cell(&row, 3) };
        if valid_id(&u.id) && !u.available.is_empty() {
            out.push(u);
        }
    }
    out
}

/// Un id de winget son letras, cifras, punto, guion y guion bajo (p. ej. `Mozilla.Firefox`).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('-')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

pub fn updates_list() -> Result<Vec<Update>, String> {
    imp::list()
}

pub fn update_apply(id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("identificador de programa no válido".into());
    }
    imp::apply(id)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn winget(args: &[&str]) -> Result<std::process::Output, String> {
        Command::new("winget")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|_| "winget no está disponible en este equipo (viene con «Instalador de aplicación» de la Microsoft Store)".to_string())
    }

    pub fn list() -> Result<Vec<Update>, String> {
        let out = winget(&["upgrade", "--accept-source-agreements", "--disable-interactivity"])?;
        Ok(parse_upgrades(&String::from_utf8_lossy(&out.stdout)))
    }

    pub fn apply(id: &str) -> Result<(), String> {
        let out = winget(&[
            "upgrade", "--id", id, "--exact", "--silent", "--disable-interactivity",
            "--accept-package-agreements", "--accept-source-agreements",
        ])?;
        if out.status.success() {
            Ok(())
        } else {
            let msg = String::from_utf8_lossy(&out.stdout);
            let last = msg.lines().rev().find(|l| l.trim().len() > 3).unwrap_or("error desconocido");
            Err(format!("winget no pudo actualizar {id}: {}", last.trim()))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn list() -> Result<Vec<Update>, String> {
        Err("Solo disponible en Windows".into())
    }

    pub fn apply(_: &str) -> Result<(), String> {
        Err("Solo disponible en Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\r   - \r\
Name                      Id                       Version      Available    Source\r\n\
-------------------------------------------------------------------------------------\r\n\
Mozilla Firefox (x64 es)  Mozilla.Firefox          130.0        131.0        winget\r\n\
7-Zip 23.01 (x64)         7zip.7zip                23.01        24.08        winget\r\n\
Weird; name               Bad Id With Spaces       1            2            winget\r\n\
\r\n\
2 upgrades available.\r\n";

    #[test]
    fn parses_table_by_column_position() {
        let v = parse_upgrades(SAMPLE);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], Update { name: "Mozilla Firefox (x64 es)".into(), id: "Mozilla.Firefox".into(), current: "130.0".into(), available: "131.0".into() });
        assert_eq!(v[1].id, "7zip.7zip");
    }

    #[test]
    fn localized_headers_work_and_empty_is_empty() {
        let es = "Nombre   Id        Versión  Disponible  Origen\n---------------------------------------\nFoo      Foo.Bar   1.0      2.0         winget\n";
        assert_eq!(parse_upgrades(es)[0].id, "Foo.Bar");
        assert!(parse_upgrades("No installed package found matching input criteria.").is_empty());
        assert!(parse_upgrades("").is_empty());
    }

    #[test]
    fn ids_are_validated() {
        assert!(valid_id("Mozilla.Firefox"));
        assert!(!valid_id("a b"));
        assert!(!valid_id("--source"));
        assert!(!valid_id("x&calc"));
        assert!(update_apply("x;y").is_err());
    }
}
