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

/// Si ya hay algo en `to` (dos archivos con el mismo nombre en la misma limpieza), busca un nombre libre
/// («nombre (2).ext»): mover encima destruiría la copia anterior, porque en Windows `rename` sobrescribe.
fn free_name(to: &Path) -> PathBuf {
    if fs::symlink_metadata(to).is_err() {
        return to.to_path_buf();
    }
    let stem = to.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = to.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2u32..)
        .map(|n| to.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| fs::symlink_metadata(p).is_err())
        .unwrap_or_else(|| to.to_path_buf())
}

/// Mueve un archivo y devuelve dónde ha quedado; si origen y destino están en volúmenes distintos,
/// copia y borra. Nunca sobrescribe algo que ya estuviera en la cuarentena.
fn move_file(from: &Path, to: &Path) -> std::io::Result<PathBuf> {
    let to = free_name(to);
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(from, &to).is_ok() {
        return Ok(to);
    }
    fs::copy(from, &to)?;
    fs::remove_file(from)?;
    Ok(to)
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
        // Los datos vienen del listado de la carpeta (como symlink_metadata, no sigue enlaces simbólicos
        // ni junctions), así no se abre cada archivo solo para saber su tamaño.
        let Ok(meta) = entry.metadata() else { continue };
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
                    Ok(stored) => {
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

/// Mueve archivos o carpetas elegidos a mano (analizador de espacio) a la cuarentena.
/// Las rutas protegidas del sistema y los enlaces simbólicos se rechazan siempre.
pub fn quarantine_items(items: &[PathBuf], quarantine_dir: &Path, batch: &str) -> CleanReport {
    let mode = Mode::Quarantine { dir: quarantine_dir.to_path_buf(), batch: batch.to_string() };
    let rep = CleanReport { rule_id: "space".into(), ..Default::default() };
    let mut ctx = Ctx { mode: &mode, rep, entries: Vec::new() };
    let protected = crate::protect::Protected::from_env();
    for item in items {
        if protected.is_protected(item) {
            ctx.rep.errors.push(format!("protegido por seguridad: {}", item.display()));
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(item) else {
            ctx.rep.errors.push(format!("no existe: {}", item.display()));
            continue;
        };
        if meta.file_type().is_symlink() {
            ctx.rep.errors.push(format!("enlace simbólico, no se toca: {}", item.display()));
        } else if meta.is_dir() {
            // La raíz de la ruta relativa es el padre, para conservar el nombre de la carpeta.
            let parent = item.parent().unwrap_or(item);
            walk(item, parent, "space", &mut ctx);
            let _ = fs::remove_dir(item); // solo si quedó vacía
        } else {
            let name = item.file_name().map(PathBuf::from).unwrap_or_default();
            let stored = quarantine_dir.join(batch).join("space").join(name);
            match move_file(item, &stored) {
                Ok(stored) => {
                    ctx.rep.files += 1;
                    ctx.rep.bytes += meta.len();
                    ctx.entries.push(Entry { original: item.clone(), stored });
                }
                Err(e) => ctx.rep.errors.push(format!("{}: {e}", item.display())),
            }
        }
    }
    if !ctx.entries.is_empty() {
        let mut rep = std::mem::take(&mut ctx.rep);
        append_manifest(&quarantine_dir.join(batch), "space", &ctx.entries, &mut rep);
        ctx.rep = rep;
    }
    ctx.rep
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
        // Una limpieza cuyos archivos ya no están (solo queda su registro) no sirve para nada:
        // se retira para que no salga como pendiente.
        if !has_payload(&d.path()) && remove_tree(&d.path()).is_ok() {
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
    let dir = quarantine_dir.join(batch);
    purge_dir(&dir);
    // Las carpetas vacías (y el registro de la limpieza) se quitan siempre; si aún queda algo,
    // son archivos que Windows no deja borrar.
    if remove_tree(&dir).is_ok() || !dir.exists() {
        return Ok(());
    }
    let left = count_files(&dir);
    if left == 0 {
        // Solo quedan carpetas vacías: no hay nada que conservar.
        return remove_tree(&dir);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!(
            "{left} archivos no se han podido borrar porque pertenecen al sistema o a otro usuario; \
             abre WinCleaner como administrador y vuelve a intentarlo (por ejemplo: {})",
            first_left(&dir).map(|p| p.display().to_string()).unwrap_or_default()
        ),
    ))
}

/// Quita el atributo de solo lectura de un elemento, si lo tiene.
fn clear_readonly(path: &Path, meta: &fs::Metadata) {
    let mut perms = meta.permissions();
    if perms.readonly() {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = fs::set_permissions(path, perms);
    }
}

/// En Windows, las rutas largas o con nombres raros (punto o espacio final, «nul»...) solo se
/// pueden tocar con el prefijo `\\?\`; sin él, borrar falla incluso como administrador.
fn long_path(p: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.to_string_lossy().replace('/', r"\");
        if p.is_absolute() && !s.starts_with(r"\\?\") {
            if let Some(unc) = s.strip_prefix(r"\\") {
                return PathBuf::from(format!(r"\\?\UNC\{unc}"));
            }
            return PathBuf::from(format!(r"\\?\{s}"));
        }
    }
    p.to_path_buf()
}

/// Borra un elemento suelto: archivo, enlace simbólico de archivo o de carpeta, o unión (junction).
/// Un enlace a carpeta en Windows no se borra con `remove_file`, hace falta `remove_dir`.
fn remove_entry(path: &Path, meta: &fs::Metadata) -> std::io::Result<()> {
    let p = long_path(path);
    clear_readonly(&p, meta);
    match fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) => fs::remove_dir(&p).map_err(|_| e),
    }
}

/// Borra una carpeta entera, quitando antes el atributo de solo lectura a lo que lo tenga
/// (también a las carpetas, que en Windows impide borrarlas). No entra en enlaces.
fn remove_tree(dir: &Path) -> std::io::Result<()> {
    let dir = long_path(dir);
    if let Ok(meta) = fs::symlink_metadata(&dir) {
        clear_readonly(&dir, &meta);
    }
    if let Ok(read) = fs::read_dir(&dir) {
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else { continue };
            if meta.is_dir() && !meta.file_type().is_symlink() {
                let _ = remove_tree(&path);
            } else {
                let _ = remove_entry(&path, &meta);
            }
        }
    }
    fs::remove_dir(&dir)
}

/// Borra los archivos que puede, sin seguir enlaces simbólicos.
fn purge_dir(dir: &Path) {
    let Ok(read) = fs::read_dir(long_path(dir)) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() && !meta.file_type().is_symlink() {
            purge_dir(&path);
        } else {
            let _ = remove_entry(&path, &meta);
        }
    }
}

fn count_files(dir: &Path) -> u64 {
    let mut n = 0;
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        match entry.metadata() {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => n += count_files(&entry.path()),
            Ok(_) => n += 1,
            Err(_) => {}
        }
    }
    n
}

/// Primer elemento (no carpeta) que queda dentro de `dir`, para explicar qué no se pudo borrar.
fn first_left(dir: &Path) -> Option<PathBuf> {
    for entry in fs::read_dir(long_path(dir)).into_iter().flatten().flatten() {
        let path = entry.path();
        match fs::symlink_metadata(&path) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {
                if let Some(p) = first_left(&path) {
                    return Some(p);
                }
            }
            Ok(_) => return Some(path),
            Err(_) => {}
        }
    }
    None
}

/// ¿Queda algo en el lote aparte del registro (manifest) de la limpieza?
fn has_payload(dir: &Path) -> bool {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let Ok(m) = entry.metadata() else { continue };
        if m.is_dir() {
            if has_payload(&entry.path()) {
                return true;
            }
        } else if !entry.file_name().to_string_lossy().ends_with(MANIFEST) {
            return true;
        }
    }
    false
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
            help: String::new(),
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

    #[test]
    fn purge_removes_read_only_files() {
        let q = tempfile::tempdir().unwrap();
        let f = q.path().join("7/rule/sub/ro.bin");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, b"x").unwrap();
        let mut perms = fs::metadata(&f).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&f, perms).unwrap();
        purge_batch(q.path(), "7").unwrap();
        assert!(!q.path().join("7").exists());
    }

    #[test]
    fn purge_removes_read_only_folders_and_leftover_logs_disappear() {
        let q = tempfile::tempdir().unwrap();
        let dir = q.path().join("8/rule/ro");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.bin"), b"x").unwrap();
        fs::write(q.path().join("8/manifest.json"), b"[]").unwrap();
        let mut perms = fs::metadata(&dir).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&dir, perms).ok();
        purge_batch(q.path(), "8").unwrap();
        assert!(!q.path().join("8").exists());

        // Un lote sin archivos (solo el registro) desaparece de la lista.
        fs::create_dir_all(q.path().join("9")).unwrap();
        fs::write(q.path().join("9/manifest.json"), b"[]").unwrap();
        assert!(list_batches(q.path()).is_empty());
        assert!(!q.path().join("9").exists());
    }

    #[cfg(unix)]
    #[test]
    fn purge_removes_symlinks_without_touching_their_target() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep.txt"), b"x").unwrap();
        let q = tempfile::tempdir().unwrap();
        fs::create_dir_all(q.path().join("5/rule")).unwrap();
        std::os::unix::fs::symlink(outside.path(), q.path().join("5/rule/link")).unwrap();
        purge_batch(q.path(), "5").unwrap();
        assert!(!q.path().join("5").exists());
        assert!(outside.path().join("keep.txt").exists());
    }

    #[test]
    fn same_named_files_do_not_overwrite_each_other_in_quarantine() {
        let src = tempfile::tempdir().unwrap();
        let q = tempfile::tempdir().unwrap();
        fs::create_dir_all(src.path().join("a")).unwrap();
        fs::create_dir_all(src.path().join("b")).unwrap();
        fs::write(src.path().join("a/foto.jpg"), b"AAAA").unwrap();
        fs::write(src.path().join("b/foto.jpg"), b"BB").unwrap();
        let rep = quarantine_items(&[src.path().join("a/foto.jpg"), src.path().join("b/foto.jpg")], q.path(), "60");
        assert_eq!((rep.files, rep.bytes, rep.errors.len()), (2, 6, 0));
        let (restored, errors) = restore_batch(q.path(), "60");
        assert_eq!((restored, errors.len()), (2, 0));
        assert_eq!(fs::read(src.path().join("a/foto.jpg")).unwrap(), b"AAAA");
        assert_eq!(fs::read(src.path().join("b/foto.jpg")).unwrap(), b"BB");
    }

    #[test]
    fn quarantine_items_moves_files_and_folders_and_refuses_protected() {
        let src = tempfile::tempdir().unwrap();
        let q = tempfile::tempdir().unwrap();
        fs::create_dir_all(src.path().join("dir/sub")).unwrap();
        fs::write(src.path().join("dir/sub/a.bin"), vec![0u8; 10]).unwrap();
        fs::write(src.path().join("big.iso"), vec![0u8; 5]).unwrap();

        let items = vec![src.path().join("dir"), src.path().join("big.iso"), PathBuf::from("/")];
        let rep = quarantine_items(&items, q.path(), "50");

        assert_eq!((rep.files, rep.bytes), (2, 15));
        assert_eq!(rep.errors.len(), 1, "la raíz se rechaza: {:?}", rep.errors);
        assert!(!src.path().join("dir").exists() && !src.path().join("big.iso").exists());
        assert!(q.path().join("50/space/dir/sub/a.bin").exists());
        assert!(q.path().join("50/space/big.iso").exists());

        let (restored, errors) = restore_batch(q.path(), "50");
        assert_eq!((restored, errors.len()), (2, 0));
        assert!(src.path().join("dir/sub/a.bin").exists());
    }
}
