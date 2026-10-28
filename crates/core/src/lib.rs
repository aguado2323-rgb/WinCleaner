//! Núcleo de WinCleaner: reglas declarativas, escaneo y limpieza segura.
//! Sin dependencias de UI para poder probarlo en cualquier plataforma.

mod clean;
mod disk;
mod restore;
mod startup;
mod rules;

pub use clean::{
    clean_rule, list_batches, purge_batch, restore_batch, scan_rule, BatchInfo, CleanReport, Mode,
    ScanResult,
};
pub use rules::{builtin_rules, expand_env, Rule};
pub use disk::{drives, scan_dir, Child, DirReport, FileEntry};
pub use startup::{list as startup_list, set_enabled as startup_set_enabled, StartupItem};
pub use restore::{
    create_restore_point, is_admin, list_restore_points, open_system_restore, relaunch_as_admin, Reason,
    RestorePoint,
};
