#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use wincleaner_core::{
    builtin_rules, clean_rule, list_batches, purge_batch, restore_batch, scan_rule, BatchInfo,
    CleanReport, Mode, Rule, ScanResult,
};

#[derive(Serialize)]
struct CleanOutcome {
    batch: String,
    reports: Vec<CleanReport>,
}

#[derive(Serialize)]
struct RestoreOutcome {
    restored: u64,
    errors: Vec<String>,
}

/// Todo lo que se limpia va a la cuarentena de WinCleaner, nunca se borra directamente.
fn quarantine_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("WinCleaner").join("Quarantine")
}

#[tauri::command]
fn list_rules() -> Vec<Rule> {
    builtin_rules()
}

#[tauri::command]
fn scan(ids: Vec<String>) -> Vec<ScanResult> {
    builtin_rules().iter().filter(|r| ids.contains(&r.id)).map(scan_rule).collect()
}

#[tauri::command]
fn clean(ids: Vec<String>) -> CleanOutcome {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let batch = secs.to_string();
    let mode = Mode::Quarantine { dir: quarantine_dir(), batch: batch.clone() };
    let reports = builtin_rules().iter().filter(|r| ids.contains(&r.id)).map(|r| clean_rule(r, &mode)).collect();
    CleanOutcome { batch, reports }
}

#[tauri::command]
fn restore(batch: String) -> RestoreOutcome {
    // El lote viene de la UI: solo se aceptan dígitos para no salir de la carpeta de cuarentena.
    if batch.is_empty() || !batch.bytes().all(|b| b.is_ascii_digit()) {
        return RestoreOutcome { restored: 0, errors: vec!["lote no válido".into()] };
    }
    let (restored, errors) = restore_batch(&quarantine_dir(), &batch);
    RestoreOutcome { restored, errors }
}

#[derive(Serialize)]
struct QuarantineInfo {
    path: String,
    batches: Vec<BatchInfo>,
}

#[tauri::command]
fn quarantine() -> QuarantineInfo {
    let dir = quarantine_dir();
    QuarantineInfo { path: dir.display().to_string(), batches: list_batches(&dir) }
}

/// `batch` vacío = vaciar toda la cuarentena. Es el único borrado real de la app.
#[tauri::command]
fn purge(batch: Option<String>) -> Result<(), String> {
    let dir = quarantine_dir();
    let targets = match batch {
        Some(b) => vec![b],
        None => list_batches(&dir).into_iter().map(|b| b.batch).collect(),
    };
    for b in targets {
        purge_batch(&dir, &b).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_quarantine() -> Result<(), String> {
    let dir = quarantine_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    std::process::Command::new("explorer").arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_rules, scan, clean, restore, quarantine, purge, open_quarantine])
        .run(tauri::generate_context!())
        .expect("error al iniciar WinCleaner");
}
