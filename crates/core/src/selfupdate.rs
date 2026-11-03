//! Actualizaciones de WinCleaner desde GitHub Releases, sin instalador.
//!
//! WinCleaner es un único exe portable, así que se actualiza a sí mismo:
//! 1. Pregunta a la API de GitHub cuál es la última versión publicada.
//! 2. Si es más nueva, descarga `WinCleaner-vX.Y.Z-portable.exe` y su firma `.sig`.
//! 3. Comprueba la firma con la clave pública que va dentro del programa (Ed25519, formato
//!    minisign, el mismo que usa Tauri). La firma también nombra el archivo, así que no se puede
//!    colar un exe antiguo con la etiqueta de uno nuevo.
//! 4. Cambia el exe de sitio: el actual pasa a `.old`, el nuevo ocupa su lugar y, si algo falla,
//!    se deja todo como estaba. Al abrir la versión nueva se borra el `.old`.
//!
//! Si la versión publicada no trae firma, o este exe no lleva clave pública, nunca se instala
//! nada: solo se avisa y se ofrece abrir la página de descarga.

use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Repositorio del que salen las versiones.
pub const REPO: &str = "aguado2323-rgb/WinCleaner";
/// Versión de este exe. El workflow de publicación la fija a partir de la etiqueta.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Clave pública de firma (la que imprime `tauri signer generate`). Vacía = solo avisar.
const PUBKEY: &str = include_str!("../updater.pub");

const MAX_EXE: u64 = 64 * 1024 * 1024;
const MAX_SIG: u64 = 16 * 1024;
const MAX_API: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppUpdate {
    pub current: String,
    /// Última versión publicada, sin la «v».
    pub latest: String,
    /// Hay una versión más nueva que esta.
    pub newer: bool,
    /// Se puede instalar sola (la versión trae firma y este exe sabe comprobarla).
    pub installable: bool,
    pub notes: String,
    pub page: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Lo que hace falta para instalar, sacado de la respuesta de GitHub.
#[derive(Debug, PartialEq, Eq)]
struct Plan {
    info: AppUpdate,
    exe: Option<(String, String)>, // (nombre, url)
    sig: Option<String>,
}

/// `v1.2.3` o `1.2.3` → (1, 2, 3). Cualquier otra cosa no se acepta.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut it = s.trim().trim_start_matches('v').split('.');
    let v = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    if it.next().is_some() {
        return None;
    }
    Some(v)
}

/// Nombre del exe que adjunta el workflow de publicación para una etiqueta.
pub fn asset_name(tag: &str) -> String {
    format!("WinCleaner-{tag}-portable.exe")
}

fn download_prefix() -> String {
    format!("https://github.com/{}/releases/download/", REPO.to_ascii_lowercase())
}

fn has_pubkey() -> bool {
    !PUBKEY.trim().is_empty()
}

fn plan(json: &str, current: &str, can_verify: bool) -> Result<Plan, String> {
    let r: Release = serde_json::from_str(json).map_err(|e| format!("respuesta de GitHub no válida: {e}"))?;
    if r.draft || r.prerelease {
        return Err("la última versión de GitHub no es definitiva".into());
    }
    let latest = parse_version(&r.tag_name).ok_or_else(|| format!("etiqueta de versión no válida: {}", r.tag_name))?;
    let mine = parse_version(current).ok_or("versión propia no válida")?;
    let name = asset_name(&r.tag_name);
    let prefix = download_prefix();
    let ours = |a: &&Asset| a.browser_download_url.to_ascii_lowercase().starts_with(&prefix);
    let exe = r.assets.iter().filter(ours).find(|a| a.name == name && a.size > 0 && a.size <= MAX_EXE);
    let sig = r.assets.iter().filter(ours).find(|a| a.name == format!("{name}.sig"));
    let newer = latest > mine;
    Ok(Plan {
        info: AppUpdate {
            current: current.to_string(),
            latest: format!("{}.{}.{}", latest.0, latest.1, latest.2),
            newer,
            installable: newer && can_verify && exe.is_some() && sig.is_some(),
            notes: r.body.unwrap_or_default().chars().take(4000).collect(),
            page: r.html_url,
        },
        exe: exe.map(|a| (a.name.clone(), a.browser_download_url.clone())),
        sig: sig.map(|a| a.browser_download_url.clone()),
    })
}

fn b64(s: &str) -> Result<String, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|_| "firma o clave con formato no válido".to_string())?;
    String::from_utf8(raw).map_err(|_| "firma o clave con formato no válido".to_string())
}

/// Comprueba que `data` está firmado con la clave `pubkey` y que la firma es de `file`.
/// `sig` y `pubkey` van en base64, tal cual los genera `tauri signer`.
pub fn verify(data: &[u8], sig: &str, pubkey: &str, file: &str) -> Result<(), String> {
    let pk = PublicKey::decode(&b64(pubkey)?).map_err(|_| "clave pública no válida".to_string())?;
    let sig = Signature::decode(&b64(sig)?).map_err(|_| "firma no válida".to_string())?;
    pk.verify(data, &sig, true).map_err(|_| "la firma no coincide: la descarga no es de WinCleaner o está dañada".to_string())?;
    // El comentario de confianza va dentro de lo firmado: «timestamp:…\tfile:Nombre.exe».
    let named = sig.trusted_comment().split('\t').any(|part| part.strip_prefix("file:") == Some(file));
    if !named {
        return Err("la firma es de otro archivo".into());
    }
    Ok(())
}

/// Pone `new` en lugar de `exe`. El exe en uso se puede renombrar en Windows aunque esté abierto.
/// Si algo falla, el exe original vuelve a su sitio.
pub fn swap_exe(exe: &Path, new: &[u8]) -> Result<(), String> {
    let tmp = sibling(exe, "new");
    let old = old_path(exe);
    fs::write(&tmp, new).map_err(|e| format!("no se pudo escribir en la carpeta del programa ({e}). Descarga la versión nueva a mano."))?;
    let _ = fs::remove_file(&old);
    if let Err(e) = fs::rename(exe, &old) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("no se pudo apartar el exe actual: {e}"));
    }
    if let Err(e) = fs::rename(&tmp, exe) {
        let _ = fs::rename(&old, exe);
        let _ = fs::remove_file(&tmp);
        return Err(format!("no se pudo colocar la versión nueva: {e}"));
    }
    Ok(())
}

fn sibling(exe: &Path, ext: &str) -> PathBuf {
    let mut s = exe.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

/// Copia del exe anterior que queda tras actualizar.
pub fn old_path(exe: &Path) -> PathBuf {
    sibling(exe, "old")
}

/// Borra lo que haya dejado una actualización anterior. Se llama al abrir la app; el proceso
/// viejo puede tardar un momento en cerrarse, así que se reintenta unos segundos.
pub fn cleanup_after_update() {
    let Ok(exe) = std::env::current_exe() else { return };
    let old = old_path(&exe);
    let _ = fs::remove_file(sibling(&exe, "new"));
    for _ in 0..20 {
        if !old.exists() || fs::remove_file(&old).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn agent() -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
    Ok(ureq::AgentBuilder::new()
        .tls_connector(std::sync::Arc::new(tls))
        .https_only(true)
        .try_proxy_from_env(true)
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .user_agent(&format!("WinCleaner/{CURRENT}"))
        .build())
}

fn get(agent: &ureq::Agent, url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let resp = agent.get(url).set("Accept", "application/vnd.github+json").call().map_err(|e| match e {
        ureq::Error::Status(403 | 429, _) => "GitHub ha limitado las consultas; prueba dentro de un rato".to_string(),
        ureq::Error::Status(404, _) => "todavía no hay ninguna versión publicada".to_string(),
        ureq::Error::Status(code, _) => format!("GitHub respondió con el error {code}"),
        ureq::Error::Transport(_) => "sin conexión con GitHub".to_string(),
    })?;
    let mut out = Vec::new();
    resp.into_reader().take(limit + 1).read_to_end(&mut out).map_err(|e| format!("descarga interrumpida: {e}"))?;
    if out.len() as u64 > limit {
        return Err("la descarga es más grande de lo esperado".into());
    }
    Ok(out)
}

fn fetch_plan(agent: &ureq::Agent) -> Result<Plan, String> {
    let json = get(agent, &format!("https://api.github.com/repos/{REPO}/releases/latest"), MAX_API)?;
    plan(&String::from_utf8_lossy(&json), CURRENT, has_pubkey())
}

/// Consulta GitHub. Solo lectura.
pub fn app_update_check() -> Result<AppUpdate, String> {
    fetch_plan(&agent()?).map(|p| p.info)
}

/// Descarga, comprueba la firma y coloca la versión nueva en lugar de este exe.
/// Devuelve la ruta del exe para abrirlo; quien llama debe cerrar la app después.
pub fn app_update_install() -> Result<PathBuf, String> {
    let agent = agent()?;
    let p = fetch_plan(&agent)?;
    if !p.info.newer {
        return Err("ya tienes la última versión".into());
    }
    let (Some((name, exe_url)), Some(sig_url), true) = (p.exe, p.sig, p.info.installable) else {
        return Err("esta versión no trae firma de actualización; descárgala desde la página de GitHub".into());
    };
    let sig = get(&agent, &sig_url, MAX_SIG)?;
    let data = get(&agent, &exe_url, MAX_EXE)?;
    verify(&data, &String::from_utf8_lossy(&sig), PUBKEY, &name)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    swap_exe(&exe, &data)?;
    Ok(exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Clave de prueba generada solo para estos tests (no es la de publicación).
    const TEST_PUB: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEEzMTY0QTMyRjBFMkVGRUYKUldUdjcrTHdNa29XbytZalhyVEFldFZTZlh4dUx0Z1k1a0U1cHo4T0lnV01TcHBhMVJPK3RxQk0K";
    // Firma de "WinCleaner test payload\n" con file:WinCleaner-v9.9.9-portable.exe
    const TEST_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUdjcrTHdNa29XbzYzMThzVmZDZk5TbzRBOEZ4WnNaZUJveksvSVhNcmlWWGsxaG04Y1NuaVkzWm9IcjBFZm92czdVT21sbFVhVXBvVkd4dDhXMmJHakc1UGJVSzNIZVE0PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkzNzAwMzQ0CWZpbGU6V2luQ2xlYW5lci12OS45LjktcG9ydGFibGUuZXhlCmxEdHFRL2hPUGNRRUFGcGlveWFmcGZ2akwwYWt5K1pUR3BYQXYzWkpXb0FJZk1TNzZSa3hZcmRKcnIyeWdTNDNaTWxrOUwvYlh3K0RFTTJ2RGovekNRPT0K";
    const PAYLOAD: &[u8] = b"WinCleaner test payload\n";

    #[test]
    fn versions() {
        assert_eq!(parse_version("v0.2.10"), Some((0, 2, 10)));
        assert_eq!(parse_version("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_version("v1.0"), None);
        assert_eq!(parse_version("v1.0.0-beta"), None);
        assert_eq!(parse_version("v1.0.0.1"), None);
        assert!(parse_version("v0.2.10") > parse_version("v0.2.9"));
    }

    #[test]
    fn verifies_good_signature() {
        verify(PAYLOAD, TEST_SIG, TEST_PUB, "WinCleaner-v9.9.9-portable.exe").unwrap();
    }

    #[test]
    fn rejects_tampered_data() {
        assert!(verify(b"WinCleaner test payload!\n", TEST_SIG, TEST_PUB, "WinCleaner-v9.9.9-portable.exe").is_err());
    }

    #[test]
    fn rejects_signature_of_other_file() {
        // Un exe antiguo firmado no se puede hacer pasar por una versión más nueva.
        assert!(verify(PAYLOAD, TEST_SIG, TEST_PUB, "WinCleaner-v10.0.0-portable.exe").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(verify(PAYLOAD, "no", TEST_PUB, "x").is_err());
        assert!(verify(PAYLOAD, TEST_SIG, "", "x").is_err());
    }

    fn release(tag: &str, assets: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}","body":"Notas","html_url":"https://github.com/{REPO}/releases/tag/{tag}","draft":false,"prerelease":false,"assets":[{assets}]}}"#
        )
    }

    fn asset(name: &str) -> String {
        format!(r#"{{"name":"{name}","size":4088320,"browser_download_url":"https://github.com/{REPO}/releases/download/v0.3.0/{name}"}}"#)
    }

    #[test]
    fn plans_signed_update() {
        let json = release("v0.3.0", &format!("{},{}", asset("WinCleaner-v0.3.0-portable.exe"), asset("WinCleaner-v0.3.0-portable.exe.sig")));
        let p = plan(&json, "0.2.2", true).unwrap();
        assert!(p.info.newer && p.info.installable);
        assert_eq!(p.info.latest, "0.3.0");
        assert_eq!(p.exe.unwrap().0, "WinCleaner-v0.3.0-portable.exe");
        // Sin clave pública en el exe: avisa pero no instala.
        assert!(!plan(&json, "0.2.2", false).unwrap().info.installable);
    }

    #[test]
    fn unsigned_release_only_notifies() {
        let json = release("v0.3.0", &asset("WinCleaner-v0.3.0-portable.exe"));
        let p = plan(&json, "0.2.2", true).unwrap();
        assert!(p.info.newer && !p.info.installable);
    }

    #[test]
    fn same_or_older_is_not_newer() {
        let json = release("v0.2.2", &asset("WinCleaner-v0.2.2-portable.exe"));
        assert!(!plan(&json, "0.2.2", true).unwrap().info.newer);
        assert!(!plan(&json, "0.3.0", true).unwrap().info.newer);
    }

    #[test]
    fn ignores_assets_hosted_elsewhere() {
        let bad = r#"{"name":"WinCleaner-v0.3.0-portable.exe","size":10,"browser_download_url":"https://example.com/WinCleaner-v0.3.0-portable.exe"}"#;
        let json = release("v0.3.0", &format!("{bad},{}", asset("WinCleaner-v0.3.0-portable.exe.sig")));
        let p = plan(&json, "0.2.2", true).unwrap();
        assert!(p.exe.is_none() && !p.info.installable);
    }

    #[test]
    fn swaps_exe_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("WinCleaner.exe");
        fs::write(&exe, b"old").unwrap();
        swap_exe(&exe, b"new").unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"new");
        assert_eq!(fs::read(old_path(&exe)).unwrap(), b"old");
        assert!(!sibling(&exe, "new").exists());
    }

    #[test]
    fn swap_failure_leaves_original() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("missing.exe");
        assert!(swap_exe(&exe, b"new").is_err());
        assert!(!sibling(&exe, "new").exists());
    }
}

#[cfg(test)]
mod live {
    /// Consulta real a GitHub: `cargo test -p wincleaner-core live -- --ignored`.
    #[test]
    #[ignore]
    fn checks_github() {
        let u = super::app_update_check().unwrap();
        assert!(super::parse_version(&u.latest).is_some());
        println!("{u:?}");
    }
}
