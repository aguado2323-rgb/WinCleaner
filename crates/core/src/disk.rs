//! Analizador de espacio: solo lectura. Resume qué ocupa cada carpeta.

use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Child {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirReport {
    pub path: String,
    pub bytes: u64,
    pub files: u64,
    /// Contenido directo de la carpeta, del más grande al más pequeño.
    pub children: Vec<Child>,
    /// Los archivos más grandes de todo el árbol.
    pub largest: Vec<FileEntry>,
    /// Carpetas que no se pudieron leer (permisos).
    pub skipped: u64,
}

struct Walk {
    top: usize,
    heap: BinaryHeap<Reverse<(u64, PathBuf)>>,
    skipped: u64,
}

impl Walk {
    fn note_file(&mut self, path: PathBuf, bytes: u64) {
        if self.top == 0 {
            return;
        }
        self.heap.push(Reverse((bytes, path)));
        if self.heap.len() > self.top {
            self.heap.pop();
        }
    }

    /// Devuelve (bytes, archivos) del subárbol. No sigue enlaces simbólicos.
    fn dir(&mut self, dir: &Path) -> (u64, u64) {
        let Ok(read) = fs::read_dir(dir) else {
            self.skipped += 1;
            return (0, 0);
        };
        let (mut bytes, mut files) = (0, 0);
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                let (b, f) = self.dir(&path);
                bytes += b;
                files += f;
            } else {
                bytes += meta.len();
                files += 1;
                self.note_file(path, meta.len());
            }
        }
        (bytes, files)
    }
}

pub fn scan_dir(path: &Path, top: usize) -> DirReport {
    let mut walk = Walk { top, heap: BinaryHeap::new(), skipped: 0 };
    let mut children = Vec::new();
    let (mut total, mut total_files) = (0, 0);

    match fs::read_dir(path) {
        Ok(read) => {
            for entry in read.flatten() {
                let p = entry.path();
                let Ok(meta) = fs::symlink_metadata(&p) else { continue };
                if meta.file_type().is_symlink() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let (bytes, files, is_dir) = if meta.is_dir() {
                    let (b, f) = walk.dir(&p);
                    (b, f, true)
                } else {
                    walk.note_file(p.clone(), meta.len());
                    (meta.len(), 1, false)
                };
                total += bytes;
                total_files += files;
                children.push(Child { name, path: p.display().to_string(), is_dir, bytes, files });
            }
        }
        Err(_) => walk.skipped += 1,
    }

    children.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    let mut largest: Vec<_> = walk.heap.into_iter().map(|Reverse((b, p))| (b, p)).collect();
    largest.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    DirReport {
        path: path.display().to_string(),
        bytes: total,
        files: total_files,
        children,
        largest: largest
            .into_iter()
            .map(|(bytes, p)| FileEntry { path: p.display().to_string(), bytes })
            .collect(),
        skipped: walk.skipped,
    }
}

/// Unidades de disco presentes (`C:\`, `D:\`…). En otros sistemas devuelve vacío.
pub fn drives() -> Vec<String> {
    ('A'..='Z')
        .map(|c| format!("{c}:\\"))
        .filter(|d| cfg!(windows) && Path::new(d).exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_children_and_largest_files() {
        let tmp = tempfile::tempdir().unwrap();
        let big = tmp.path().join("big");
        let small = tmp.path().join("small/deep");
        fs::create_dir_all(&big).unwrap();
        fs::create_dir_all(&small).unwrap();
        fs::write(big.join("a.bin"), vec![0u8; 500]).unwrap();
        fs::write(big.join("b.bin"), vec![0u8; 100]).unwrap();
        fs::write(small.join("c.bin"), vec![0u8; 40]).unwrap();
        fs::write(tmp.path().join("root.txt"), vec![0u8; 60]).unwrap();

        let r = scan_dir(tmp.path(), 2);
        assert_eq!((r.bytes, r.files), (700, 4));
        let names: Vec<_> = r.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["big", "root.txt", "small"]);
        assert!(r.children[0].is_dir && !r.children[1].is_dir);
        assert_eq!(r.largest.len(), 2);
        assert_eq!(r.largest[0].bytes, 500);
        assert_eq!(r.largest[1].bytes, 100);
    }

    #[test]
    fn unreadable_path_is_reported_not_fatal() {
        let r = scan_dir(Path::new("/definitivamente/no/existe"), 5);
        assert_eq!((r.bytes, r.skipped), (0, 1));
    }
}
