use crate::rules::{expand_env, Rule};
use serde::Serialize;
use std::fs;
use std::path::Path;

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

/// Una raíz demasiado corta (`C:\`, `/`) nunca se limpia, pase lo que pase en la regla.
fn is_safe_root(p: &Path) -> bool {
    p.is_absolute() && p.components().count() > 2
}

fn walk(dir: &Path, delete: bool, rep: &mut CleanReport) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        // symlink_metadata: no seguimos enlaces simbólicos ni junctions.
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            walk(&path, delete, rep);
            if delete {
                let _ = fs::remove_dir(&path); // solo si quedó vacío
            }
        } else {
            let size = meta.len();
            if delete {
                match fs::remove_file(&path) {
                    Ok(()) => {
                        rep.files += 1;
                        rep.bytes += size;
                    }
                    Err(e) => rep.errors.push(format!("{}: {e}", path.display())),
                }
            } else {
                rep.files += 1;
                rep.bytes += size;
            }
        }
    }
}

fn run(rule: &Rule, delete: bool) -> CleanReport {
    let mut rep = CleanReport { rule_id: rule.id.clone(), dry_run: !delete, ..Default::default() };
    for raw in &rule.paths {
        match expand_env(raw) {
            Some(p) if is_safe_root(&p) => walk(&p, delete, &mut rep),
            Some(p) => rep.errors.push(format!("ruta rechazada por segura: {}", p.display())),
            None => {} // variable no definida (p. ej. otro SO): se omite
        }
    }
    rep
}

pub fn scan_rule(rule: &Rule) -> ScanResult {
    let r = run(rule, false);
    ScanResult { rule_id: r.rule_id, files: r.files, bytes: r.bytes }
}

pub fn clean_rule(rule: &Rule, dry_run: bool) -> CleanReport {
    run(rule, !dry_run)
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

    #[test]
    fn scan_counts_and_clean_deletes() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("a/b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("x.bin"), [0u8; 100]).unwrap();
        fs::write(tmp.path().join("y.bin"), [0u8; 50]).unwrap();
        let rule = rule_for(tmp.path(), "WC_T1");

        let s = scan_rule(&rule);
        assert_eq!((s.files, s.bytes), (2, 150));

        let dry = clean_rule(&rule, true);
        assert!(dry.dry_run && tmp.path().join("y.bin").exists());

        let real = clean_rule(&rule, false);
        assert_eq!((real.files, real.bytes), (2, 150));
        assert!(tmp.path().exists());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn refuses_filesystem_root() {
        let rule = rule_for(Path::new("/"), "WC_T2");
        let rep = clean_rule(&rule, false);
        assert_eq!(rep.files, 0);
        assert_eq!(rep.errors.len(), 1);
    }
}
