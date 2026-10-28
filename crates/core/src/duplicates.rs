//! Archivos duplicados y archivos grandes que llevan tiempo sin tocarse. Solo lee; lo que se elige
//! va a la cuarentena (nada se borra). Los duplicados se comprueban por contenido (SHA-256), no por nombre,
//! y antes de mover nada se vuelven a comprobar contra el archivo que se conserva.

use crate::clean::{quarantine_items, CleanReport};
use crate::protect::Protected;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Carpetas del usuario que se pueden revisar. La interfaz solo envía el id.
pub const FOLDERS: &[(&str, &str, &str)] = &[
    ("downloads", "Descargas", "Downloads"),
    ("documents", "Documentos", "Documents"),
    ("desktop", "Escritorio", "Desktop"),
    ("pictures", "Imágenes", "Pictures"),
    ("videos", "Vídeos", "Videos"),
    ("music", "Música", "Music"),
];

/// Carpetas que no aportan nada (copias de librerías, historiales) o que no se deben revisar.
const SKIP_DIRS: &[&str] = &["node_modules", ".git", "appdata", "$recycle.bin", "system volume information"];

#[derive(Debug, Clone, Serialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// Carpetas del usuario que existen en este equipo.
pub fn folders() -> Vec<Folder> {
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from) else {
        return Vec::new();
    };
    FOLDERS
        .iter()
        .map(|(id, name, dir)| Folder { id: (*id).into(), name: (*name).into(), path: home.join(dir).to_string_lossy().into_owned() })
        .filter(|f| Path::new(&f.path).is_dir())
        .collect()
}

fn roots_for(ids: &[String]) -> Vec<PathBuf> {
    folders().into_iter().filter(|f| ids.contains(&f.id)).map(|f| PathBuf::from(f.path)).collect()
}

fn mtime(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

/// Recorre las carpetas sin seguir enlaces simbólicos y sin entrar en rutas protegidas.
fn walk(dir: &Path, prot: &Protected, skipped: &mut u64, visit: &mut dyn FnMut(&Path, &fs::Metadata)) {
    let Ok(read) = fs::read_dir(dir) else {
        *skipped += 1;
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !SKIP_DIRS.contains(&name.as_str()) && !prot.is_protected(&path) {
                walk(&path, prot, skipped, visit);
            }
        } else if meta.is_file() {
            visit(&path, &meta);
        }
    }
}

/// SHA-256 del archivo (todo, o solo los primeros `limit` bytes).
fn hash_file(path: &Path, limit: Option<u64>) -> Option<[u8; 32]> {
    let mut f = File::open(path).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut left = limit.unwrap_or(u64::MAX);
    while left > 0 {
        let want = buf.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        left -= n as u64;
    }
    Some(h.finalize().into())
}

#[derive(Debug, Clone, Serialize)]
pub struct DupFile {
    pub path: String,
    /// Última modificación, en segundos desde 1970.
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DupGroup {
    /// Tamaño de cada copia.
    pub bytes: u64,
    pub files: Vec<DupFile>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DupReport {
    pub groups: Vec<DupGroup>,
    pub scanned: u64,
    /// Espacio que se recupera quedándose con una copia de cada grupo.
    pub wasted: u64,
    pub skipped: u64,
}

/// Busca archivos idénticos de al menos `min_bytes` dentro de las carpetas elegidas.
pub fn find_duplicates(folder_ids: &[String], min_bytes: u64) -> DupReport {
    find_in(&roots_for(folder_ids), min_bytes, &Protected::from_env())
}

fn find_in(roots: &[PathBuf], min_bytes: u64, prot: &Protected) -> DupReport {
    let mut rep = DupReport::default();
    let mut by_size: HashMap<u64, Vec<(PathBuf, u64)>> = HashMap::new();
    for root in roots {
        walk(root, prot, &mut rep.skipped, &mut |p, m| {
            rep.scanned += 1;
            if m.len() >= min_bytes.max(1) {
                by_size.entry(m.len()).or_default().push((p.to_path_buf(), mtime(m)));
            }
        });
    }
    for (size, files) in by_size.into_iter().filter(|(_, f)| f.len() > 1) {
        // Primero los primeros 64 KB (barato); solo los que coinciden se leen enteros.
        let mut partial: HashMap<[u8; 32], Vec<(PathBuf, u64)>> = HashMap::new();
        for f in files {
            if let Some(h) = hash_file(&f.0, Some(64 * 1024)) {
                partial.entry(h).or_default().push(f);
            }
        }
        for cands in partial.into_values().filter(|c| c.len() > 1) {
            let mut full: HashMap<[u8; 32], Vec<(PathBuf, u64)>> = HashMap::new();
            for f in cands {
                if let Some(h) = hash_file(&f.0, None) {
                    full.entry(h).or_default().push(f);
                }
            }
            for mut same in full.into_values().filter(|c| c.len() > 1) {
                same.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
                rep.wasted += size * (same.len() as u64 - 1);
                rep.groups.push(DupGroup {
                    bytes: size,
                    files: same.into_iter().map(|(p, m)| DupFile { path: p.to_string_lossy().into_owned(), modified: m }).collect(),
                });
            }
        }
    }
    rep.groups.sort_by_key(|g| std::cmp::Reverse(g.bytes * g.files.len() as u64));
    rep
}

/// Lo que la persona decide para un grupo: qué copia se queda y cuáles se quitan.
#[derive(Debug, Clone, Deserialize)]
pub struct DupSelection {
    pub keep: String,
    pub remove: Vec<String>,
}

/// Mueve a la cuarentena las copias elegidas. Cada copia se vuelve a comparar con la que se conserva
/// (mismo tamaño y mismo SHA-256) justo antes de moverla; si algo cambió desde el escaneo, se deja.
pub fn quarantine_duplicates(sel: &[DupSelection], quarantine_dir: &Path, batch: &str) -> CleanReport {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    for s in sel {
        let keep = PathBuf::from(&s.keep);
        let ok_keep = fs::symlink_metadata(&keep).ok().filter(|m| m.is_file());
        let (Some(keep_meta), Some(keep_hash)) = (ok_keep, hash_file(&keep, None)) else {
            errors.push(format!("la copia que se conserva ya no está: {}", s.keep));
            continue;
        };
        for r in &s.remove {
            let p = PathBuf::from(r);
            if p == keep {
                errors.push(format!("no se quita la copia que se conserva: {r}"));
                continue;
            }
            let same = fs::symlink_metadata(&p)
                .ok()
                .filter(|m| m.is_file() && m.len() == keep_meta.len())
                .and_then(|_| hash_file(&p, None))
                .is_some_and(|h| h == keep_hash);
            if same {
                items.push(p);
            } else {
                errors.push(format!("ya no es idéntico al original, no se toca: {r}"));
            }
        }
    }
    let mut rep = quarantine_items(&items, quarantine_dir, batch);
    rep.errors.extend(errors);
    rep
}

#[derive(Debug, Clone, Serialize)]
pub struct OldFile {
    pub path: String,
    pub bytes: u64,
    pub modified: u64,
}

/// Archivos de al menos `min_bytes` que no se han modificado en `older_than_days` días, los mayores primero.
pub fn find_large_old(folder_ids: &[String], min_bytes: u64, older_than_days: u64, now_secs: u64) -> Vec<OldFile> {
    large_old_in(&roots_for(folder_ids), min_bytes, older_than_days, now_secs, &Protected::from_env())
}

fn large_old_in(roots: &[PathBuf], min_bytes: u64, older_than_days: u64, now_secs: u64, prot: &Protected) -> Vec<OldFile> {
    let limit = now_secs.saturating_sub(older_than_days * 86_400);
    let mut out = Vec::new();
    let mut skipped = 0;
    for root in roots {
        walk(root, prot, &mut skipped, &mut |p, m| {
            let t = mtime(m);
            if m.len() >= min_bytes && t > 0 && t <= limit && !prot.is_protected(p) {
                out.push(OldFile { path: p.to_string_lossy().into_owned(), bytes: m.len(), modified: t });
            }
        });
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.path.cmp(&b.path)));
    out.truncate(300);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn write(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let p = dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, data).unwrap();
        p
    }

    #[test]
    fn finds_identical_files_by_content_not_name() {
        let t = tempfile::tempdir().unwrap();
        let big = vec![7u8; 200_000];
        let mut almost = big.clone();
        *almost.last_mut().unwrap() = 8; // mismo tamaño y mismo inicio, distinto final
        write(t.path(), "a/foto.jpg", &big);
        write(t.path(), "b/copia de foto.jpg", &big);
        write(t.path(), "c/otra.bin", &almost);
        write(t.path(), "c/pequeno.txt", b"hola");
        write(t.path(), "d/pequeno.txt", b"hola");

        let rep = find_in(&[t.path().to_path_buf()], 1000, &Protected::new(&[], &[]));
        assert_eq!(rep.groups.len(), 1, "{:?}", rep.groups);
        assert_eq!(rep.groups[0].files.len(), 2);
        assert_eq!(rep.wasted, 200_000);

        // Con umbral bajo también salen los pequeños.
        let rep = find_in(&[t.path().to_path_buf()], 1, &Protected::new(&[], &[]));
        assert_eq!(rep.groups.len(), 2);
    }

    #[test]
    fn quarantines_only_verified_copies_and_keeps_one() {
        let t = tempfile::tempdir().unwrap();
        let q = tempfile::tempdir().unwrap();
        let keep = write(t.path(), "a/x.bin", b"contenido");
        let same = write(t.path(), "b/x.bin", b"contenido");
        let different = write(t.path(), "c/x.bin", b"OTRO CONTE");
        let sel = [DupSelection {
            keep: keep.to_string_lossy().into(),
            remove: vec![same.to_string_lossy().into(), different.to_string_lossy().into(), keep.to_string_lossy().into()],
        }];
        let rep = quarantine_duplicates(&sel, q.path(), "70");
        assert_eq!(rep.files, 1);
        assert!(keep.exists() && different.exists() && !same.exists());
        assert_eq!(rep.errors.len(), 2, "{:?}", rep.errors);

        // Si la copia que se conserva desaparece, no se mueve nada.
        fs::remove_file(&keep).unwrap();
        let again = write(t.path(), "e/x.bin", b"contenido");
        let sel = [DupSelection { keep: keep.to_string_lossy().into(), remove: vec![again.to_string_lossy().into()] }];
        let rep = quarantine_duplicates(&sel, q.path(), "71");
        assert_eq!(rep.files, 0);
        assert!(again.exists());
    }

    #[test]
    fn large_old_files_respect_size_age_and_protection() {
        let t = tempfile::tempdir().unwrap();
        let old = write(t.path(), "viejo.iso", &vec![1u8; 5000]);
        let new = write(t.path(), "nuevo.iso", &vec![1u8; 5000]);
        write(t.path(), "pequeno.iso", b"x");
        write(t.path(), "node_modules/pkg/enorme.bin", &vec![1u8; 9000]);
        let year_ago = SystemTime::now() - Duration::from_secs(400 * 86_400);
        File::options().write(true).open(&old).unwrap().set_modified(year_ago).unwrap();
        File::options().write(true).open(write(t.path(), "protegido.iso", &vec![1u8; 7000])).unwrap().set_modified(year_ago).unwrap();
        let _ = new;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let prot = Protected::new(&[], &[]);
        let found = large_old_in(&[t.path().to_path_buf()], 1000, 365, now, &prot);
        let names: Vec<_> = found.iter().map(|f| Path::new(&f.path).file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["protegido.iso", "viejo.iso"], "mayor primero; sin nuevos, pequeños ni node_modules");

        // Una ruta protegida no se ofrece.
        let prot = Protected::new(&[t.path().join("protegido.iso")], &[]);
        let found = large_old_in(&[t.path().to_path_buf()], 1000, 365, now, &prot);
        assert_eq!(found.len(), 1);
    }
}
