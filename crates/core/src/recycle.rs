//! Papelera de reciclaje: medir lo que ocupa y vaciarla con la función oficial de Windows.
//! Vaciarla no se puede deshacer (los archivos ya los había borrado la persona), así que la
//! interfaz lo pide aparte y con confirmación.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
pub struct RecycleInfo {
    pub items: u64,
    pub bytes: u64,
}

pub fn recycle_info() -> RecycleInfo {
    imp::info()
}

pub fn recycle_empty() -> Result<(), String> {
    imp::empty()
}

#[cfg(windows)]
mod imp {
    use super::RecycleInfo;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn powershell(script: &str) -> std::io::Result<std::process::Output> {
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    pub fn info() -> RecycleInfo {
        // Papelera = carpeta virtual 10 del Explorador; suma las papeleras de todas las unidades.
        let script = "$i=(New-Object -ComObject Shell.Application).Namespace(10).Items(); \
                      $b=0; foreach($x in $i){ $b += [int64]$x.Size }; \
                      Write-Output \"$($i.Count) $b\"";
        let Ok(out) = powershell(script) else { return RecycleInfo::default() };
        let text = String::from_utf8_lossy(&out.stdout);
        parse(text.trim())
    }

    pub fn parse(s: &str) -> RecycleInfo {
        let mut it = s.split_whitespace();
        let items = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let bytes = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        RecycleInfo { items, bytes }
    }

    pub fn empty() -> Result<(), String> {
        let out = powershell("Clear-RecycleBin -Force -ErrorAction Stop").map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            // Una papelera ya vacía también devuelve error: no es un fallo.
            let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if msg.is_empty() || info().items == 0 {
                Ok(())
            } else {
                Err(msg)
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::RecycleInfo;

    pub fn info() -> RecycleInfo {
        RecycleInfo::default()
    }

    pub fn empty() -> Result<(), String> {
        Err("Solo disponible en Windows".into())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn parses_powershell_output() {
        let r = super::imp::parse("3 1048576");
        assert_eq!((r.items, r.bytes), (3, 1_048_576));
        assert_eq!(super::imp::parse("").items, 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn empty_fails_outside_windows() {
        assert!(super::recycle_empty().is_err());
        assert_eq!(super::recycle_info().items, 0);
    }
}
