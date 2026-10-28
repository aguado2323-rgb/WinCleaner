#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use wincleaner_core::{
    builtin_rules, clean_rule, create_restore_point, debloat_remove, debloat_scan, drives, find_leftovers, is_admin, list_batches,
    list_programs, program_icons, list_restore_points, quarantine_items, open_system_restore, purge_batch, relaunch_as_admin,
    remove_leftovers, restore_batch, restore_registry, uninstall_program,
    schedule_last_run, schedule_load, schedule_run_now, run_scheduled, schedule_save, schedule_task_exists, LastRun, Schedule,
    find_duplicates, find_large_old, quarantine_duplicates, user_folders, DupReport, DupSelection, Folder, OldFile, service_set, services_list, ServiceInfo, scan_dir, scan_rule, tweak_set, tweaks_list, TweakInfo, startup_list, startup_set_enabled, BatchInfo, CleanReport, DirReport, Mode,
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

/// Mueve a la cuarentena lo elegido en el analizador de espacio. El núcleo rechaza rutas del sistema.
#[tauri::command]
async fn space_quarantine(paths: Vec<String>) -> Result<CleanReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let items: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        quarantine_items(&items, &quarantine_dir(), &now_batch())
    })
    .await
    .map_err(|e| e.to_string())
}

/// Carpetas del usuario que se pueden revisar en busca de duplicados y archivos grandes.
#[tauri::command]
fn dup_folders() -> Vec<Folder> {
    user_folders()
}

/// Archivos idénticos (por contenido) dentro de las carpetas elegidas por id.
#[tauri::command]
async fn dup_scan(folders: Vec<String>, min_mb: u64) -> Result<DupReport, String> {
    tauri::async_runtime::spawn_blocking(move || find_duplicates(&folders, min_mb.saturating_mul(1_000_000)))
        .await
        .map_err(|e| e.to_string())
}

/// Mueve a la cuarentena las copias elegidas; el núcleo las vuelve a comparar con la que se conserva.
#[tauri::command]
async fn dup_quarantine(selection: Vec<DupSelection>) -> Result<CleanReport, String> {
    tauri::async_runtime::spawn_blocking(move || quarantine_duplicates(&selection, &quarantine_dir(), &now_batch()))
        .await
        .map_err(|e| e.to_string())
}

/// Archivos grandes que llevan tiempo sin modificarse. Se mueven con `space_quarantine`.
#[tauri::command]
async fn old_scan(folders: Vec<String>, min_mb: u64, days: u64) -> Result<Vec<OldFile>, String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    tauri::async_runtime::spawn_blocking(move || find_large_old(&folders, min_mb.saturating_mul(1_000_000), days, now))
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

#[tauri::command]
async fn tweaks_state() -> Result<Vec<TweakInfo>, String> {
    tauri::async_runtime::spawn_blocking(tweaks_list).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn services_state() -> Result<Vec<ServiceInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| services_list(&data_dir())).await.map_err(|e| e.to_string())
}

/// Pasa a «Manual» o deshace un servicio del catálogo; el núcleo guarda y restaura el tipo de inicio original.
#[tauri::command]
async fn service_apply(id: String, enable: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || service_set(&id, enable, &data_dir()))
        .await
        .map_err(|e| e.to_string())?
}

/// Aplica o deshace un ajuste del catálogo; el núcleo guarda y restaura los valores originales.
#[tauri::command]
async fn tweak_apply(id: String, enable: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || tweak_set(&id, enable, &data_dir()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn app_is_admin() -> bool {
    tauri::async_runtime::spawn_blocking(is_admin).await.unwrap_or(false)
}

/// Reinicia el Explorador para que se apliquen ajustes de la barra de tareas y el menú Inicio.
#[tauri::command]
fn restart_explorer() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/f", "/im", "explorer.exe"])
            .creation_flags(0x0800_0000)
            .status();
        std::process::Command::new("explorer.exe").spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
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

#[derive(Serialize)]
struct ScheduleState {
    config: Schedule,
    /// La tarea existe en el Programador de tareas de Windows.
    task: bool,
    exe: String,
    last: Option<LastRun>,
}

fn exe_path() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[tauri::command]
async fn schedule_state() -> Result<ScheduleState, String> {
    tauri::async_runtime::spawn_blocking(|| ScheduleState {
        config: schedule_load(&data_dir()),
        task: schedule_task_exists(),
        exe: exe_path(),
        last: schedule_last_run(&data_dir()),
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn schedule_apply(config: Schedule) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || schedule_save(&config, &exe_path(), &data_dir()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn schedule_now(config: Schedule) -> Result<LastRun, String> {
    config.validate()?;
    tauri::async_runtime::spawn_blocking(move || schedule_run_now(&config, &data_dir(), &quarantine_dir(), now_secs()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn updates_scan() -> Result<Vec<Update>, String> {
    tauri::async_runtime::spawn_blocking(updates_list).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn updates_apply(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || update_apply(&id)).await.map_err(|e| e.to_string())?
}

fn main() {
    // Lanzado por el Programador de tareas: sin ventana, hace la pasada y sale.
    if std::env::args().any(|a| a == "--auto") {
        run_scheduled(&data_dir(), &quarantine_dir(), now_secs());
        return;
    }
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_rules, scan, clean, restore, quarantine, purge, open_quarantine, disk_roots, analyze, space_quarantine, dup_folders, dup_scan, dup_quarantine, old_scan, reveal, startup_items, startup_set, restore_state, restore_create, restore_open, restart_as_admin, debloat_list, debloat_apply, tweaks_state, tweak_apply, services_state, service_apply, app_is_admin, restart_explorer, programs_list, programs_icons, program_uninstall, leftovers_find, leftovers_remove, registry_restore, updates_scan, updates_apply, schedule_state, schedule_apply, schedule_now])
        .run(tauri::generate_context!())
        .expect("error al iniciar WinCleaner");
}
