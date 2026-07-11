use anyhow::{anyhow, Context, Result};
use bitshit_auto_loader::ComponentRegistry;
use eframe::egui;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use sysinfo::System;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendChoice {
    Auto,
    Cpu,
    Cuda,
    BitNetCpu,
}

impl BackendChoice {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatisch erkennen",
            Self::Cpu => "CPU / GGUF",
            Self::Cuda => "NVIDIA CUDA",
            Self::BitNetCpu => "BitNet CPU (I2_S / ternär)",
        }
    }

    fn cargo_features(self) -> &'static [&'static str] {
        match self {
            Self::Cuda => &["cuda"],
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn label(self) -> &'static str {
        match self {
            Self::Debug => "Debug",
            Self::Release => "Release (empfohlen)",
        }
    }
}

#[derive(Debug, Clone)]
struct HardwareSummary {
    os: String,
    cpu: String,
    logical_cpus: usize,
    memory_gb: f64,
    nvidia_detected: bool,
}

impl HardwareSummary {
    fn detect() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let cpu = system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Unbekannte CPU".to_string());

        let nvidia_detected = Command::new(if cfg!(windows) { "nvidia-smi.exe" } else { "nvidia-smi" })
            .arg("--query-gpu=name")
            .arg("--format=csv,noheader")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        Self {
            os: std::env::consts::OS.to_string(),
            cpu,
            logical_cpus: system.cpus().len(),
            memory_gb: system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0,
            nvidia_detected,
        }
    }
}

enum WorkerMessage {
    Log(String),
    Finished(Result<PathBuf, String>),
}

struct InstallerApp {
    page: usize,
    install_root: String,
    backend: BackendChoice,
    profile: BuildProfile,
    install_kernel: bool,
    install_drivers: bool,
    install_engine: bool,
    install_cli: bool,
    launch_after_install: bool,
    hardware: HardwareSummary,
    running: bool,
    progress: f32,
    logs: Vec<String>,
    rx: Option<Receiver<WorkerMessage>>,
}

impl Default for InstallerApp {
    fn default() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let hardware = HardwareSummary::detect();
        let backend = if hardware.nvidia_detected {
            BackendChoice::Auto
        } else {
            BackendChoice::BitNetCpu
        };

        Self {
            page: 0,
            install_root: PathBuf::from(home)
                .join(".bitshit")
                .to_string_lossy()
                .to_string(),
            backend,
            profile: BuildProfile::Release,
            install_kernel: true,
            install_drivers: true,
            install_engine: true,
            install_cli: true,
            launch_after_install: true,
            hardware,
            running: false,
            progress: 0.0,
            logs: Vec::new(),
            rx: None,
        }
    }
}

impl InstallerApp {
    fn selected_components(&self) -> Vec<&'static str> {
        let mut selected = Vec::new();
        if self.install_kernel { selected.push("kernel"); }
        if self.install_drivers { selected.push("drivers"); }
        if self.install_engine { selected.push("engine"); }
        if self.install_cli { selected.push("cli"); }
        selected
    }

    fn start_install(&mut self) {
        if self.running {
            return;
        }

        let install_root = PathBuf::from(self.install_root.trim());
        let backend = self.backend;
        let profile = self.profile;
        let selected = self.selected_components();
        let launch_after_install = self.launch_after_install;
        let (tx, rx) = mpsc::channel();

        self.running = true;
        self.progress = 0.0;
        self.logs.clear();
        self.rx = Some(rx);

        thread::spawn(move || {
            let result = run_install(
                install_root,
                backend,
                profile,
                selected,
                launch_after_install,
                &tx,
            )
            .map_err(|error| format!("{error:#}"));
            let _ = tx.send(WorkerMessage::Finished(result));
        });
    }

    fn poll_worker(&mut self) {
        let Some(rx) = &self.rx else { return; };
        while let Ok(message) = rx.try_recv() {
            match message {
                WorkerMessage::Log(line) => {
                    self.logs.push(line);
                    self.progress = (self.logs.len() as f32 / 20.0).min(0.95);
                }
                WorkerMessage::Finished(result) => {
                    self.running = false;
                    self.progress = 1.0;
                    match result {
                        Ok(path) => self.logs.push(format!("Installation abgeschlossen: {}", path.display())),
                        Err(error) => self.logs.push(format!("FEHLER: {error}")),
                    }
                }
            }
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            if self.page > 0 && !self.running && ui.button("Zurück").clicked() {
                self.page -= 1;
            }
            ui.add_space(8.0);
            if self.page < 4 {
                if ui.button("Weiter").clicked() {
                    self.page += 1;
                }
            } else if ui
                .add_enabled(!self.running, egui::Button::new("BitShit installieren"))
                .clicked()
            {
                self.start_install();
            }
        });
    }
}

impl eframe::App for InstallerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
        if self.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("BitShit Setup");
            ui.label(format!("Schritt {} von 5", self.page + 1));
            ui.add(egui::ProgressBar::new((self.page as f32 + 1.0) / 5.0).show_percentage());
            ui.add_space(16.0);

            match self.page {
                0 => {
                    ui.heading("Willkommen");
                    ui.label("Dieser Assistent installiert Kernel, Treiber, Engine und CLI als zusammenhängenden Stack.");
                    ui.add_space(12.0);
                    ui.label("Installationsverzeichnis");
                    ui.text_edit_singleline(&mut self.install_root);
                    ui.checkbox(&mut self.launch_after_install, "BitShit nach erfolgreicher Installation starten");
                }
                1 => {
                    ui.heading("Hardware-Erkennung");
                    egui::Grid::new("hardware").striped(true).show(ui, |ui| {
                        ui.label("Betriebssystem"); ui.label(&self.hardware.os); ui.end_row();
                        ui.label("CPU"); ui.label(&self.hardware.cpu); ui.end_row();
                        ui.label("Logische Kerne"); ui.label(self.hardware.logical_cpus.to_string()); ui.end_row();
                        ui.label("Arbeitsspeicher"); ui.label(format!("{:.1} GB", self.hardware.memory_gb)); ui.end_row();
                        ui.label("NVIDIA"); ui.label(if self.hardware.nvidia_detected { "erkannt" } else { "nicht erkannt" }); ui.end_row();
                    });
                    if ui.button("Hardware neu prüfen").clicked() {
                        self.hardware = HardwareSummary::detect();
                    }
                }
                2 => {
                    ui.heading("Backend wählen");
                    for choice in [BackendChoice::Auto, BackendChoice::Cpu, BackendChoice::Cuda, BackendChoice::BitNetCpu] {
                        ui.radio_value(&mut self.backend, choice, choice.label());
                    }
                    ui.add_space(12.0);
                    match self.backend {
                        BackendChoice::Auto => { ui.label("Der Installer wählt anhand der Hardware den passenden Pfad."); }
                        BackendChoice::Cpu => { ui.label("Normale GGUF-Modelle über CPU und llama.cpp."); }
                        BackendChoice::Cuda => { ui.label("CUDA-Build. Benötigt NVIDIA-Treiber und ein unterstütztes Toolkit."); }
                        BackendChoice::BitNetCpu => { ui.label("BitNet/I2_S über den ternären CPU-Kernel. Kein CUDA erforderlich."); }
                    }
                }
                3 => {
                    ui.heading("Komponenten und Build");
                    ui.checkbox(&mut self.install_kernel, "Kernel");
                    ui.checkbox(&mut self.install_drivers, "Treiber / FFI");
                    ui.checkbox(&mut self.install_engine, "Engine");
                    ui.checkbox(&mut self.install_cli, "CLI / App");
                    ui.add_space(12.0);
                    ui.label("Build-Profil");
                    ui.radio_value(&mut self.profile, BuildProfile::Release, BuildProfile::Release.label());
                    ui.radio_value(&mut self.profile, BuildProfile::Debug, BuildProfile::Debug.label());
                }
                _ => {
                    ui.heading("Installation");
                    ui.label(format!("Ziel: {}", self.install_root));
                    ui.label(format!("Backend: {}", self.backend.label()));
                    ui.label(format!("Profil: {}", self.profile.label()));
                    ui.label(format!("Komponenten: {}", self.selected_components().join(", ")));
                    ui.add_space(12.0);
                    ui.add(egui::ProgressBar::new(self.progress).show_percentage());
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for line in &self.logs {
                            ui.monospace(line);
                        }
                    });
                }
            }

            self.footer(ui);
        });
    }
}

fn run_command(command: &mut Command, label: &str, tx: &Sender<WorkerMessage>) -> Result<()> {
    let _ = tx.send(WorkerMessage::Log(label.to_string()));
    let status = command.status().with_context(|| format!("{label}: Prozess konnte nicht gestartet werden"))?;
    if !status.success() {
        return Err(anyhow!("{label}: Exitcode {:?}", status.code()));
    }
    Ok(())
}

fn run_install(
    install_root: PathBuf,
    backend: BackendChoice,
    profile: BuildProfile,
    selected: Vec<&'static str>,
    launch_after_install: bool,
    tx: &Sender<WorkerMessage>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(&install_root)?;
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("package.json");
    let components_root = install_root.join("components");
    let registry = ComponentRegistry::load(&manifest_path, components_root.clone())?;

    let _ = tx.send(WorkerMessage::Log("Komponenten werden synchronisiert ...".to_string()));
    for component in ["kernel", "drivers", "engine", "cli"] {
        if selected.contains(&component) {
            registry.ensure_component(component)?;
            let _ = tx.send(WorkerMessage::Log(format!("{component}: Repository bereit")));
        }
    }

    for component in ["kernel", "drivers", "engine", "cli"] {
        if !selected.contains(&component) {
            continue;
        }
        let dir = registry.component_dir(component);
        let mut command = Command::new("cargo");
        command.arg("build");
        if profile == BuildProfile::Release {
            command.arg("--release");
        }
        for feature in backend.cargo_features() {
            command.arg("--features").arg(feature);
        }
        command.current_dir(&dir);
        run_command(&mut command, &format!("{component}: Build läuft"), tx)?;
        let _ = tx.send(WorkerMessage::Log(format!("{component}: Build abgeschlossen")));
    }

    if launch_after_install && selected.contains(&"cli") {
        let cli_dir = registry.component_dir("cli");
        let profile_dir = if profile == BuildProfile::Release { "release" } else { "debug" };
        let binary = cli_dir
            .join("target")
            .join(profile_dir)
            .join(if cfg!(windows) { "bitshit.exe" } else { "bitshit" });
        if binary.is_file() {
            let _ = tx.send(WorkerMessage::Log("BitShit wird gestartet ...".to_string()));
            Command::new(binary).spawn()?;
        } else {
            let _ = tx.send(WorkerMessage::Log("CLI-Binary wurde nach dem Build nicht am erwarteten Ort gefunden.".to_string()));
        }
    }

    Ok(install_root)
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 620.0])
            .with_min_inner_size([640.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "BitShit Installer",
        options,
        Box::new(|_cc| Ok(Box::<InstallerApp>::default())),
    )
}
