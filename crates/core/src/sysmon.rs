//! Monitor del equipo: CPU, RAM, discos y temperaturas en vivo, más las especificaciones.
//! Solo lectura; no toca nada del sistema.

use serde::Serialize;
use std::time::{Duration, Instant};
use sysinfo::{Components, Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct Specs {
    pub os: String,
    pub host: String,
    pub cpu: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub ram_total: u64,
    pub gpus: Vec<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct Live {
    pub cpu: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disks: Vec<DiskUse>,
    pub temps: Vec<Temp>,
    pub uptime: u64,
}

/// Guarda el estado entre muestras: la CPU se mide entre dos lecturas seguidas.
pub struct Monitor {
    sys: System,
    // Lecturas lentas (PowerShell, nvidia-smi): se repiten cada pocos segundos, no en cada muestra.
    extra: Vec<Temp>,
    extra_at: Option<Instant>,
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
        Monitor { sys, extra: Vec::new(), extra_at: None }
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
        }
    }

    pub fn sample(&mut self) -> Live {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let disks = Disks::new_with_refreshed_list();
        Live {
            cpu: self.sys.global_cpu_usage(),
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            disks: disk_list(&disks),
            temps: self.temps(),
            uptime: System::uptime(),
        }
    }
}

impl Monitor {
    /// Sensores del propio sistema y, si no hay o además, las lecturas alternativas (zonas térmicas de
    /// Windows y tarjeta NVIDIA). Cuando no se encontró nada se vuelve a intentar solo cada minuto.
    fn temps(&mut self) -> Vec<Temp> {
        let mut out = read_temps();
        let wait = Duration::from_secs(if self.extra.is_empty() { 60 } else { 6 });
        if self.extra_at.is_none_or(|t| t.elapsed() >= wait) {
            self.extra = std::panic::catch_unwind(imp::extra_temps).unwrap_or_default();
            self.extra_at = Some(Instant::now());
        }
        for t in &self.extra {
            if !out.iter().any(|o| o.label == t.label) {
                out.push(t.clone());
            }
        }
        out
    }
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
    use super::Temp;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn run(program: &str, args: &[&str]) -> Option<String> {
        let out = Command::new(program).args(args).creation_flags(CREATE_NO_WINDOW).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Zonas térmicas ACPI del contador de rendimiento (no pide administrador) y temperatura de NVIDIA.
    pub fn extra_temps() -> Vec<Temp> {
        let mut out = Vec::new();
        let zones = "Get-CimInstance Win32_PerfFormattedData_Counters_ThermalZoneInformation | ForEach-Object { $_.Name + '|' + $_.Temperature }";
        if let Some(text) = run("powershell", &["-NoProfile", "-NonInteractive", "-Command", zones]) {
            out.extend(super::parse_thermal_zones(&text));
        }
        if let Some(text) = run("nvidia-smi", &["--query-gpu=name,temperature.gpu", "--format=csv,noheader,nounits"]) {
            out.extend(super::parse_nvidia(&text));
        }
        out
    }

    pub fn gpus() -> Vec<String> {
        let out = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match out {
            Ok(o) => super::parse_names(&String::from_utf8_lossy(&o.stdout)),
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Temp;

    pub fn gpus() -> Vec<String> {
        Vec::new()
    }

    pub fn extra_temps() -> Vec<Temp> {
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

/// `nombre, grados` por línea (salida CSV de `nvidia-smi`). `[N/A]` o texto raro se ignora.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_nvidia(text: &str) -> Vec<Temp> {
    text.lines()
        .filter_map(|l| {
            let (name, deg) = l.trim().rsplit_once(',')?;
            let celsius = deg.trim().parse::<f32>().ok()?;
            (celsius > 0.0 && celsius < 150.0).then(|| Temp { label: format!("GPU {}", name.trim().trim_start_matches("NVIDIA ").trim()), celsius })
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
        let l = m.sample();
        assert!(l.ram_used <= l.ram_total);
        assert!((0.0..=100.0).contains(&l.cpu));
    }

    #[test]
    fn parses_thermal_zones_and_nvidia() {
        let z = parse_thermal_zones("\\_TZ.TZ00|300\r\n\\_TZ.THRM|0\r\nbasura\r\n");
        assert_eq!(z.len(), 1);
        assert_eq!(z[0].label, "ACPI TZ00");
        assert!((z[0].celsius - 26.85).abs() < 0.1);
        let g = parse_nvidia("NVIDIA GeForce RTX 3070, 54\n");
        assert_eq!((g[0].label.as_str(), g[0].celsius), ("GPU GeForce RTX 3070", 54.0));
        assert!(parse_nvidia("NVIDIA GeForce RTX 3070, [N/A]\n").is_empty());
    }

    #[test]
    fn gpu_names_are_trimmed_and_deduplicated() {
        assert_eq!(parse_names("NVIDIA GeForce RTX 4060\r\n\r\nIntel(R) UHD\r\nNVIDIA GeForce RTX 4060\r\n"), vec!["NVIDIA GeForce RTX 4060", "Intel(R) UHD"]);
    }
}
