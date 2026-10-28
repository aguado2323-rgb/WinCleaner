//! Historial de limpiezas para el informe de uso. Solo guarda cifras (fecha, tipo, archivos y
//! bytes): nunca rutas ni nombres de archivos.

use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "history.json";
/// Tope de entradas guardadas; las más antiguas se descartan.
const MAX: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Limpieza manual a la cuarentena.
    Clean,
    /// Limpieza programada a la cuarentena.
    Auto,
    /// Cuarentena vaciada (borrado real).
    Purge,
    /// Papelera de reciclaje vaciada.
    Recycle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub when: u64,
    pub kind: Kind,
    pub files: u64,
    pub bytes: u64,
}

pub fn load(data_dir: &Path) -> Vec<Event> {
    std::fs::read(data_dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Añade un evento. Los fallos de escritura se ignoran: el historial nunca debe romper una limpieza.
pub fn record(data_dir: &Path, event: Event) {
    if event.files == 0 && event.bytes == 0 {
        return;
    }
    let mut all = load(data_dir);
    all.push(event);
    if all.len() > MAX {
        let extra = all.len() - MAX;
        all.drain(..extra);
    }
    let _ = std::fs::create_dir_all(data_dir);
    if let Ok(bytes) = serde_json::to_vec(&all) {
        let _ = std::fs::write(data_dir.join(FILE), bytes);
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct Report {
    /// Movido a la cuarentena (manual + programado).
    pub cleaned_bytes: u64,
    pub cleaned_files: u64,
    /// Borrado de verdad al vaciar la cuarentena.
    pub purged_bytes: u64,
    pub recycle_bytes: u64,
    pub runs: u64,
    /// Bytes limpiados por día en los últimos 30 días, del más antiguo al de hoy.
    pub last_30_days: Vec<u64>,
    /// Las 15 últimas entradas, de la más reciente a la más antigua.
    pub recent: Vec<Event>,
}

pub fn report(data_dir: &Path, now: u64) -> Report {
    summarize(&load(data_dir), now)
}

pub fn summarize(events: &[Event], now: u64) -> Report {
    let mut r = Report { last_30_days: vec![0; 30], ..Default::default() };
    let today = now / 86_400;
    for e in events {
        match e.kind {
            Kind::Clean | Kind::Auto => {
                r.cleaned_bytes += e.bytes;
                r.cleaned_files += e.files;
                r.runs += 1;
                let day = e.when / 86_400;
                if day <= today && today - day < 30 {
                    r.last_30_days[(29 - (today - day)) as usize] += e.bytes;
                }
            }
            Kind::Purge => r.purged_bytes += e.bytes,
            Kind::Recycle => r.recycle_bytes += e.bytes,
        }
    }
    r.recent = events.iter().rev().take(15).cloned().collect();
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(when: u64, kind: Kind, bytes: u64) -> Event {
        Event { when, kind, files: 1, bytes }
    }

    #[test]
    fn records_and_summarizes() {
        let d = tempfile::tempdir().unwrap();
        let now = 100 * 86_400 + 500;
        record(d.path(), ev(now, Kind::Clean, 1000));
        record(d.path(), ev(now - 86_400, Kind::Auto, 500));
        record(d.path(), ev(now - 40 * 86_400, Kind::Clean, 200));
        record(d.path(), ev(now, Kind::Purge, 700));
        record(d.path(), Event { when: now, kind: Kind::Recycle, files: 0, bytes: 0 }); // sin datos: no se guarda
        let r = report(d.path(), now);
        assert_eq!((r.cleaned_bytes, r.purged_bytes, r.runs), (1700, 700, 3));
        assert_eq!(r.last_30_days[29], 1000);
        assert_eq!(r.last_30_days[28], 500);
        assert_eq!(r.last_30_days.iter().sum::<u64>(), 1500, "lo de hace 40 días queda fuera de la gráfica");
        assert_eq!(r.recent.len(), 4);
    }

    #[test]
    fn corrupt_file_is_ignored_and_history_is_capped() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(FILE), b"no es json").unwrap();
        assert!(load(d.path()).is_empty());
        for i in 0..(MAX as u64 + 5) {
            record(d.path(), ev(i, Kind::Clean, 1));
        }
        assert_eq!(load(d.path()).len(), MAX);
    }
}
