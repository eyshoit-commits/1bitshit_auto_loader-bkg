use anyhow::{anyhow, Context, Result};
use bitshit_auto_loader::ComponentRegistry;
use inquire::{Confirm, Select};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ORDER: [&str; 4] = ["kernel", "drivers", "engine", "cli"];
const MIN_FREE_GIB: u64 = 12;

#[derive(Clone, Copy)]
enum MenuAction {
    Status,
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

fn shared_target(repo_root: &Path) -> PathBuf {
    repo_root.join("target").join("components")
}

fn ensure_disk_space(repo_root: &Path) -> Result<()> {
    let free = fs2::available_space(repo_root)
        .with_context(|| format!("Freier Speicher konnte fuer {} nicht ermittelt werden", repo_root.display()))?;
    let free_gib = free as f64 / 1024.0 / 1024.0 / 1024.0;
    println!("[Manager] Freier Speicher: {free_gib:.2} GiB");
    if free < MIN_FREE_GIB * 1024 * 1024 * 1024 {
        return Err(anyhow!(
            "Zu wenig Speicher: {free_gib:.2} GiB frei. Fuer den kompletten Rust/LLVM/Wasmtime-Build werden mindestens {MIN_FREE_GIB} GiB verlangt. Erst Build-Artefakte bereinigen oder Speicher freigeben."
        ));
    }
    Ok(())
}

fn configure_cargo(command: &mut Command, repo_root: &Path) {
    command
        .arg("--jobs")
        .arg("2")
        .env("CARGO_TARGET_DIR", shared_target(repo_root))
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0");
}

fn component_status(registry: &ComponentRegistry, repo_root: &Path) {
    println!("\n+------------+----------------+----------+");
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
    if let Ok(free) = fs2::available_space(repo_root) {
        println!("Freier Speicher: {:.2} GiB", free as f64 / 1024.0 / 1024.0 / 1024.0);
    }
    println!("Gemeinsames Target: {}", shared_target(repo_root).display());
}

fn build_stack(registry: &ComponentRegistry, repo_root: &Path, release: bool) -> Result<()> {
    ensure_disk_space(repo_root)?;
    std::fs::create_dir_all(shared_target(repo_root))?;

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
        configure_cargo(&mut command, repo_root);
        command.current_dir(&dir);
        run_checked(&mut command, &format!("Baue {name} ({})", if release { "Release" } else { "Debug" }))?;
    }
    println!("\n[Manager] Stack erfolgreich gebaut.");
    Ok(())
}

fn start_cli(registry: &ComponentRegistry, repo_root: &Path) -> Result<()> {
    let cli_dir = registry.component_dir("cli");
    if !cli_dir.join("Cargo.toml").is_file() {
        return Err(anyhow!("CLI ist nicht installiert."));
    }
    let mut command = Command::new("cargo");
    command.args(["run", "--release"]);
    configure_cargo(&mut command, repo_root);
    command.current_dir(cli_dir);
    run_checked(&mut command, "Starte BitShit CLI")
}

fn terminal_setup_wizard(
    registry: &ComponentRegistry,
    runtime: &tokio::runtime::Runtime,
    repo_root: &Path,
) -> Result<()> {
    println!("\n+--------------------------------------------------+");
    println!("|        BitShit Installationsassistent            |");
    println!("+--------------------------------------------------+");
    println!("Plattform: {} / {}", std::env::consts::OS, std::env::consts::ARCH);

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
        "Release" => build_stack(registry, repo_root, true)?,
        "Debug" => build_stack(registry, repo_root, false)?,
        _ => {}
    }

    if Confirm::new("BitShit CLI anschliessend starten?")
        .with_default(false)
        .prompt()?
    {
        start_cli(registry, repo_root)?;
    }
    Ok(())
}

fn clean_stack(registry: &ComponentRegistry, repo_root: &Path) -> Result<()> {
    if !Confirm::new("Wirklich alle Cargo-Build-Artefakte loeschen?")
        .with_default(false)
        .prompt()?
    {
        return Ok(());
    }

    let target = shared_target(repo_root);
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
    let registry = ComponentRegistry::load(&repo_root.join("package.json"), repo_root.join("components"))?;
    let runtime = tokio::runtime::Runtime::new()?;

    println!("\n+==================================================+");
    println!("|          BitShit Component Manager               |");
    println!("+==================================================+");
    println!("Version {} | {} Komponenten | {} / {}", registry.manifest.version, registry.manifest.components.len(), std::env::consts::OS, std::env::consts::ARCH);
    println!("Terminal-Modus: Linux, Windows CMD und PowerShell");

    loop {
        let action = Select::new(
            "Aktion auswaehlen:",
            vec![
                MenuAction::Status,
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
                component_status(&registry, &repo_root);
                Ok(())
            }
            MenuAction::InstallUpdate => runtime.block_on(async {
                let dirs = registry.ensure_all().await?;
                println!("\n[Manager] Komponenten bereit:");
                for dir in dirs {
                    println!("  {}", dir.display());
                }
                Ok(())
            }),
            MenuAction::SetupWizard => terminal_setup_wizard(&registry, &runtime, &repo_root),
            MenuAction::BuildRelease => build_stack(&registry, &repo_root, true),
            MenuAction::BuildDebug => build_stack(&registry, &repo_root, false),
            MenuAction::StartCli => start_cli(&registry, &repo_root),
            MenuAction::Clean => clean_stack(&registry, &repo_root),
            MenuAction::Quit => break,
        };
        if let Err(error) = result {
            eprintln!("\n[Manager] FEHLER: {error:#}");
        }
    }

    println!("[Manager] Beendet.");
    Ok(())
}
