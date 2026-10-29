//! Monitor del equipo: CPU, RAM, discos, red, procesos, GPU y temperaturas en vivo, más las
//! especificaciones. Solo lectura; no toca nada del sistema.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Components, Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System};

/// Cuántos programas se listan en cada ranking.
const TOP: usize = 4;

// Cada cuánto se repiten las lecturas caras. CPU, RAM, red y procesos se leen en cada muestra;
// lo demás cambia despacio y cuesta mucho más (WMI, PowerShell, programas externos).
/// Sensores de temperatura por WMI: conexión nueva cada vez.
const SENSORS_EVERY: Duration = Duration::from_secs(10);
/// Zonas térmicas: arranca un PowerShell entero.
const ZONES_EVERY: Duration = Duration::from_secs(20);
/// `nvidia-smi`: un programa aparte en cada lectura.
const GPU_EVERY: Duration = Duration::from_secs(6);
/// Espacio libre de las unidades.
const DISKS_EVERY: Duration = Duration::from_secs(10);
/// Si algo no existe en este equipo (sin sensores, sin NVIDIA) se vuelve a probar muy de vez en cuando.
const MISSING_EVERY: Duration = Duration::from_secs(300);

/// Solo lo que se muestra: CPU y memoria. Sin E/S de disco ni ruta del ejecutable, que en Windows
/// cuestan llamadas extra por cada proceso en cada muestra.
fn proc_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing().with_cpu().with_memory()
}

/// ¿Toca repetir una lectura hecha en `at`? Si no dio nada, se espera más.
fn due(at: Option<Instant>, every: Duration, empty: bool) -> bool {
    at.is_none_or(|t| t.elapsed() >= if empty { MISSING_EVERY } else { every })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PhysDisk {
    pub name: String,
    pub media: String,
    pub health: String,
    pub size: u64,
    /// Temperatura y desgaste (% de vida usada) del contador de fiabilidad. Windows solo los da
    /// en algunos discos y a veces solo como administrador: si no hay dato, `None`.
    pub temp: Option<f32>,
    pub wear: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Specs {
    pub os: String,
    pub host: String,
    pub cpu: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub ram_total: u64,
    pub gpus: Vec<String>,
    pub drives: Vec<PhysDisk>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskUse {
    pub mount: String,
    pub name: String,
    pub total: u64,
    pub free: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Temp {
    pub label: String,
    pub celsius: f32,
}

/// Un programa (varios procesos con el mismo nombre se suman).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Hog {
    pub name: String,
    pub cpu: f32,
    pub mem: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GpuLive {
    pub name: String,
    pub util: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub temp: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Live {
    pub cpu: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disks: Vec<DiskUse>,
    pub temps: Vec<Temp>,
    pub uptime: u64,
    /// Bytes por segundo, sumando todas las conexiones reales.
    pub net_down: u64,
    pub net_up: u64,
    pub top_cpu: Vec<Hog>,
    pub top_mem: Vec<Hog>,
    pub gpu: Vec<GpuLive>,
}

/// Lecturas caras (WMI, PowerShell, nvidia-smi). Se refrescan en un hilo aparte y la muestra usa
/// lo último que haya: así un PowerShell lento o colgado nunca deja el monitor sin datos.
#[derive(Default)]
struct Slow {
    sensors: Vec<Temp>,
    sensors_at: Option<Instant>,
    zones: Vec<Temp>,
    zones_at: Option<Instant>,
    gpu: Vec<GpuLive>,
    gpu_at: Option<Instant>,
    /// Hay un hilo refrescando; no se lanza otro.
    running: bool,
}

impl Slow {
    fn anything_due(&self) -> bool {
        due(self.sensors_at, SENSORS_EVERY, self.sensors.is_empty())
            || due(self.zones_at, ZONES_EVERY, self.zones.is_empty())
            || due(self.gpu_at, GPU_EVERY, self.gpu.is_empty())
    }
}

/// Refresca lo que toque. Corre en su propio hilo; cada lectura se guarda en cuanto termina.
fn refresh_slow(slow: &Mutex<Slow>) {
    let (want_sensors, want_zones, want_gpu) = match slow.lock() {
        Ok(s) => (
            due(s.sensors_at, SENSORS_EVERY, s.sensors.is_empty()),
            due(s.zones_at, ZONES_EVERY, s.zones.is_empty()),
            due(s.gpu_at, GPU_EVERY, s.gpu.is_empty()),
        ),
        Err(_) => return,
    };
    if want_gpu {
        let gpu = std::panic::catch_unwind(imp::nvidia).unwrap_or_default();
        if let Ok(mut s) = slow.lock() {
            s.gpu = gpu;
            s.gpu_at = Some(Instant::now());
        }
    }
    if want_sensors {
        let sensors = read_temps();
        if let Ok(mut s) = slow.lock() {
            s.sensors = sensors;
            s.sensors_at = Some(Instant::now());
        }
    }
    if want_zones {
        let zones = std::panic::catch_unwind(imp::zone_temps).unwrap_or_default();
        if let Ok(mut s) = slow.lock() {
            s.zones = zones;
            s.zones_at = Some(Instant::now());
        }
    }
}

/// Guarda el estado entre muestras: la CPU y la red se miden entre dos lecturas seguidas.
pub struct Monitor {
    sys: System,
    nets: Networks,
    last: Instant,
    slow: Arc<Mutex<Slow>>,
    disks: Vec<DiskUse>,
    disks_at: Option<Instant>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, proc_kind());
        Monitor {
            sys,
            nets: Networks::new_with_refreshed_list(),
            last: Instant::now(),
            slow: Arc::new(Mutex::new(Slow::default())),
            disks: Vec::new(),
            disks_at: None,
        }
    }

    pub fn specs(&mut self) -> Specs {
        self.sys.refresh_cpu_all();
        let cpu = self.sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
        Specs {
            os: [System::name(), System::os_version()].into_iter().flatten().collect::<Vec<_>>().join(" "),
            host: System::host_name().unwrap_or_default(),
            cpu,
            physical_cores: self.sys.physical_core_count().unwrap_or(0),
            logical_cores: self.sys.cpus().len(),
            ram_total: self.sys.total_memory(),
            gpus: imp::gpus(),
            // La salud de los discos va aparte (`drives`): puede tardar varios segundos.
            drives: Vec::new(),
        }
    }

    /// Discos físicos con su estado de salud. Lento (PowerShell y contadores de fiabilidad): se pide
    /// una vez y por separado para no retrasar el resto del panel.
    pub fn drives() -> Vec<PhysDisk> {
        std::panic::catch_unwind(imp::drives).unwrap_or_default()
    }

    pub fn sample(&mut self) -> Live {
        let secs = self.last.elapsed().as_secs_f64().max(0.2);
        self.last = Instant::now();
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, proc_kind());
        self.nets.refresh(true);
        let (down, up) = net_totals(&self.nets);
        let cores = self.sys.cpus().len().max(1) as f32;
        let (top_cpu, top_mem) = rankings(&self.sys, cores);
        if due(self.disks_at, DISKS_EVERY, false) {
            self.disks = disk_list(&Disks::new_with_refreshed_list());
            self.disks_at = Some(Instant::now());
        }
        let (gpu, mut temps) = self.slow_values();
        for g in &gpu {
            if let Some(t) = g.temp {
                let label = format!("GPU {}", g.name);
                if !temps.iter().any(|x| x.label == label) {
                    temps.push(Temp { label, celsius: t });
                }
            }
        }
        Live {
            cpu: self.sys.global_cpu_usage(),
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            disks: self.disks.clone(),
            temps,
            uptime: System::uptime(),
            net_down: (down as f64 / secs) as u64,
            net_up: (up as f64 / secs) as u64,
            top_cpu,
            top_mem,
            gpu,
        }
    }

    /// GPU y temperaturas con lo último leído (sensores del sistema y, además, zonas térmicas de Windows).
    /// Si toca refrescar, lanza un hilo aparte y no lo espera: la muestra nunca se queda bloqueada.
    fn slow_values(&mut self) -> (Vec<GpuLive>, Vec<Temp>) {
        let Ok(mut s) = self.slow.lock() else { return (Vec::new(), Vec::new()) };
        if !s.running && s.anything_due() {
            s.running = true;
            let slow = Arc::clone(&self.slow);
            let started = std::thread::Builder::new().name("monitor-lento".into()).spawn(move || {
                refresh_slow(&slow);
                if let Ok(mut s) = slow.lock() {
                    s.running = false;
                }
            });
            if started.is_err() {
                s.running = false;
            }
        }
        let mut temps = s.sensors.clone();
        for t in &s.zones {
            if !temps.iter().any(|o| o.label == t.label) {
                temps.push(t.clone());
            }
        }
        (s.gpu.clone(), temps)
    }
}

/// Bytes recibidos y enviados desde la lectura anterior, sin contar el bucle local.
fn net_totals(nets: &Networks) -> (u64, u64) {
    nets.iter()
        .filter(|(name, _)| !name.to_lowercase().contains("loopback"))
        .fold((0, 0), |(d, u), (_, n)| (d + n.received(), u + n.transmitted()))
}

/// Agrupa los procesos por nombre y devuelve los que más CPU y más RAM usan.
fn rankings(sys: &System, cores: f32) -> (Vec<Hog>, Vec<Hog>) {
    let mut by_name: HashMap<String, Hog> = HashMap::new();
    for p in sys.processes().values() {
        let name = p.name().to_string_lossy().into_owned();
        if name.is_empty() || p.pid().as_u32() == 0 || name.eq_ignore_ascii_case("system idle process") {
            continue;
        }
        let h = by_name.entry(name.clone()).or_insert(Hog { name, cpu: 0.0, mem: 0 });
        // sysinfo da el uso respecto a un núcleo; se normaliza al total de la CPU.
        h.cpu += p.cpu_usage() / cores;
        h.mem += p.memory();
    }
    top_hogs(by_name.into_values().collect())
}

fn top_hogs(all: Vec<Hog>) -> (Vec<Hog>, Vec<Hog>) {
    let mut cpu = all.clone();
    cpu.retain(|h| h.cpu >= 0.5);
    cpu.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then_with(|| a.name.cmp(&b.name)));
    cpu.truncate(TOP);
    let mut mem = all;
    mem.sort_by(|a, b| b.mem.cmp(&a.mem).then_with(|| a.name.cmp(&b.name)));
    mem.truncate(TOP);
    (cpu, mem)
}

fn disk_list(disks: &Disks) -> Vec<DiskUse> {
    let mut out: Vec<DiskUse> = Vec::new();
    for d in disks.list() {
        let mount = d.mount_point().display().to_string();
        // Una unidad puede aparecer dos veces (montada en varias rutas); se queda con una.
        if d.total_space() == 0 || out.iter().any(|x| x.mount == mount) {
            continue;
        }
        out.push(DiskUse { mount, name: d.name().to_string_lossy().into_owned(), total: d.total_space(), free: d.available_space() });
    }
    out
}

/// Los sensores se leen con una consulta nueva cada vez (en Windows usa WMI, ligado al hilo que la abre),
/// y un fallo del sensor nunca debe tirar la aplicación: sin lectura, lista vacía.
fn read_temps() -> Vec<Temp> {
    std::panic::catch_unwind(|| temp_list(&Components::new_with_refreshed_list())).unwrap_or_default()
}

fn temp_list(comps: &Components) -> Vec<Temp> {
    comps
        .list()
        .iter()
        .filter_map(|c| c.temperature().map(|t| Temp { label: c.label().to_string(), celsius: t }))
        // Lecturas imposibles (sensor sin conectar) fuera.
        .filter(|t| t.celsius.is_finite() && t.celsius > 0.0 && t.celsius < 150.0)
        .collect()
}

#[cfg(windows)]
mod imp {
    use super::{GpuLive, PhysDisk, Temp};
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// Lecturas en vivo (zonas térmicas, nvidia-smi).
    const LIVE_LIMIT: Duration = Duration::from_secs(10);
    /// Consultas de una vez (tarjetas gráficas, discos): los contadores de fiabilidad pueden tardar.
    const ONCE_LIMIT: Duration = Duration::from_secs(30);

    /// Ejecuta un programa y devuelve su salida. Si no termina a tiempo se cierra y no hay dato:
    /// un PowerShell o un driver colgado nunca deja el monitor esperando para siempre.
    fn run(program: &str, args: &[&str], limit: Duration) -> Option<String> {
        let mut child = Command::new(program)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut stdout = child.stdout.take()?;
        let reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf);
            buf
        });
        let end = Instant::now() + limit;
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(50)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        let out = reader.join().ok()?;
        status.filter(|s| s.success()).map(|_| String::from_utf8_lossy(&out).into_owned())
    }

    fn powershell(script: &str, limit: Duration) -> Option<String> {
        run("powershell", &["-NoProfile", "-NonInteractive", "-Command", script], limit)
    }

    /// Zonas térmicas ACPI del contador de rendimiento (no pide administrador).
    pub fn zone_temps() -> Vec<Temp> {
        let script = "Get-CimInstance Win32_PerfFormattedData_Counters_ThermalZoneInformation | ForEach-Object { $_.Name + '|' + $_.Temperature }";
        powershell(script, LIVE_LIMIT).map(|t| super::parse_thermal_zones(&t)).unwrap_or_default()
    }

    pub fn nvidia() -> Vec<GpuLive> {
        let args = ["--query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total", "--format=csv,noheader,nounits"];
        run("nvidia-smi", &args, LIVE_LIMIT).map(|t| super::parse_nvidia(&t)).unwrap_or_default()
    }

    pub fn gpus() -> Vec<String> {
        powershell("Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }", ONCE_LIMIT).map(|t| super::parse_names(&t)).unwrap_or_default()
    }

    /// Discos físicos con el estado de salud que informa Windows (no pide administrador).
    pub fn drives() -> Vec<PhysDisk> {
        let script = "Get-PhysicalDisk | ForEach-Object { $r = $_ | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue; $_.FriendlyName + '|' + $_.MediaType + '|' + $_.HealthStatus + '|' + $_.Size + '|' + $r.Temperature + '|' + $r.Wear }";
        powershell(script, ONCE_LIMIT).map(|t| super::parse_drives(&t)).unwrap_or_default()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{GpuLive, PhysDisk, Temp};

    pub fn zone_temps() -> Vec<Temp> {
        Vec::new()
    }

    pub fn nvidia() -> Vec<GpuLive> {
        Vec::new()
    }

    pub fn gpus() -> Vec<String> {
        Vec::new()
    }

    pub fn drives() -> Vec<PhysDisk> {
        Vec::new()
    }
}

/// `nombre|kelvin` por línea → grados Celsius. Descarta lecturas imposibles (0 K, sensores sin dato).
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_thermal_zones(text: &str) -> Vec<Temp> {
    text.lines()
        .filter_map(|l| {
            let (name, kelvin) = l.trim().rsplit_once('|')?;
            let celsius = kelvin.trim().parse::<f32>().ok()? - 273.15;
            let zone = name.rsplit(['.', '\\']).next().unwrap_or(name).trim();
            (celsius > 0.0 && celsius < 150.0).then(|| Temp { label: format!("ACPI {zone}"), celsius })
        })
        .collect()
}

/// Salida CSV de `nvidia-smi`: `nombre, grados, uso %, MiB usados, MiB totales`. `[N/A]` pasa a vacío o a 0.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_nvidia(text: &str) -> Vec<GpuLive> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').map(str::trim).collect();
            if f.len() != 5 || f[0].is_empty() {
                return None;
            }
            let temp = f[1].parse::<f32>().ok().filter(|t| *t > 0.0 && *t < 150.0);
            let mib = |s: &str| s.parse::<u64>().unwrap_or(0) * 1024 * 1024;
            Some(GpuLive {
                name: f[0].trim_start_matches("NVIDIA ").trim().to_string(),
                util: f[2].parse::<f32>().unwrap_or(0.0).clamp(0.0, 100.0),
                mem_used: mib(f[3]),
                mem_total: mib(f[4]),
                temp,
            })
        })
        .collect()
}

/// `nombre|tipo|salud|bytes|grados|desgaste` por línea (Get-PhysicalDisk y su contador de fiabilidad).
/// Los dos últimos campos pueden venir vacíos o faltar; 0 °C también cuenta como «sin dato».
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_drives(text: &str) -> Vec<PhysDisk> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim().split('|').map(str::trim).collect();
            if !(f.len() == 4 || f.len() == 6) || f[0].is_empty() {
                return None;
            }
            let temp = f.get(4).and_then(|s| s.parse::<f32>().ok()).filter(|t| *t > 0.0 && *t < 150.0);
            let wear = f.get(5).and_then(|s| s.parse::<u8>().ok()).filter(|w| *w <= 100);
            Some(PhysDisk { name: f[0].to_string(), media: f[1].to_string(), health: f[2].to_string(), size: f[3].parse().unwrap_or(0), temp, wear })
        })
        .collect()
}

#[cfg_attr(not(windows), allow(dead_code))]
fn parse_names(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !out.iter().any(|x| x == l) {
            out.push(l.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_something_sane() {
        let mut m = Monitor::new();
        let s = m.specs();
        assert!(s.ram_total > 0 && s.logical_cores > 0);
        std::thread::sleep(Duration::from_millis(300));
        let l = m.sample();
        assert!(l.ram_used <= l.ram_total);
        assert!((0.0..=100.0).contains(&l.cpu));
        assert!(l.top_mem.len() <= TOP && l.top_cpu.len() <= TOP);
        assert!(l.top_mem.windows(2).all(|w| w[0].mem >= w[1].mem));
    }

    #[test]
    fn parses_thermal_zones() {
        let z = parse_thermal_zones("\\_TZ.TZ00|300\r\n\\_TZ.THRM|0\r\nbasura\r\n");
        assert_eq!(z.len(), 1);
        assert_eq!(z[0].label, "ACPI TZ00");
        assert!((z[0].celsius - 26.85).abs() < 0.1);
    }

    #[test]
    fn parses_nvidia_with_missing_values() {
        let g = parse_nvidia("NVIDIA GeForce RTX 3070, 54, 37, 1520, 8192\n");
        assert_eq!(g, vec![GpuLive { name: "GeForce RTX 3070".into(), util: 37.0, mem_used: 1520 * 1048576, mem_total: 8192 * 1048576, temp: Some(54.0) }]);
        let n = parse_nvidia("NVIDIA GeForce RTX 3070, [N/A], [N/A], [N/A], 8192\nlinea rota\n");
        assert_eq!(n.len(), 1);
        assert_eq!((n[0].temp, n[0].util, n[0].mem_used), (None, 0.0, 0));
    }

    #[test]
    fn parses_physical_disks() {
        let d = parse_drives("Samsung SSD 980|SSD|Healthy|500107862016|38|3\r\nvacío|HDD\r\nWD Blue|HDD|Warning|1000204886016||\r\nViejo|HDD|Healthy|1\r\n");
        assert_eq!(d.len(), 3);
        assert_eq!((d[0].media.as_str(), d[0].health.as_str(), d[0].size), ("SSD", "Healthy", 500107862016));
        assert_eq!((d[0].temp, d[0].wear), (Some(38.0), Some(3)));
        assert_eq!(d[1].health, "Warning");
        assert_eq!((d[1].temp, d[1].wear), (None, None));
        assert_eq!(d[2].temp, None);
    }

    #[test]
    fn rankings_group_sort_and_cut() {
        let hog = |n: &str, c: f32, m: u64| Hog { name: n.into(), cpu: c, mem: m };
        let all = vec![hog("a", 0.1, 5), hog("b", 30.0, 1), hog("c", 12.0, 9), hog("d", 2.0, 7), hog("e", 8.0, 3), hog("f", 1.0, 2)];
        let (cpu, mem) = top_hogs(all);
        assert_eq!(cpu.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(), ["b", "c", "e", "d"]);
        assert_eq!(mem.iter().map(|h| h.name.as_str()).collect::<Vec<_>>(), ["c", "d", "a", "e"]);
    }

    #[test]
    fn gpu_names_are_trimmed_and_deduplicated() {
        assert_eq!(parse_names("NVIDIA GeForce RTX 4060\r\n\r\nIntel(R) UHD\r\nNVIDIA GeForce RTX 4060\r\n"), vec!["NVIDIA GeForce RTX 4060", "Intel(R) UHD"]);
    }
}
