use anyhow::{anyhow, Context, Result};
use bitshit_auto_loader::ComponentRegistry;
use inquire::{Confirm, Select, Text};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ORDER: [&str; 4] = ["kernel", "drivers", "engine", "cli"];
const MIN_FREE_GIB: u64 = 12;
const INSTALL_PATH_FILE: &str = ".bitshit-install-path";

#[derive(Clone, Copy)]
enum MenuAction {
    Status,
    SetInstallPath,
    InstallUpdate,
    SetupWizard,
    BuildRelease,
    BuildDebug,
    StartCli,
    Clean,
    Quit,
}

impl std::fmt::Display for MenuAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Status => "Status der Komponenten anzeigen",
            Self::SetInstallPath => "Installationspfad waehlen",
            Self::InstallUpdate => "Komponenten installieren / aktualisieren",
            Self::SetupWizard => "Installationsassistent starten",
            Self::BuildRelease => "Gesamten Stack bauen (Release)",
            Self::BuildDebug => "Gesamten Stack bauen (Debug)",
            Self::StartCli => "BitShit CLI starten",
            Self::Clean => "Build-Artefakte bereinigen",
            Self::Quit => "Beenden",
        };
        write!(f, "{label}")
    }
}

fn run_checked(command: &mut Command, label: &str) -> Result<()> {
    println!("\n[Manager] {label}");
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Konnte Prozess nicht starten: {label}"))?;
    if !status.success() {
        return Err(anyhow!("{label} fehlgeschlagen (Exitcode: {:?})", status.code()));
    }
    Ok(())
}

fn install_path_config(repo_root: &Path) -> PathBuf {
    repo_root.join(INSTALL_PATH_FILE)
}

fn resolve_install_path(repo_root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value.trim());
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}

fn load_install_path(repo_root: &Path) -> PathBuf {
    if let Some(value) = std::env::var_os("BITSHIT_INSTALL_ROOT") {
        return resolve_install_path(repo_root, &value.to_string_lossy());
    }

    let config = install_path_config(repo_root);
    if let Ok(value) = std::fs::read_to_string(config) {
        let value = value.trim();
        if !value.is_empty() {
            return resolve_install_path(repo_root, value);
        }
    }

    repo_root.join("components")
}

fn save_install_path(repo_root: &Path, install_root: &Path) -> Result<()> {
    std::fs::write(install_path_config(repo_root), install_root.to_string_lossy().as_bytes())?;
    Ok(())
}

fn choose_install_path(repo_root: &Path, current: &Path) -> Result<PathBuf> {
    let entered = Text::new("Installationspfad:")
        .with_default(&current.to_string_lossy())
        .with_help_message("Absoluter Pfad oder relativ zum Auto-Loader-Verzeichnis")
        .prompt()?;

    let selected = resolve_install_path(repo_root, &entered);
    std::fs::create_dir_all(&selected)
        .with_context(|| format!("Installationspfad konnte nicht erstellt werden: {}", selected.display()))?;
    save_install_path(repo_root, &selected)?;
    println!("[Manager] Installationspfad gespeichert: {}", selected.display());
    Ok(selected)
}

fn shared_target(install_root: &Path) -> PathBuf {
    install_root.join(".build").join("cargo-target")
}

fn ensure_disk_space(install_root: &Path) -> Result<()> {
    std::fs::create_dir_all(install_root)?;
    let free = fs2::available_space(install_root)
        .with_context(|| format!("Freier Speicher konnte fuer {} nicht ermittelt werden", install_root.display()))?;
    let free_gib = free as f64 / 1024.0 / 1024.0 / 1024.0;
    println!("[Manager] Freier Speicher am Installationsziel: {free_gib:.2} GiB");
    if free < MIN_FREE_GIB * 1024 * 1024 * 1024 {
        return Err(anyhow!(
            "Zu wenig Speicher: {free_gib:.2} GiB frei. Fuer den kompletten Rust/LLVM/Wasmtime-Build werden mindestens {MIN_FREE_GIB} GiB verlangt. Waehle einen anderen Installationspfad oder bereinige Build-Artefakte."
        ));
    }
    Ok(())
}

fn configure_cargo(command: &mut Command, install_root: &Path) {
    command
        .arg("--jobs")
        .arg("2")
        .env("CARGO_TARGET_DIR", shared_target(install_root))
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0");
}

fn component_status(registry: &ComponentRegistry) {
    println!("\nInstallationspfad: {}", registry.install_root.display());
    println!("+------------+----------------+----------+");
    println!("| Komponente | Status         | Revision |");
    println!("+------------+----------------+----------+");
    for name in ORDER {
        let dir = registry.component_dir(name);
        let installed = dir.join(".git").is_dir() && dir.join("Cargo.toml").is_file();
        let revision = if installed {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .current_dir(&dir)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "unbekannt".to_string())
        } else {
            "-".to_string()
        };
        println!("| {:<10} | {:<14} | {:<8} |", name, if installed { "installiert" } else { "fehlt" }, revision);
    }
    println!("+------------+----------------+----------+");
    if let Ok(free) = fs2::available_space(&registry.install_root) {
        println!("Freier Speicher: {:.2} GiB", free as f64 / 1024.0 / 1024.0 / 1024.0);
    }
    println!("Gemeinsames Build-Ziel: {}", shared_target(&registry.install_root).display());
}

fn build_stack(registry: &ComponentRegistry, release: bool) -> Result<()> {
    ensure_disk_space(&registry.install_root)?;
    std::fs::create_dir_all(shared_target(&registry.install_root))?;

    for name in ORDER {
        let dir = registry.component_dir(name);
        if !dir.join("Cargo.toml").is_file() {
            return Err(anyhow!("Komponente '{name}' fehlt. Zuerst installieren oder aktualisieren."));
        }
        let mut command = Command::new("cargo");
        command.arg("build");
        if release {
            command.arg("--release");
        }
        configure_cargo(&mut command, &registry.install_root);
        command.current_dir(&dir);
        run_checked(&mut command, &format!("Baue {name} ({})", if release { "Release" } else { "Debug" }))?;
    }
    println!("\n[Manager] Stack erfolgreich gebaut.");
    Ok(())
}

fn start_cli(registry: &ComponentRegistry) -> Result<()> {
    let cli_dir = registry.component_dir("cli");
    if !cli_dir.join("Cargo.toml").is_file() {
        return Err(anyhow!("CLI ist nicht installiert."));
    }
    let mut command = Command::new("cargo");
    command.args(["run", "--release"]);
    configure_cargo(&mut command, &registry.install_root);
    command.current_dir(cli_dir);
    run_checked(&mut command, "Starte BitShit CLI")
}

fn terminal_setup_wizard(
    repo_root: &Path,
    manifest_path: &Path,
    current_install_root: &Path,
    runtime: &tokio::runtime::Runtime,
) -> Result<PathBuf> {
    println!("\n+--------------------------------------------------+");
    println!("|        BitShit Installationsassistent            |");
    println!("+--------------------------------------------------+");
    println!("Plattform: {} / {}", std::env::consts::OS, std::env::consts::ARCH);

    let install_root = choose_install_path(repo_root, current_install_root)?;
    ensure_disk_space(&install_root)?;
    let registry = ComponentRegistry::load(manifest_path, install_root.clone())?;

    if Confirm::new("Komponenten installieren oder aktualisieren?")
        .with_default(true)
        .prompt()?
    {
        let dirs = runtime.block_on(registry.ensure_all())?;
        for dir in dirs {
            println!("[OK] {}", dir.display());
        }
    }

    let build = Select::new("Build-Profil:", vec!["Release", "Debug", "Nicht bauen"]).prompt()?;
    match build {
        "Release" => build_stack(&registry, true)?,
        "Debug" => build_stack(&registry, false)?,
        _ => {}
    }

    if Confirm::new("BitShit CLI anschliessend starten?")
        .with_default(false)
        .prompt()?
    {
        start_cli(&registry)?;
    }

    Ok(install_root)
}

fn clean_stack(registry: &ComponentRegistry) -> Result<()> {
    if !Confirm::new("Wirklich alle Cargo-Build-Artefakte loeschen?")
        .with_default(false)
        .prompt()?
    {
        return Ok(());
    }

    let target = shared_target(&registry.install_root);
    if target.exists() {
        println!("[Manager] Entferne {}", target.display());
        std::fs::remove_dir_all(&target)?;
    }

    for name in ORDER {
        let target = registry.component_dir(name).join("target");
        if target.exists() {
            println!("[Manager] Entferne {}", target.display());
            std::fs::remove_dir_all(target)?;
        }
    }
    println!("[Manager] Build-Artefakte bereinigt.");
    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = repo_root.join("package.json");
    let mut install_root = load_install_path(&repo_root);
    std::fs::create_dir_all(&install_root)?;
    let runtime = tokio::runtime::Runtime::new()?;

    println!("\n+==================================================+");
    println!("|          BitShit Component Manager               |");
    println!("+==================================================+");
    println!("Terminal-Modus: Linux, Windows CMD und PowerShell");

    loop {
        let registry = ComponentRegistry::load(&manifest_path, install_root.clone())?;
        println!("\nInstallationspfad: {}", install_root.display());

        let action = Select::new(
            "Aktion auswaehlen:",
            vec![
                MenuAction::Status,
                MenuAction::SetInstallPath,
                MenuAction::InstallUpdate,
                MenuAction::SetupWizard,
                MenuAction::BuildRelease,
                MenuAction::BuildDebug,
                MenuAction::StartCli,
                MenuAction::Clean,
                MenuAction::Quit,
            ],
        )
        .with_help_message("Pfeiltasten bewegen | Enter auswaehlen | Esc beendet")
        .prompt()
        .unwrap_or(MenuAction::Quit);

        let result = match action {
            MenuAction::Status => {
                component_status(&registry);
                Ok(())
            }
            MenuAction::SetInstallPath => {
                install_root = choose_install_path(&repo_root, &install_root)?;
                Ok(())
            }
            MenuAction::InstallUpdate => runtime.block_on(async {
                ensure_disk_space(&registry.install_root)?;
                let dirs = registry.ensure_all().await?;
                println!("\n[Manager] Komponenten bereit:");
                for dir in dirs {
                    println!("  {}", dir.display());
                }
                Ok(())
            }),
            MenuAction::SetupWizard => {
                install_root = terminal_setup_wizard(
                    &repo_root,
                    &manifest_path,
                    &install_root,
                    &runtime,
                )?;
                Ok(())
            }
            MenuAction::BuildRelease => build_stack(&registry, true),
            MenuAction::BuildDebug => build_stack(&registry, false),
            MenuAction::StartCli => start_cli(&registry),
            MenuAction::Clean => clean_stack(&registry),
            MenuAction::Quit => break,
        };
        if let Err(error) = result {
            eprintln!("\n[Manager] FEHLER: {error:#}");
        }
    }

    println!("[Manager] Beendet.");
    Ok(())
}
