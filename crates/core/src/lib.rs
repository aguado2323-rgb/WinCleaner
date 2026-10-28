//! Núcleo de WinCleaner: reglas declarativas, escaneo y limpieza segura.
//! Sin dependencias de UI para poder probarlo en cualquier plataforma.

mod clean;
mod rules;

pub use clean::{clean_rule, restore_batch, scan_rule, CleanReport, Mode, ScanResult};
pub use rules::{builtin_rules, expand_env, Rule};
