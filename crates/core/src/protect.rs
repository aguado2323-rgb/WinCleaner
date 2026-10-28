//! Rutas que WinCleaner nunca mueve ni borra, decida lo que decida la persona.
//! Es la última barrera para que el analizador de espacio no pueda romper Windows.

use std::path::{Component, Path, PathBuf};

/// Nombres de archivo o carpeta que nunca se tocan, estén donde estén.
const PROTECTED_NAMES: &[&str] = &[
    "pagefile.sys", "hiberfil.sys", "swapfile.sys", "system volume information", "$recycle.bin",
    "recovery", "boot", "efi", "bootmgr", "documents and settings", "windowsapps", "config.msi",
];

#[derive(Debug, Default)]
pub struct Protected {
    /// Estas rutas exactas no se tocan, pero sí su contenido (p. ej. `C:\Users`).
    exact: Vec<String>,
    /// Estas carpetas y todo lo que hay dentro no se tocan (p. ej. `C:\Windows`).
    trees: Vec<String>,
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

impl Protected {
    pub fn new(exact: &[PathBuf], trees: &[PathBuf]) -> Self {
        Protected { exact: exact.iter().map(|p| norm(p)).collect(), trees: trees.iter().map(|p| norm(p)).collect() }
    }

    /// Construye la lista a partir de las variables de entorno de Windows.
    pub fn from_env() -> Self {
        let var = |v: &str| std::env::var_os(v).map(PathBuf::from);
        let mut exact: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramData", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "SystemDrive"]
            .iter()
            .filter_map(|v| var(v))
            .collect();
        let mut trees: Vec<PathBuf> = ["SystemRoot", "WINDIR"].iter().filter_map(|v| var(v)).collect();
        if let Some(drive) = var("SystemDrive") {
            exact.push(drive.join("Users"));
        }
        for pf in ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(|v| var(v)) {
            for sub in ["Common Files", "Windows Defender", "Windows NT", "Windows Mail", "Windows Media Player", "Microsoft Update Health Tools"] {
                trees.push(pf.join(sub));
            }
        }
        if let Some(pd) = var("ProgramData") {
            trees.push(pd.join("Microsoft").join("Windows"));
        }
        Protected::new(&exact, &trees)
    }

    pub fn is_protected(&self, path: &Path) -> bool {
        if !path.is_absolute() {
            return true;
        }
        // Raíz de unidad (`C:\`) o `/`.
        if path.components().filter(|c| matches!(c, Component::Normal(_))).count() == 0 {
            return true;
        }
        // `..` o `.` dentro de la ruta: no se interpreta, se rechaza.
        if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
            return true;
        }
        let n = norm(path);
        if let Some(name) = n.rsplit('/').next() {
            if PROTECTED_NAMES.contains(&name) {
                return true;
            }
        }
        self.exact.contains(&n) || self.trees.iter().any(|t| n == *t || n.starts_with(&format!("{t}/")))
    }
}

pub fn is_protected(path: &Path) -> bool {
    Protected::from_env().is_protected(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rutas de prueba con forma Unix; en Windows se convierten a `C:/...` para que sean absolutas.
    fn abs(s: &str) -> PathBuf {
        if cfg!(windows) {
            let rest = s.trim_start_matches('/');
            match rest.split_once('/') {
                Some((d, tail)) if d.len() == 1 => PathBuf::from(format!("{}:/{tail}", d.to_uppercase())),
                None if rest.len() == 1 => PathBuf::from(format!("{}:/", rest.to_uppercase())),
                _ if rest.is_empty() => PathBuf::from("C:/"),
                _ => PathBuf::from(s),
            }
        } else {
            PathBuf::from(s)
        }
    }

    fn prot() -> Protected {
        Protected::new(
            &[abs("/c/Users"), abs("/c/Program Files"), abs("/c/Users/aldo")],
            &[abs("/c/Windows")],
        )
    }

    #[test]
    fn system_trees_and_roots_are_protected() {
        let p = prot();
        assert!(p.is_protected(&abs("/")));
        assert!(p.is_protected(&abs("/c/Windows")));
        assert!(p.is_protected(&abs("/c/windows/System32/cmd.exe")), "sin distinguir mayúsculas");
        assert!(p.is_protected(&abs("/c/Users")));
        assert!(p.is_protected(&abs("/c/Program Files")));
        assert!(p.is_protected(&abs("/c/Users/aldo")));
        assert!(p.is_protected(Path::new("relativa/ruta")));
        assert!(p.is_protected(&abs("/c/Users/aldo/../../Windows")));
    }

    #[test]
    fn special_files_are_protected_anywhere() {
        let p = prot();
        assert!(p.is_protected(&abs("/c/pagefile.sys")));
        assert!(p.is_protected(&abs("/d/System Volume Information")));
        assert!(p.is_protected(&abs("/c/$Recycle.Bin")));
    }

    #[test]
    fn ordinary_user_content_is_allowed() {
        let p = prot();
        assert!(!p.is_protected(&abs("/c/Users/aldo/Downloads/big.iso")));
        assert!(!p.is_protected(&abs("/c/Program Files/Algun Juego")));
        assert!(!p.is_protected(&abs("/c/Juegos")));
        assert!(!p.is_protected(&abs("/c/Windows.old")));
    }
}
