//! Monitor del equipo: CPU, RAM, discos y temperaturas en vivo, más las especificaciones.
//! Solo lectura; no toca nada del sistema.

use serde::Serialize;
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
    comps: Components,
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
        Monitor { sys, comps: Components::new_with_refreshed_list() }
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
        self.comps.refresh(true);
        let disks = Disks::new_with_refreshed_list();
        Live {
            cpu: self.sys.global_cpu_usage(),
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            disks: disk_list(&disks),
            temps: temp_list(&self.comps),
            uptime: System::uptime(),
        }
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
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
    pub fn gpus() -> Vec<String> {
        Vec::new()
    }
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
    fn gpu_names_are_trimmed_and_deduplicated() {
        assert_eq!(parse_names("NVIDIA GeForce RTX 4060\r\n\r\nIntel(R) UHD\r\nNVIDIA GeForce RTX 4060\r\n"), vec!["NVIDIA GeForce RTX 4060", "Intel(R) UHD"]);
    }
}
