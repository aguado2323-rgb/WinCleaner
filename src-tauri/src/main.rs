#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use wincleaner_core::{
    builtin_rules, clean_rule, create_restore_point, debloat_remove, debloat_scan, drives, find_leftovers, is_admin, list_batches,
    list_programs, program_icons, list_restore_points, open_system_restore, purge_batch, relaunch_as_admin,
    remove_leftovers, restore_batch, restore_registry, uninstall_program,
    scan_dir, scan_rule, startup_list, startup_set_enabled, BatchInfo, CleanReport, DirReport, Mode,
    Installed, Leftover, LeftoverOutcome, Program, Reason, RemoveResult, RestorePoint, Rule, ScanResult, StartupItem, UninstallOutcome,
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

#[derive(Serialize)]
struct Root {
    label: String,
    path: String,
}

#[tauri::command]
fn disk_roots() -> Vec<Root> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE") {
        roots.push(Root { label: "Tu carpeta".into(), path: PathBuf::from(home).display().to_string() });
    }
    for d in drives() {
        roots.push(Root { label: format!("Disco {}", &d[..2]), path: d });
    }
    roots
}

/// Solo lectura. Recorre el árbol en un hilo aparte para no bloquear la ventana.
#[tauri::command]
async fn analyze(path: String) -> Result<DirReport, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err("la ruta no es una carpeta".into());
    }
    tauri::async_runtime::spawn_blocking(move || scan_dir(&p, 25))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("no existe".into());
    }
    #[cfg(windows)]
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", p.display()))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn startup_items() -> Vec<StartupItem> {
    startup_list()
}

#[tauri::command]
fn startup_set(id: String, enabled: bool) -> Result<(), String> {
    startup_set_enabled(&id, enabled)
}

#[derive(Serialize)]
struct RestoreState {
    admin: bool,
    points: Vec<RestorePoint>,
}

/// PowerShell tarda un poco: se ejecuta en un hilo aparte para no congelar la ventana.
#[tauri::command]
async fn restore_state() -> Result<RestoreState, String> {
    tauri::async_runtime::spawn_blocking(|| RestoreState { admin: is_admin(), points: list_restore_points() })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn restore_create(reason: String) -> Result<(), String> {
    let reason = Reason::parse(&reason).ok_or("motivo no válido")?;
    tauri::async_runtime::spawn_blocking(move || create_restore_point(reason))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn restore_open() -> Result<(), String> {
    open_system_restore()
}

fn data_dir() -> PathBuf {
    quarantine_dir().parent().map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

fn now_batch() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).to_string()
}

#[tauri::command]
async fn programs_list() -> Result<Vec<Program>, String> {
    tauri::async_runtime::spawn_blocking(list_programs).await.map_err(|e| e.to_string())
}

/// Iconos (PNG en base64) de los programas pedidos; PowerShell tarda, así que va en un hilo aparte.
#[tauri::command]
async fn programs_icons(ids: Vec<String>) -> Result<std::collections::HashMap<String, String>, String> {
    tauri::async_runtime::spawn_blocking(move || program_icons(&ids)).await.map_err(|e| e.to_string())
}

/// Espera a que termine el desinstalador oficial, así que corre en un hilo aparte.
#[tauri::command]
async fn program_uninstall(id: String) -> Result<UninstallOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || uninstall_program(&id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn leftovers_find(name: String, publisher: String, location: String) -> Result<Vec<Leftover>, String> {
    tauri::async_runtime::spawn_blocking(move || find_leftovers(&name, &publisher, &location))
        .await
        .map_err(|e| e.to_string())
}

/// Las rutas vuelven desde la interfaz, así que el núcleo las valida de nuevo una a una.
#[tauri::command]
async fn leftovers_remove(items: Vec<Leftover>) -> Result<LeftoverOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || remove_leftovers(&items, &data_dir(), &now_batch()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn registry_restore(batch: String) -> Result<u64, String> {
    tauri::async_runtime::spawn_blocking(move || restore_registry(&data_dir(), &batch))
        .await
        .map_err(|e| e.to_string())?
}

/// Apps del catálogo de debloat que están instaladas. PowerShell tarda: hilo aparte.
#[tauri::command]
async fn debloat_list() -> Result<Vec<Installed>, String> {
    tauri::async_runtime::spawn_blocking(debloat_scan).await.map_err(|e| e.to_string())?
}

/// Quita apps por id de catálogo (el nombre del paquete nunca viene de la interfaz).
#[tauri::command]
async fn debloat_apply(ids: Vec<String>) -> Result<Vec<RemoveResult>, String> {
    tauri::async_runtime::spawn_blocking(move || debloat_remove(&ids)).await.map_err(|e| e.to_string())
}

/// Lanza otra instancia con permisos de administrador y cierra esta.
#[tauri::command]
fn restart_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    relaunch_as_admin()?;
    app.exit(0);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_rules, scan, clean, restore, quarantine, purge, open_quarantine, disk_roots, analyze, reveal, startup_items, startup_set, restore_state, restore_create, restore_open, restart_as_admin, debloat_list, debloat_apply, programs_list, programs_icons, program_uninstall, leftovers_find, leftovers_remove, registry_restore])
        .run(tauri::generate_context!())
        .expect("error al iniciar WinCleaner");
}
