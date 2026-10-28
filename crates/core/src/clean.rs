use crate::rules::{expand_env, Rule};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Serialize)]
pub struct ScanResult {
    pub rule_id: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct CleanReport {
    pub rule_id: String,
    pub dry_run: bool,
    pub files: u64,
    pub bytes: u64,
    pub errors: Vec<String>,
}

/// Qué hace la limpieza con cada archivo. No existe un modo de borrado directo:
/// lo limpiado siempre se puede recuperar hasta que se vacíe la cuarentena.
#[derive(Debug, Clone)]
pub enum Mode {
    DryRun,
    /// Mueve los archivos a `dir/<lote>/` y deja un manifiesto para restaurarlos.
    Quarantine { dir: PathBuf, batch: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    original: PathBuf,
    stored: PathBuf,
}

const MANIFEST: &str = "manifest.json";

/// Una raíz demasiado corta (`C:\`, `/`) nunca se limpia, pase lo que pase en la regla.
fn is_safe_root(p: &Path) -> bool {
    p.is_absolute() && p.components().count() > 2
}

/// Mueve un archivo; si origen y destino están en volúmenes distintos, copia y borra.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    fs::copy(from, to)?;
    fs::remove_file(from)
}

struct Ctx<'a> {
    mode: &'a Mode,
    rep: CleanReport,
    entries: Vec<Entry>,
}

fn walk(dir: &Path, root: &Path, rule_id: &str, ctx: &mut Ctx) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        // symlink_metadata: no seguimos enlaces simbólicos ni junctions.
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            walk(&path, root, rule_id, ctx);
            if matches!(ctx.mode, Mode::Quarantine { .. }) {
                let _ = fs::remove_dir(&path); // solo si quedó vacío
            }
            continue;
        }
        let size = meta.len();
        match ctx.mode {
            Mode::DryRun => {
                ctx.rep.files += 1;
                ctx.rep.bytes += size;
            }
            Mode::Quarantine { dir: qdir, batch } => {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let stored = qdir.join(batch).join(rule_id).join(rel);
                match move_file(&path, &stored) {
                    Ok(()) => {
                        ctx.rep.files += 1;
                        ctx.rep.bytes += size;
                        ctx.entries.push(Entry { original: path, stored });
                    }
                    Err(e) => ctx.rep.errors.push(format!("{}: {e}", path.display())),
                }
            }
        }
    }
}

fn run(rule: &Rule, mode: &Mode) -> CleanReport {
    let rep = CleanReport {
        rule_id: rule.id.clone(),
        dry_run: matches!(mode, Mode::DryRun),
        ..Default::default()
    };
    let mut ctx = Ctx { mode, rep, entries: Vec::new() };
    for raw in &rule.paths {
        match expand_env(raw) {
            Some(p) if is_safe_root(&p) => walk(&p, &p, &rule.id, &mut ctx),
            Some(p) => ctx.rep.errors.push(format!("ruta rechazada por segura: {}", p.display())),
            None => {} // variable no definida (p. ej. otro SO): se omite
        }
    }
    if let Mode::Quarantine { dir, batch } = mode {
        if !ctx.entries.is_empty() {
            append_manifest(&dir.join(batch), &rule.id, &ctx.entries, &mut ctx.rep);
        }
    }
    ctx.rep
}

fn append_manifest(batch_dir: &Path, rule_id: &str, entries: &[Entry], rep: &mut CleanReport) {
    let file = batch_dir.join(format!("{rule_id}.{MANIFEST}"));
    let result = serde_json::to_vec(entries)
        .map_err(std::io::Error::other)
        .and_then(|bytes| fs::write(&file, bytes));
    if let Err(e) = result {
        rep.errors.push(format!("no se pudo escribir el manifiesto {}: {e}", file.display()));
    }
}

pub fn scan_rule(rule: &Rule) -> ScanResult {
    let r = run(rule, &Mode::DryRun);
    ScanResult { rule_id: r.rule_id, files: r.files, bytes: r.bytes }
}

pub fn clean_rule(rule: &Rule, mode: &Mode) -> CleanReport {
    run(rule, mode)
}

/// Devuelve cada archivo del lote a su sitio original. No sobrescribe nada que
/// exista ya en destino. Devuelve (restaurados, errores).
pub fn restore_batch(quarantine_dir: &Path, batch: &str) -> (u64, Vec<String>) {
    let batch_dir = quarantine_dir.join(batch);
    let mut restored = 0;
    let mut errors = Vec::new();
    let Ok(read) = fs::read_dir(&batch_dir) else {
        return (0, vec![format!("lote no encontrado: {batch}")]);
    };
    for f in read.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        if !name.ends_with(MANIFEST) {
            continue;
        }
        let Ok(bytes) = fs::read(f.path()) else { continue };
        let Ok(entries) = serde_json::from_slice::<Vec<Entry>>(&bytes) else { continue };
        for e in entries {
            if e.original.exists() {
                errors.push(format!("ya existe, no se sobrescribe: {}", e.original.display()));
            } else if let Err(err) = move_file(&e.stored, &e.original) {
                errors.push(format!("{}: {err}", e.original.display()));
            } else {
                restored += 1;
            }
        }
    }
    (restored, errors)
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchInfo {
    pub batch: String,
    pub files: u64,
    pub bytes: u64,
}

fn valid_batch(batch: &str) -> bool {
    !batch.is_empty() && batch.bytes().all(|b| b.is_ascii_digit())
}

/// Lotes que hay ahora mismo en la cuarentena, del más reciente al más antiguo.
pub fn list_batches(quarantine_dir: &Path) -> Vec<BatchInfo> {
    let Ok(read) = fs::read_dir(quarantine_dir) else { return Vec::new() };
    let mut out = Vec::new();
    for d in read.flatten() {
        let batch = d.file_name().to_string_lossy().into_owned();
        if !valid_batch(&batch) {
            continue;
        }
        let mut info = BatchInfo { batch, files: 0, bytes: 0 };
        for f in fs::read_dir(d.path()).into_iter().flatten().flatten() {
            if !f.file_name().to_string_lossy().ends_with(MANIFEST) {
                continue;
            }
            let Ok(bytes) = fs::read(f.path()) else { continue };
            let Ok(entries) = serde_json::from_slice::<Vec<Entry>>(&bytes) else { continue };
            for e in entries {
                if let Ok(m) = fs::symlink_metadata(&e.stored) {
                    info.files += 1;
                    info.bytes += m.len();
                }
            }
        }
        out.push(info);
    }
    out.sort_by(|a, b| b.batch.len().cmp(&a.batch.len()).then(b.batch.cmp(&a.batch)));
    out
}

/// Borra definitivamente un lote de la cuarentena. Es la única operación de
/// borrado real y solo se ejecuta cuando la persona la pide expresamente.
pub fn purge_batch(quarantine_dir: &Path, batch: &str) -> std::io::Result<()> {
    if !valid_batch(batch) {
        return Err(std::io::Error::other("lote no válido"));
    }
    fs::remove_dir_all(quarantine_dir.join(batch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_for(dir: &Path, var: &str) -> Rule {
        std::env::set_var(var, dir);
        Rule {
            id: "t".into(),
            name: "t".into(),
            category: "t".into(),
            description: "t".into(),
            paths: vec![format!("%{var}%")],
            safe: true,
        }
    }

    fn quarantine(dir: &Path) -> Mode {
        Mode::Quarantine { dir: dir.to_path_buf(), batch: "b1".into() }
    }

    #[test]
    fn scan_counts_and_quarantine_moves_files() {
        let tmp = tempfile::tempdir().unwrap();
        let q = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("a/b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("x.bin"), [0u8; 100]).unwrap();
        fs::write(tmp.path().join("y.bin"), [0u8; 50]).unwrap();
        let rule = rule_for(tmp.path(), "WC_T1");

        let s = scan_rule(&rule);
        assert_eq!((s.files, s.bytes), (2, 150));

        let dry = clean_rule(&rule, &Mode::DryRun);
        assert!(dry.dry_run && tmp.path().join("y.bin").exists());

        let real = clean_rule(&rule, &quarantine(q.path()));
        assert_eq!((real.files, real.bytes), (2, 150));
        assert!(tmp.path().exists());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
        assert!(q.path().join("b1/t/y.bin").exists());
    }

    #[test]
    fn restore_returns_files_and_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let q = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("y.bin"), b"original").unwrap();
        fs::write(tmp.path().join("z.bin"), b"zeta").unwrap();
        let rule = rule_for(tmp.path(), "WC_T3");
        clean_rule(&rule, &quarantine(q.path()));

        // Un archivo nuevo ocupa el sitio de y.bin antes de restaurar.
        fs::write(tmp.path().join("y.bin"), b"nuevo").unwrap();
        let (restored, errors) = restore_batch(q.path(), "b1");

        assert_eq!(restored, 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(fs::read(tmp.path().join("y.bin")).unwrap(), b"nuevo");
        assert_eq!(fs::read(tmp.path().join("z.bin")).unwrap(), b"zeta");
    }

    #[test]
    fn list_and_purge_batches() {
        let tmp = tempfile::tempdir().unwrap();
        let q = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.bin"), [0u8; 30]).unwrap();
        fs::write(tmp.path().join("b.bin"), [0u8; 20]).unwrap();
        let rule = rule_for(tmp.path(), "WC_T4");
        clean_rule(&rule, &Mode::Quarantine { dir: q.path().to_path_buf(), batch: "100".into() });

        let batches = list_batches(q.path());
        assert_eq!(batches.len(), 1);
        assert_eq!((batches[0].files, batches[0].bytes), (2, 50));

        assert!(purge_batch(q.path(), "../x").is_err());
        purge_batch(q.path(), "100").unwrap();
        assert!(list_batches(q.path()).is_empty());
    }

    #[test]
    fn refuses_filesystem_root() {
        let q = tempfile::tempdir().unwrap();
        let rule = rule_for(Path::new("/"), "WC_T2");
        let rep = clean_rule(&rule, &quarantine(q.path()));
        assert_eq!(rep.files, 0);
        assert_eq!(rep.errors.len(), 1);
    }
}
