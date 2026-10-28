//! Desinstalador: lista los programas instalados, lanza su desinstalador oficial y busca
//! restos (carpetas y claves de registro) para que la persona los revise.
//!
//! Seguridad:
//! - El comando de desinstalación se lee del registro en el momento; nunca llega de la interfaz.
//! - Los restos se buscan solo en ubicaciones fijas (`Archivos de programa`, `AppData`, claves
//!   `Software\<empresa>\<producto>`) y se vuelven a validar antes de tocarlos, porque las rutas
//!   vuelven desde la interfaz.
//! - Las carpetas van a la cuarentena (reversible). Las claves de registro se exportan a un
//!   `.reg` antes de borrarse y, si la copia falla, no se borra nada.

#![cfg_attr(not(windows), allow(dead_code))]

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Program {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub location: String,
    pub size_kb: u64,
    /// `AAAA-MM-DD` o vacío si el programa no lo declara.
    pub install_date: String,
    /// Última vez que se abrió (aproximada, de los registros de uso de Windows) o vacío.
    pub last_used: String,
    /// Origen del icono (`DisplayIcon` del registro). No se envía a la interfaz.
    #[serde(skip)]
    pub icon_src: String,
    /// Instalado para todos los usuarios (HKLM): desinstalar pide permisos de administrador.
    pub machine_wide: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LeftoverKind {
    Folder,
    Registry,
}

/// Un resto encontrado tras desinstalar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Leftover {
    pub kind: LeftoverKind,
    /// Ruta de carpeta, o clave de registro con prefijo `HKCU\` o `HKLM\`.
    pub path: String,
    /// Marcado por defecto: solo carpetas del propio programa.
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallOutcome {
    pub exit_code: Option<i32>,
    /// La entrada de Programas y características ya no existe.
    pub removed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeftoverOutcome {
    pub batch: String,
    pub folders: u64,
    pub keys: u64,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------- ids

const UNINSTALL_BASES: [(&str, &str, &str); 3] = [
    ("hkcu", "HKCU", r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
    ("hklm", "HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ("hklm32", "HKLM", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
];

pub fn make_id(base_tag: &str, key_name: &str) -> String {
    format!("{base_tag}|{key_name}")
}

/// `hklm|{GUID}` → (HKLM, ruta de la clave). Solo cuelga de las tres claves `Uninstall` conocidas.
pub fn parse_id(id: &str) -> Option<(&'static str, String)> {
    let (tag, name) = id.split_once('|')?;
    if name.is_empty() || name.contains('\\') || name.contains('/') {
        return None;
    }
    let (_, hive, base) = UNINSTALL_BASES.iter().find(|(t, _, _)| *t == tag)?;
    Some((hive, format!(r"{base}\{name}")))
}

// ---------------------------------------------------------------- nombres

/// Nombres que nunca se consideran restos de un programa: son del sistema o estructurales.
const PROTECTED: &[&str] = &[
    "microsoft", "microsoft corporation", "windows", "windows nt", "windowsapps", "common files",
    "microsoft shared", "internet explorer", "windows defender", "windows mail", "windows media player",
    "windows photo viewer", "windows security", "windowspowershell", "modifiablewindowsapps", "system32",
    "classes", "policies", "wow6432node", "clients", "registeredapplications", "program files",
    "programdata", "users", "appdata", "local", "locallow", "roaming", "temp", "packages", "microsoft edge",
    "edge", "webview2", "windows kits", "dotnet", ".net", "reference assemblies", "msbuild",
];

fn is_protected(segment: &str) -> bool {
    let s = segment.trim().to_lowercase();
    PROTECTED.contains(&s.as_str())
}

/// Un segmento de ruta válido: sin separadores, sin `..`, sin caracteres de control ni de comodín.
fn clean_segment(s: &str) -> Option<String> {
    let t = s.trim().trim_end_matches('.').trim();
    if t.len() < 3
        || t.len() > 60
        || t.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|', '%'])
        || t.chars().any(char::is_control)
        || t.split('.').all(str::is_empty)
        || is_protected(t)
    {
        return None;
    }
    Some(t.to_string())
}

/// Variantes del nombre con el que un programa suele crear sus carpetas:
/// el nombre completo y el nombre sin versión ni arquitectura (`7-Zip 23.01 (x64)` → `7-Zip`).
pub fn name_variants(display_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(n) = clean_segment(display_name) {
        out.push(n);
    }
    let mut words: Vec<&str> = display_name.split_whitespace().collect();
    // Quita lo que va entre paréntesis y los tokens de versión del final.
    words.retain(|w| !(w.starts_with('(') && w.ends_with(')')));
    while let Some(last) = words.last() {
        let is_version = last.chars().next().is_some_and(|c| c.is_ascii_digit() || c == 'v')
            && last.chars().any(|c| c.is_ascii_digit())
            && last.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        if is_version && words.len() > 1 {
            words.pop();
        } else {
            break;
        }
    }
    if let Some(n) = clean_segment(&words.join(" ")) {
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// `Igor Pavlov`, `Mozilla Corporation` → `Mozilla`: sin sufijos de sociedad.
pub fn publisher_variants(publisher: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(p) = clean_segment(publisher) {
        out.push(p);
    }
    let mut words: Vec<&str> = publisher.split_whitespace().collect();
    let suffixes = ["inc", "inc.", "llc", "ltd", "ltd.", "corporation", "corp", "corp.", "gmbh", "s.l.", "sa", "co."];
    while let Some(last) = words.last() {
        let l = last.trim_end_matches(',').to_lowercase();
        if words.len() > 1 && suffixes.contains(&l.as_str()) {
            words.pop();
        } else {
            break;
        }
    }
    if let Some(p) = clean_segment(words.join(" ").trim_end_matches(',')) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

// ---------------------------------------------------------------- fechas y uso

/// `20261027` → `2026-10-27`. Cualquier otra cosa → vacío.
pub fn parse_install_date(raw: &str) -> String {
    let d = raw.trim();
    let ok = d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) && &d[0..4] >= "1990" && &d[4..6] <= "12" && &d[6..8] <= "31";
    if ok {
        format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
    } else {
        String::new()
    }
}

/// `"C:\\Foo\\app.exe",0` → `C:\\Foo\\app.exe`. Quita comillas y el índice del icono.
pub fn icon_path(display_icon: &str) -> String {
    let t = display_icon.trim();
    let t = if let Some(rest) = t.strip_prefix('"') {
        rest.split('"').next().unwrap_or("")
    } else {
        t.rsplit_once(',').map_or(t, |(p, idx)| if idx.trim().trim_start_matches('-').bytes().all(|b| b.is_ascii_digit()) { p } else { t })
    };
    t.trim().to_string()
}

/// FILETIME de Windows (100 ns desde 1601) → `AAAA-MM-DD`. 0 → vacío.
pub fn filetime_to_date(ft: u64) -> String {
    if ft == 0 {
        return String::new();
    }
    let unix_secs = (ft / 10_000_000) as i64 - 11_644_473_600;
    let z = unix_secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a') + 13) % 26 + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A') + 13) % 26 + b'A') as char,
            _ => c,
        })
        .collect()
}

/// Una entrada de «UserAssist»: Windows anota ahí (con el nombre en ROT13) cada programa que
/// abres desde el Explorador. Devuelve (ruta en minúsculas, FILETIME de la última ejecución).
pub fn userassist_entry(name_rot13: &str, data: &[u8]) -> Option<(String, u64)> {
    if data.len() < 68 {
        return None;
    }
    let ft = u64::from_le_bytes(data[60..68].try_into().ok()?);
    (ft > 0).then(|| (rot13(name_rot13).to_lowercase().replace('/', "\\"), ft))
}

/// Última ejecución conocida de un programa: busca entradas cuya ruta pase por la carpeta del
/// programa (por su último componente, de al menos 4 letras) y devuelve la fecha más reciente.
pub fn last_used(entries: &[(String, u64)], location: &str, icon_src: &str) -> String {
    // Se parte a mano por `\` y `/`: así funciona igual con rutas de Windows en cualquier sistema.
    let loc = location.trim().trim_matches('"');
    let dir = if loc.is_empty() {
        let icon = icon_path(icon_src);
        icon.rsplit_once(['\\', '/']).map(|(d, _)| d.to_string()).unwrap_or_default()
    } else {
        loc.to_string()
    };
    let key = dir.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
    if key.len() < 4 || is_protected(&key) {
        return String::new();
    }
    let needle = format!("\\{key}\\");
    let best = entries.iter().filter(|(p, _)| p.contains(&needle)).map(|(_, t)| *t).max().unwrap_or(0);
    filetime_to_date(best)
}

// ---------------------------------------------------------------- carpetas

/// Raíces donde puede haber carpetas de un programa. Nada fuera de ellas se propone ni se toca.
pub fn folder_bases() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)", "ProgramData", "APPDATA", "LOCALAPPDATA"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect()
}

/// ¿Está la carpeta estrictamente dentro de una raíz permitida, a 1-3 niveles, y sin
/// componentes protegidos ni `..`?
pub fn folder_allowed(path: &Path, bases: &[PathBuf]) -> bool {
    bases.iter().any(|base| {
        let Ok(rel) = path.strip_prefix(base) else { return false };
        let parts: Vec<_> = rel.components().collect();
        (1..=3).contains(&parts.len())
            && parts.iter().all(|c| match c {
                Component::Normal(s) => s.to_str().is_some_and(|s| !is_protected(s)),
                _ => false,
            })
    })
}

/// Carpetas candidatas: `<raíz>\<producto>` y `<raíz>\<empresa>\<producto>` (nunca la empresa sola,
/// porque puede contener otros productos) más la carpeta de instalación que declara el programa.
pub fn folder_candidates(name: &str, publisher: &str, location: &str, bases: &[PathBuf]) -> Vec<PathBuf> {
    let names = name_variants(name);
    let pubs = publisher_variants(publisher);
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if folder_allowed(&p, bases) && !out.contains(&p) {
            out.push(p);
        }
    };
    if !location.trim().is_empty() {
        push(PathBuf::from(location.trim().trim_matches('"')));
    }
    for base in bases {
        for n in &names {
            push(base.join(n));
            for p in &pubs {
                push(base.join(p).join(n));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- registro

/// Divide `HKCU\Software\X` en (`HKCU`, `Software\X`).
pub fn split_key(key: &str) -> Option<(&'static str, &str)> {
    let (hive, rest) = key.split_once('\\')?;
    let hive = match hive.to_ascii_uppercase().as_str() {
        "HKCU" => "HKCU",
        "HKLM" => "HKLM",
        _ => return None,
    };
    Some((hive, rest))
}

/// Solo claves `Software\<producto>` o `Software\<empresa>\<producto>` (con o sin `WOW6432Node`),
/// sin segmentos protegidos.
pub fn key_allowed(key: &str) -> bool {
    let Some((_, rest)) = split_key(key) else { return false };
    let mut parts: Vec<&str> = rest.split('\\').collect();
    if parts.is_empty() || !parts[0].eq_ignore_ascii_case("software") {
        return false;
    }
    parts.remove(0);
    if parts.first().is_some_and(|p| p.eq_ignore_ascii_case("wow6432node")) {
        parts.remove(0);
    }
    (1..=2).contains(&parts.len())
        && parts.iter().all(|p| clean_segment(p).is_some_and(|c| c == p.trim().trim_end_matches('.').trim()))
}

pub fn key_candidates(name: &str, publisher: &str) -> Vec<String> {
    let names = name_variants(name);
    let pubs = publisher_variants(publisher);
    let mut out: Vec<String> = Vec::new();
    for (hive, prefixes) in [("HKCU", vec!["Software"]), ("HKLM", vec!["SOFTWARE", r"SOFTWARE\WOW6432Node"])] {
        for prefix in prefixes {
            for n in &names {
                out.push(format!(r"{hive}\{prefix}\{n}"));
                for p in &pubs {
                    out.push(format!(r"{hive}\{prefix}\{p}\{n}"));
                }
            }
        }
    }
    out.retain(|k| key_allowed(k));
    out.dedup();
    out
}

// ---------------------------------------------------------------- Windows

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::clean::{clean_rule, Mode};
    use crate::rules::Rule;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use winreg::enums::*;
    use winreg::RegKey;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn hive(h: &str) -> RegKey {
        RegKey::predef(if h == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
    }

    /// Lee «UserAssist» del usuario actual: (ruta en minúsculas, FILETIME de última ejecución).
    fn userassist() -> Vec<(String, u64)> {
        let mut out = Vec::new();
        let path = r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist";
        let Ok(root) = hive("HKCU").open_subkey_with_flags(path, KEY_READ) else { return out };
        for guid in root.enum_keys().flatten() {
            let Ok(count) = root.open_subkey_with_flags(format!(r"{guid}\Count"), KEY_READ) else { continue };
            for (name, value) in count.enum_values().flatten() {
                if let Some(e) = userassist_entry(&name, &value.bytes) {
                    out.push(e);
                }
            }
        }
        out
    }

    pub fn list() -> Vec<Program> {
        let usage = userassist();
        let mut out: Vec<Program> = Vec::new();
        for (tag, h, base) in UNINSTALL_BASES {
            let Ok(root) = hive(h).open_subkey_with_flags(base, KEY_READ) else { continue };
            for key_name in root.enum_keys().flatten() {
                let Ok(k) = root.open_subkey_with_flags(&key_name, KEY_READ) else { continue };
                let s = |n: &str| k.get_value::<String, _>(n).unwrap_or_default().trim().to_string();
                let name = s("DisplayName");
                if name.is_empty() || s("UninstallString").is_empty() {
                    continue;
                }
                // Componentes del sistema, parches y actualizaciones no se listan.
                if k.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1
                    || !s("ParentKeyName").is_empty()
                    || matches!(s("ReleaseType").as_str(), "Update" | "Hotfix" | "Security Update" | "Update Rollup")
                {
                    continue;
                }
                let version = s("DisplayVersion");
                if out.iter().any(|p| p.name == name && p.version == version) {
                    continue;
                }
                let icon_src = s("DisplayIcon");
                let location = s("InstallLocation");
                out.push(Program {
                    install_date: parse_install_date(&s("InstallDate")),
                    last_used: last_used(&usage, &location, &icon_src),
                    icon_src,
                    id: make_id(tag, &key_name),
                    name,
                    publisher: s("Publisher"),
                    version,
                    location,
                    size_kb: k.get_value::<u32, _>("EstimatedSize").unwrap_or(0) as u64,
                    machine_wide: h == "HKLM",
                });
            }
        }
        out.sort_by_key(|p| p.name.to_lowercase());
        out
    }

    /// Iconos de los programas como PNG en base64. La ruta del icono sale del registro (no de la
    /// interfaz) y solo se leen archivos `.exe`, `.ico` o `.dll` que existen.
    pub fn icons(ids: &[String]) -> std::collections::HashMap<String, String> {
        let mut paths = serde_json::Map::new();
        for id in ids {
            let Some((h, path)) = parse_id(id) else { continue };
            let Ok(key) = hive(h).open_subkey_with_flags(&path, KEY_READ) else { continue };
            let src = icon_path(&key.get_value::<String, _>("DisplayIcon").unwrap_or_default());
            let p = Path::new(&src);
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if p.is_file() && matches!(ext.as_str(), "exe" | "ico" | "dll") {
                paths.insert(id.clone(), serde_json::Value::String(src));
            }
        }
        let mut out = std::collections::HashMap::new();
        let all: Vec<_> = paths.into_iter().collect();
        for chunk in all.chunks(60) {
            let json = serde_json::Value::Object(chunk.iter().cloned().collect()).to_string();
            let script = "Add-Type -AssemblyName System.Drawing; $m = $env:WC_ICONS | ConvertFrom-Json; $o = @{}; \
                foreach ($p in $m.PSObject.Properties) { try { $i = [System.Drawing.Icon]::ExtractAssociatedIcon($p.Value); \
                if ($i) { $s = New-Object IO.MemoryStream; $i.ToBitmap().Save($s, [System.Drawing.Imaging.ImageFormat]::Png); \
                $o[$p.Name] = [Convert]::ToBase64String($s.ToArray()) } } catch {} }; $o | ConvertTo-Json -Compress";
            let Ok(res) = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
                .env("WC_ICONS", json)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            else {
                continue;
            };
            if let Ok(serde_json::Value::Object(m)) = serde_json::from_slice(&res.stdout) {
                for (k, v) in m {
                    if let Some(b64) = v.as_str() {
                        out.insert(k, b64.to_string());
                    }
                }
            }
        }
        out
    }

    fn key_exists(id: &str) -> bool {
        parse_id(id)
            .map(|(h, path)| hive(h).open_subkey_with_flags(path, KEY_READ).is_ok())
            .unwrap_or(false)
    }

    /// Lanza el desinstalador oficial que el programa registró y espera a que termine.
    /// Para programas de todos los usuarios se pide elevación (aviso UAC de Windows).
    pub fn uninstall(id: &str) -> UninstallOutcome {
        let fail = |e: String| UninstallOutcome { exit_code: None, removed: false, error: Some(e) };
        let Some((h, path)) = parse_id(id) else { return fail("programa no válido".into()) };
        let Ok(key) = hive(h).open_subkey_with_flags(&path, KEY_READ) else {
            return fail("el programa ya no está instalado".into());
        };
        let cmd: String = key.get_value("UninstallString").unwrap_or_default();
        if cmd.trim().is_empty() {
            return fail("el programa no tiene desinstalador".into());
        }
        // La línea se pasa por variable de entorno: no hay que escapar nada en el script.
        let verb = if h == "HKLM" { " -Verb RunAs" } else { "" };
        let script = format!(
            "$p = Start-Process -FilePath cmd.exe -ArgumentList ('/C \"' + $env:WC_CMD + '\"') -Wait -PassThru{verb}; exit $p.ExitCode"
        );
        let out = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .env("WC_CMD", cmd.trim())
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match out {
            Ok(o) => {
                let removed = !key_exists(id);
                let error = if o.status.success() || removed {
                    None
                } else if String::from_utf8_lossy(&o.stderr).to_lowercase().contains("cancel") {
                    Some("Se canceló el aviso de permisos de Windows.".into())
                } else {
                    Some("El desinstalador terminó con error o fue cancelado.".into())
                };
                UninstallOutcome { exit_code: o.status.code(), removed, error }
            }
            Err(e) => fail(e.to_string()),
        }
    }

    pub fn find_leftovers(name: &str, publisher: &str, location: &str) -> Vec<Leftover> {
        let bases = folder_bases();
        let mut out = Vec::new();
        for p in folder_candidates(name, publisher, location, &bases) {
            if std::fs::symlink_metadata(&p).map(|m| m.is_dir()).unwrap_or(false) {
                out.push(Leftover { kind: LeftoverKind::Folder, path: p.display().to_string(), selected: true });
            }
        }
        for k in key_candidates(name, publisher) {
            if let Some((h, rest)) = split_key(&k) {
                if hive(h).open_subkey_with_flags(rest, KEY_READ).is_ok() {
                    out.push(Leftover { kind: LeftoverKind::Registry, path: k, selected: false });
                }
            }
        }
        out
    }

    fn backup_dir(base: &Path, batch: &str) -> PathBuf {
        base.join("RegBackup").join(batch)
    }

    /// Elimina los restos elegidos. `data_dir` es `%LOCALAPPDATA%\WinCleaner`.
    /// Carpetas → cuarentena. Claves → copia `.reg` y después borrado.
    pub fn remove_leftovers(items: &[Leftover], data_dir: &Path, batch: &str) -> LeftoverOutcome {
        let mut res = LeftoverOutcome { batch: batch.to_string(), folders: 0, keys: 0, errors: Vec::new() };
        let bases = folder_bases();
        let mode = Mode::Quarantine { dir: data_dir.join("Quarantine"), batch: batch.to_string() };
        for (n, item) in items.iter().enumerate() {
            match item.kind {
                LeftoverKind::Folder => {
                    let p = PathBuf::from(&item.path);
                    if !folder_allowed(&p, &bases) {
                        res.errors.push(format!("ruta rechazada por segura: {}", item.path));
                        continue;
                    }
                    let rule = Rule {
                        id: format!("uninstall-{n}"),
                        name: String::new(),
                        category: String::new(),
                        description: String::new(),
                        help: String::new(),
                        paths: vec![item.path.clone()],
                        safe: false,
                    };
                    let rep = clean_rule(&rule, &mode);
                    res.errors.extend(rep.errors);
                    let _ = std::fs::remove_dir(&p); // la raíz solo se quita si quedó vacía
                    res.folders += 1;
                }
                LeftoverKind::Registry => {
                    if !key_allowed(&item.path) {
                        res.errors.push(format!("clave rechazada por segura: {}", item.path));
                        continue;
                    }
                    let Some((h, rest)) = split_key(&item.path) else { continue };
                    let dir = backup_dir(data_dir, batch);
                    if let Err(e) = std::fs::create_dir_all(&dir) {
                        res.errors.push(format!("no se pudo crear la copia: {e}"));
                        continue;
                    }
                    let file = dir.join(format!("{n}.reg"));
                    let exported = Command::new("reg")
                        .args(["export", &item.path, &file.display().to_string(), "/y"])
                        .creation_flags(CREATE_NO_WINDOW)
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if !exported {
                        res.errors.push(format!("sin copia de seguridad, no se borra: {}", item.path));
                        continue;
                    }
                    match hive(h).delete_subkey_all(rest) {
                        Ok(()) => res.keys += 1,
                        Err(e) => res.errors.push(format!("{}: {e}", item.path)),
                    }
                }
            }
        }
        res
    }

    /// Vuelve a importar las copias `.reg` de un lote.
    pub fn restore_registry(data_dir: &Path, batch: &str) -> Result<u64, String> {
        if batch.is_empty() || !batch.bytes().all(|b| b.is_ascii_digit()) {
            return Err("lote no válido".into());
        }
        let mut n = 0;
        for f in std::fs::read_dir(backup_dir(data_dir, batch)).map_err(|e| e.to_string())?.flatten() {
            if f.path().extension().is_some_and(|e| e == "reg") {
                let ok = Command::new("reg")
                    .args(["import", &f.path().display().to_string()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if ok {
                    n += 1;
                }
            }
        }
        Ok(n)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    const NO: &str = "solo disponible en Windows";

    pub fn list() -> Vec<Program> {
        Vec::new()
    }
    pub fn icons(_ids: &[String]) -> std::collections::HashMap<String, String> {
        Default::default()
    }
    pub fn uninstall(_id: &str) -> UninstallOutcome {
        UninstallOutcome { exit_code: None, removed: false, error: Some(NO.into()) }
    }
    pub fn find_leftovers(_n: &str, _p: &str, _l: &str) -> Vec<Leftover> {
        Vec::new()
    }
    pub fn remove_leftovers(_i: &[Leftover], _d: &Path, batch: &str) -> LeftoverOutcome {
        LeftoverOutcome { batch: batch.into(), folders: 0, keys: 0, errors: vec![NO.into()] }
    }
    pub fn restore_registry(_d: &Path, _b: &str) -> Result<u64, String> {
        Err(NO.into())
    }
}

pub use imp::{
    find_leftovers, icons as program_icons, list as list_programs, remove_leftovers, restore_registry, uninstall as uninstall_program,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn bases() -> Vec<PathBuf> {
        vec![PathBuf::from("/pf"), PathBuf::from("/appdata")]
    }

    #[test]
    fn ids_only_point_at_uninstall_keys() {
        assert_eq!(parse_id("hklm|{ABC}").unwrap().0, "HKLM");
        assert!(parse_id("hklm|{ABC}").unwrap().1.ends_with(r"Uninstall\{ABC}"));
        assert!(parse_id(r"hklm|..\..\Run").is_none());
        assert!(parse_id("hklm|a/b").is_none());
        assert!(parse_id("otro|x").is_none());
        assert!(parse_id("hkcu|").is_none());
    }

    #[test]
    fn names_lose_version_and_arch() {
        assert_eq!(name_variants("7-Zip 23.01 (x64)"), vec!["7-Zip 23.01 (x64)", "7-Zip"]);
        assert_eq!(name_variants("Mozilla Firefox"), vec!["Mozilla Firefox"]);
        assert!(name_variants("ab").is_empty());
        assert!(name_variants("Microsoft").is_empty());
        assert!(name_variants(r"..\..\Windows").is_empty());
    }

    #[test]
    fn publisher_suffixes_are_removed() {
        assert_eq!(publisher_variants("Mozilla Corporation"), vec!["Mozilla Corporation", "Mozilla"]);
        assert!(publisher_variants("Microsoft Corporation").is_empty());
    }

    #[test]
    fn folders_must_be_inside_bases_and_not_protected() {
        let b = bases();
        assert!(folder_allowed(Path::new("/pf/Foo"), &b));
        assert!(folder_allowed(Path::new("/pf/Acme/Foo"), &b));
        assert!(!folder_allowed(Path::new("/pf"), &b), "la raíz nunca");
        assert!(!folder_allowed(Path::new("/other/Foo"), &b));
        assert!(!folder_allowed(Path::new("/pf/Common Files/X"), &b));
        assert!(!folder_allowed(Path::new("/pf/Foo/../../etc"), &b));
        assert!(!folder_allowed(Path::new("/pf/a/b/c/d"), &b), "demasiado profundo");
    }

    #[test]
    fn candidates_are_derived_and_filtered() {
        let b = bases();
        let c = folder_candidates("Foo Editor 2.1", "Acme Inc.", "/pf/Acme/Foo", &b);
        assert!(c.contains(&PathBuf::from("/pf/Acme/Foo")));
        assert!(c.contains(&PathBuf::from("/pf/Foo Editor")));
        assert!(c.contains(&PathBuf::from("/appdata/Acme/Foo Editor")));
        assert!(!c.contains(&PathBuf::from("/pf/Acme")), "la carpeta de la empresa sola no es un resto");
        // Una ubicación declarada fuera de las raíces se ignora.
        assert!(folder_candidates("Foo Editor", "", r"/etc", &b).iter().all(|p| p != Path::new("/etc")));
    }

    #[test]
    fn registry_keys_are_restricted() {
        assert!(key_allowed(r"HKCU\Software\Foo"));
        assert!(key_allowed(r"HKLM\SOFTWARE\Acme\Foo"));
        assert!(key_allowed(r"HKLM\SOFTWARE\WOW6432Node\Acme\Foo"));
        assert!(!key_allowed(r"HKLM\SOFTWARE"));
        assert!(!key_allowed(r"HKLM\SOFTWARE\Microsoft\Windows"));
        assert!(!key_allowed(r"HKLM\SYSTEM\CurrentControlSet\Services\Foo"));
        assert!(!key_allowed(r"HKCR\Foo"));
        assert!(!key_allowed(r"HKCU\Software\A\B\C"));
        assert!(!key_allowed(r"HKCU\Software\Foo\..\Bar"));
        for k in key_candidates("Foo Editor 2.1", "Acme Inc.") {
            assert!(key_allowed(&k), "{k}");
        }
    }

    #[test]
    fn dates_and_icons_are_parsed() {
        assert_eq!(parse_install_date("20261027"), "2026-10-27");
        assert_eq!(parse_install_date(""), "");
        assert_eq!(parse_install_date("27/10/2026"), "");
        assert_eq!(icon_path(r#""C:\Foo\app.exe",0"#), r"C:\Foo\app.exe");
        assert_eq!(icon_path(r"C:\Foo\app.exe,-12"), r"C:\Foo\app.exe");
        assert_eq!(icon_path(r"C:\Foo\app.ico"), r"C:\Foo\app.ico");
        assert_eq!(filetime_to_date(116_444_736_000_000_000), "1970-01-01");
        assert_eq!(filetime_to_date((1_700_000_000 + 11_644_473_600) * 10_000_000), "2023-11-14");
        assert_eq!(filetime_to_date(0), "");
    }

    #[test]
    fn userassist_gives_last_use_by_folder() {
        assert_eq!(rot13("Pbaqhgf"), "Conduts");
        let mut data = vec![0u8; 72];
        let ft: u64 = (1_700_000_000 + 11_644_473_600) * 10_000_000;
        data[60..68].copy_from_slice(&ft.to_le_bytes());
        let e = userassist_entry(&rot13(r"{ABC}\7-Zip\7zFM.exe"), &data).unwrap();
        assert!(e.0.contains("\\7-zip\\"));
        assert_eq!(userassist_entry("x", &[0u8; 10]), None);
        let entries = vec![e];
        assert_eq!(last_used(&entries, r"C:\Program Files\7-Zip", ""), "2023-11-14");
        assert_eq!(last_used(&entries, r"C:\Program Files\Otro", ""), "");
        assert_eq!(last_used(&entries, "", r#""C:\Program Files\7-Zip\7zFM.exe",0"#), "2023-11-14");
    }
}
