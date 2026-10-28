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

    pub fn list() -> Vec<Program> {
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
                out.push(Program {
                    id: make_id(tag, &key_name),
                    name,
                    publisher: s("Publisher"),
                    version,
                    location: s("InstallLocation"),
                    size_kb: k.get_value::<u32, _>("EstimatedSize").unwrap_or(0) as u64,
                    machine_wide: h == "HKLM",
                });
            }
        }
        out.sort_by_key(|p| p.name.to_lowercase());
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
    find_leftovers, list as list_programs, remove_leftovers, restore_registry, uninstall as uninstall_program,
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
}
