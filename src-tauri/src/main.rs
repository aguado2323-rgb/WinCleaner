#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use wincleaner_core::{builtin_rules, clean_rule, scan_rule, CleanReport, Rule, ScanResult};

#[tauri::command]
fn list_rules() -> Vec<Rule> {
    builtin_rules()
}

#[tauri::command]
fn scan(ids: Vec<String>) -> Vec<ScanResult> {
    builtin_rules().iter().filter(|r| ids.contains(&r.id)).map(scan_rule).collect()
}

#[tauri::command]
fn clean(ids: Vec<String>, dry_run: bool) -> Vec<CleanReport> {
    builtin_rules().iter().filter(|r| ids.contains(&r.id)).map(|r| clean_rule(r, dry_run)).collect()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_rules, scan, clean])
        .run(tauri::generate_context!())
        .expect("error al iniciar WinCleaner");
}
