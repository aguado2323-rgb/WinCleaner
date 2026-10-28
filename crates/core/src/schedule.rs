//! Limpieza programada. Windows lanza `WinCleaner.exe --auto` con el Programador de tareas: solo se
//! limpian las reglas marcadas como seguras y todo va a la cuarentena (restaurable). Si la persona lo
//! activa, también se vacían los lotes de la cuarentena con más de N días, que es lo que libera espacio de verdad.

#![cfg_attr(not(windows), allow(dead_code))]

use crate::clean::{clean_rule, list_batches, purge_batch, scan_rule, Mode};
use crate::rules::{builtin_rules, Rule};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoMode {
    /// Limpia lo seguro y lo manda a la cuarentena.
    Clean,
    /// Solo avisa de cuánto espacio se puede recuperar.
    Notify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Schedule {
    pub enabled: bool,
    pub freq: Freq,
    /// Hora local, `HH:MM`.
    pub time: String,
    pub mode: AutoMode,
    /// Días que se guarda la cuarentena antes de vaciarse sola. 0 = nunca (por defecto).
    pub purge_days: u32,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule { enabled: false, freq: Freq::Weekly, time: "12:00".into(), mode: AutoMode::Clean, purge_days: 0 }
    }
}

impl Schedule {
    /// La configuración viene de la interfaz: se valida antes de usarla.
    pub fn validate(&self) -> Result<(), String> {
        let ok_time = self.time.len() == 5
            && self.time.as_bytes()[2] == b':'
            && self.time[..2].parse::<u32>().is_ok_and(|h| h < 24)
            && self.time[3..].parse::<u32>().is_ok_and(|m| m < 60);
        if !ok_time {
            return Err("hora no válida".into());
        }
        if ![0, 7, 30, 90].contains(&self.purge_days) {
            return Err("plazo de la cuarentena no válido".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LastRun {
    /// Segundos desde 1970.
    pub when: u64,
    pub mode: Option<AutoMode>,
    pub files: u64,
    pub bytes: u64,
    /// Lotes de la cuarentena vaciados por haber superado el plazo.
    pub purged: u64,
}

const SCHEDULE_FILE: &str = "schedule.json";
const LAST_FILE: &str = "auto-last.json";
/// Por debajo de esto, el modo «avisar» no molesta.
const NOTIFY_MIN_BYTES: u64 = 500_000_000;

pub fn load(data_dir: &Path) -> Schedule {
    std::fs::read(data_dir.join(SCHEDULE_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Schedule>(&b).ok())
        .filter(|s| s.validate().is_ok())
        .unwrap_or_default()
}

pub fn save(data_dir: &Path, s: &Schedule) -> Result<(), String> {
    s.validate()?;
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    std::fs::write(data_dir.join(SCHEDULE_FILE), serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn last_run(data_dir: &Path) -> Option<LastRun> {
    std::fs::read(data_dir.join(LAST_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

/// Ejecuta una pasada: limpia (o solo mide) las reglas seguras y vacía lo caducado de la cuarentena.
pub fn run_with(rules: &[Rule], cfg: &Schedule, data_dir: &Path, quarantine_dir: &Path, now: u64) -> LastRun {
    let mut run = LastRun { when: now, mode: Some(cfg.mode), ..Default::default() };
    let safe = rules.iter().filter(|r| r.safe);
    match cfg.mode {
        AutoMode::Notify => {
            for r in safe {
                let s = scan_rule(r);
                run.files += s.files;
                run.bytes += s.bytes;
            }
        }
        AutoMode::Clean => {
            let mode = Mode::Quarantine { dir: quarantine_dir.to_path_buf(), batch: now.to_string() };
            for r in safe {
                let c = clean_rule(r, &mode);
                run.files += c.files;
                run.bytes += c.bytes;
            }
        }
    }
    if cfg.purge_days > 0 {
        let limit = now.saturating_sub(u64::from(cfg.purge_days) * 86_400);
        for b in list_batches(quarantine_dir) {
            // El nombre del lote es la hora a la que se hizo la limpieza.
            if b.batch.parse::<u64>().is_ok_and(|t| t <= limit) && purge_batch(quarantine_dir, &b.batch).is_ok() {
                run.purged += 1;
            }
        }
    }
    if cfg.mode == AutoMode::Clean {
        crate::history::record(
            data_dir,
            crate::history::Event { when: now, kind: crate::history::Kind::Auto, files: run.files, bytes: run.bytes },
        );
    }
    let _ = std::fs::create_dir_all(data_dir);
    if let Ok(bytes) = serde_json::to_vec_pretty(&run) {
        let _ = std::fs::write(data_dir.join(LAST_FILE), bytes);
    }
    run
}

pub fn run_now(cfg: &Schedule, data_dir: &Path, quarantine_dir: &Path, now: u64) -> LastRun {
    run_with(&builtin_rules(), cfg, data_dir, quarantine_dir, now)
}

/// Punto de entrada de `WinCleaner.exe --auto`: sin ventana, hace la pasada y avisa si procede.
pub fn run_scheduled(data_dir: &Path, quarantine_dir: &Path, now: u64) {
    let cfg = load(data_dir);
    if !cfg.enabled {
        return;
    }
    let run = run_now(&cfg, data_dir, quarantine_dir, now);
    let mb = |b: u64| format!("{:.0} MB", b as f64 / 1e6);
    match cfg.mode {
        AutoMode::Clean if run.bytes > 0 => {
            toast("WinCleaner", &format!("Limpieza automática: {} movidos a la cuarentena.", mb(run.bytes)));
        }
        AutoMode::Notify if run.bytes >= NOTIFY_MIN_BYTES => {
            toast("WinCleaner", &format!("Hay unos {} que se pueden limpiar. Abre WinCleaner para revisarlo.", mb(run.bytes)));
        }
        _ => {}
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Definición de la tarea para el Programador de Windows. Corre con los permisos normales de la persona
/// (sin administrador) y, si el equipo estaba apagado a la hora prevista, se ejecuta al volver a encenderse.
pub fn task_xml(cfg: &Schedule, exe: &str) -> String {
    let trigger = match cfg.freq {
        Freq::Daily => "<ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>".to_string(),
        Freq::Weekly => {
            "<ScheduleByWeek><DaysOfWeek><Monday/></DaysOfWeek><WeeksInterval>1</WeeksInterval></ScheduleByWeek>".to_string()
        }
        Freq::Monthly => "<ScheduleByMonth><DaysOfMonth><Day>1</Day></DaysOfMonth><Months><January/><February/><March/><April/><May/><June/><July/><August/><September/><October/><November/><December/></Months></ScheduleByMonth>".to_string(),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>WinCleaner: limpieza automática (a la cuarentena)</Description></RegistrationInfo>
  <Triggers><CalendarTrigger><StartBoundary>2026-01-01T{time}:00</StartBoundary><Enabled>true</Enabled>{trigger}</CalendarTrigger></Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><StartWhenAvailable>true</StartWhenAvailable><ExecutionTimeLimit>PT1H</ExecutionTimeLimit></Settings>
  <Actions Context="Author"><Exec><Command>{exe}</Command><Arguments>--auto</Arguments></Exec></Actions>
</Task>
"#,
        time = cfg.time,
        exe = xml_escape(exe),
    )
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const TASK: &str = "WinCleaner Limpieza";

    fn schtasks(args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW).output()
    }

    pub fn task_exists() -> bool {
        schtasks(&["/Query", "/TN", TASK]).is_ok_and(|o| o.status.success())
    }

    /// Crea o actualiza la tarea (si está activada) o la quita (si no).
    pub fn apply(cfg: &Schedule, exe: &str, data_dir: &Path) -> Result<(), String> {
        if !cfg.enabled {
            if task_exists() {
                let out = schtasks(&["/Delete", "/TN", TASK, "/F"]).map_err(|e| e.to_string())?;
                if !out.status.success() {
                    return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
                }
            }
            return Ok(());
        }
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let file = data_dir.join("task.xml");
        // El Programador espera UTF-16 con marca de orden de bytes.
        let bytes: Vec<u8> = std::iter::once(0xFEFFu16).chain(task_xml(cfg, exe).encode_utf16()).flat_map(u16::to_le_bytes).collect();
        std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
        let out = schtasks(&["/Create", "/TN", TASK, "/XML", &file.to_string_lossy(), "/F"]).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&file);
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("Windows no pudo crear la tarea programada: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }

    /// Notificación de Windows. El texto va por variables de entorno y se añade como texto, no como código.
    pub fn toast(title: &str, body: &str) {
        let script = "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
            $x = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
            $n = $x.GetElementsByTagName('text'); \
            [void]$n.Item(0).AppendChild($x.CreateTextNode($env:WC_TITLE)); [void]$n.Item(1).AppendChild($x.CreateTextNode($env:WC_BODY)); \
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe').Show([Windows.UI.Notifications.ToastNotification]::new($x))";
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .env("WC_TITLE", title)
            .env("WC_BODY", body)
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn task_exists() -> bool {
        false
    }
    pub fn apply(_: &Schedule, _: &str, _: &Path) -> Result<(), String> {
        Err("solo disponible en Windows".into())
    }
    pub fn toast(_: &str, _: &str) {}
}

pub use imp::task_exists;
use imp::toast;

/// Guarda la configuración y crea, actualiza o quita la tarea programada para que coincida.
pub fn schedule_save(cfg: &Schedule, exe: &str, data_dir: &Path) -> Result<(), String> {
    cfg.validate()?;
    if exe.contains('"') || exe.contains('<') || exe.is_empty() {
        return Err("ruta del programa no válida".into());
    }
    imp::apply(cfg, exe, data_dir)?;
    save(data_dir, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn rule(var: &str, dir: &Path, safe: bool) -> Rule {
        std::env::set_var(var, dir);
        Rule {
            id: var.into(),
            name: var.into(),
            category: "t".into(),
            description: "t".into(),
            help: String::new(),
            paths: vec![format!("%{var}%")],
            safe,
        }
    }

    #[test]
    fn validation_rejects_bad_time_and_purge() {
        assert!(Schedule::default().validate().is_ok());
        for t in ["", "9:00", "24:00", "12:60", "ab:cd", "12-00", "12:000"] {
            assert!(Schedule { time: t.into(), ..Default::default() }.validate().is_err(), "{t}");
        }
        assert!(Schedule { purge_days: 5, ..Default::default() }.validate().is_err());
        assert!(Schedule { purge_days: 30, ..Default::default() }.validate().is_ok());
    }

    #[test]
    fn task_xml_runs_auto_with_normal_rights_and_escapes_the_path() {
        let cfg = Schedule { enabled: true, freq: Freq::Daily, time: "08:30".into(), ..Default::default() };
        let x = task_xml(&cfg, r"C:\Mis & Cosas\WinCleaner.exe");
        assert!(x.contains("T08:30:00") && x.contains("ScheduleByDay"));
        assert!(x.contains("<Command>C:\\Mis &amp; Cosas\\WinCleaner.exe</Command><Arguments>--auto</Arguments>"));
        assert!(x.contains("LeastPrivilege") && x.contains("StartWhenAvailable"));
        assert!(task_xml(&Schedule { freq: Freq::Weekly, ..cfg.clone() }, "a").contains("<Monday/>"));
        assert!(task_xml(&Schedule { freq: Freq::Monthly, ..cfg }, "a").contains("<Day>1</Day>"));
    }

    #[test]
    fn config_roundtrips_and_garbage_falls_back_to_default() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(load(t.path()), Schedule::default());
        let cfg = Schedule { enabled: true, freq: Freq::Monthly, time: "07:05".into(), mode: AutoMode::Notify, purge_days: 90 };
        save(t.path(), &cfg).unwrap();
        assert_eq!(load(t.path()), cfg);
        fs::write(t.path().join("schedule.json"), br#"{"enabled":true,"freq":"weekly","time":"99:99","mode":"clean","purge_days":0}"#).unwrap();
        assert_eq!(load(t.path()), Schedule::default());
        fs::write(t.path().join("schedule.json"), b"basura").unwrap();
        assert_eq!(load(t.path()), Schedule::default());
    }

    #[test]
    fn auto_run_cleans_only_safe_rules_into_quarantine_and_purges_old_batches() {
        let t = tempfile::tempdir().unwrap();
        let q = t.path().join("q");
        let seguro = t.path().join("seguro");
        let delicado = t.path().join("delicado");
        fs::create_dir_all(&seguro).unwrap();
        fs::create_dir_all(&delicado).unwrap();
        fs::write(seguro.join("a.tmp"), vec![0u8; 100]).unwrap();
        fs::write(delicado.join("b.dat"), vec![0u8; 50]).unwrap();
        let rules = [rule("WC_TEST_SAFE", &seguro, true), rule("WC_TEST_CARE", &delicado, false)];

        // Un lote antiguo (hora 1000) y uno reciente en la cuarentena.
        for (batch, name) in [("1000", "viejo.bin"), ("999999000", "reciente.bin")] {
            let d = q.join(batch).join("x");
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(name), b"zz").unwrap();
        }

        let now = 1_000_000_000;
        let notify = Schedule { enabled: true, mode: AutoMode::Notify, ..Default::default() };
        let r = run_with(&rules, &notify, t.path(), &q, now);
        assert_eq!((r.files, r.bytes, r.purged), (1, 100, 0));
        assert!(seguro.join("a.tmp").exists(), "avisar no toca nada");

        let clean = Schedule { enabled: true, mode: AutoMode::Clean, purge_days: 30, ..Default::default() };
        let r = run_with(&rules, &clean, t.path(), &q, now);
        assert_eq!((r.files, r.bytes, r.purged), (1, 100, 1));
        assert!(!seguro.join("a.tmp").exists() && delicado.join("b.dat").exists(), "solo las reglas seguras");
        assert!(q.join(now.to_string()).exists(), "lo limpiado va a la cuarentena");
        assert!(!q.join("1000").exists() && q.join("999999000").exists(), "solo se vacía lo caducado");
        assert_eq!(last_run(t.path()).unwrap().bytes, 100);
    }
}
