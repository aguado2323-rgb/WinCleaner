//! Monitor del equipo: CPU, RAM, discos, red, procesos, GPU y temperaturas en vivo, más las
//! especificaciones. Solo lectura; no toca nada del sistema.

use serde::Serialize;
use std::collections::HashMap;
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

/// Guarda el estado entre muestras: la CPU y la red se miden entre dos lecturas seguidas.
pub struct Monitor {
    sys: System,
    nets: Networks,
    last: Instant,
    // Lecturas caras: se guardan y se repiten cada pocos segundos, no en cada muestra.
    sensors: Vec<Temp>,
    sensors_at: Option<Instant>,
    zones: Vec<Temp>,
    zones_at: Option<Instant>,
    gpu: Vec<GpuLive>,
    gpu_at: Option<Instant>,
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
            sensors: Vec::new(),
            sensors_at: None,
            zones: Vec::new(),
            zones_at: None,
            gpu: Vec::new(),
            gpu_at: None,
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
            drives: imp::drives(),
        }
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
        let gpu = self.gpu();
        let mut temps = self.temps();
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

    /// Sensores del propio sistema y, si no hay o además, las zonas térmicas de Windows.
    /// Cuando no se encontró nada se vuelve a intentar solo cada pocos minutos.
    fn temps(&mut self) -> Vec<Temp> {
        if due(self.sensors_at, SENSORS_EVERY, self.sensors.is_empty()) {
            self.sensors = read_temps();
            self.sensors_at = Some(Instant::now());
        }
        let mut out = self.sensors.clone();
        if due(self.zones_at, ZONES_EVERY, self.zones.is_empty()) {
            self.zones = std::panic::catch_unwind(imp::zone_temps).unwrap_or_default();
            self.zones_at = Some(Instant::now());
        }
        for t in &self.zones {
            if !out.iter().any(|o| o.label == t.label) {
                out.push(t.clone());
            }
        }
        out
    }

    fn gpu(&mut self) -> Vec<GpuLive> {
        if due(self.gpu_at, GPU_EVERY, self.gpu.is_empty()) {
            self.gpu = std::panic::catch_unwind(imp::nvidia).unwrap_or_default();
            self.gpu_at = Some(Instant::now());
        }
        self.gpu.clone()
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

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn run(program: &str, args: &[&str]) -> Option<String> {
        let out = Command::new(program).args(args).creation_flags(CREATE_NO_WINDOW).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn powershell(script: &str) -> Option<String> {
        run("powershell", &["-NoProfile", "-NonInteractive", "-Command", script])
    }

    /// Zonas térmicas ACPI del contador de rendimiento (no pide administrador).
    pub fn zone_temps() -> Vec<Temp> {
        let script = "Get-CimInstance Win32_PerfFormattedData_Counters_ThermalZoneInformation | ForEach-Object { $_.Name + '|' + $_.Temperature }";
        powershell(script).map(|t| super::parse_thermal_zones(&t)).unwrap_or_default()
    }

    pub fn nvidia() -> Vec<GpuLive> {
        let args = ["--query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total", "--format=csv,noheader,nounits"];
        run("nvidia-smi", &args).map(|t| super::parse_nvidia(&t)).unwrap_or_default()
    }

    pub fn gpus() -> Vec<String> {
        powershell("Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }").map(|t| super::parse_names(&t)).unwrap_or_default()
    }

    /// Discos físicos con el estado de salud que informa Windows (no pide administrador).
    pub fn drives() -> Vec<PhysDisk> {
        let script = "Get-PhysicalDisk | ForEach-Object { $_.FriendlyName + '|' + $_.MediaType + '|' + $_.HealthStatus + '|' + $_.Size }";
        powershell(script).map(|t| super::parse_drives(&t)).unwrap_or_default()
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

/// `nombre|tipo|salud|bytes` por línea (Get-PhysicalDisk).
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_drives(text: &str) -> Vec<PhysDisk> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim().split('|').map(str::trim).collect();
            if f.len() != 4 || f[0].is_empty() {
                return None;
            }
            Some(PhysDisk { name: f[0].to_string(), media: f[1].to_string(), health: f[2].to_string(), size: f[3].parse().unwrap_or(0) })
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
        let d = parse_drives("Samsung SSD 980|SSD|Healthy|500107862016\r\nvacío|HDD\r\nWD Blue|HDD|Warning|1000204886016\r\n");
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].media.as_str(), d[0].health.as_str(), d[0].size), ("SSD", "Healthy", 500107862016));
        assert_eq!(d[1].health, "Warning");
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
