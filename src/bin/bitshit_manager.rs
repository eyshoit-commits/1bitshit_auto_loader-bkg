use anyhow::{anyhow, Context, Result};
use bitshit_auto_loader::ComponentRegistry;
use inquire::{Confirm, Select};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ORDER: [&str; 4] = ["kernel", "drivers", "engine", "cli"];

#[derive(Clone, Copy)]
enum MenuAction {
    Status,
    InstallUpdate,
    BuildRelease,
    BuildDebug,
    StartCli,
    OpenGui,
    Clean,
    Quit,
}

impl std::fmt::Display for MenuAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Status => "Status der Komponenten anzeigen",
            Self::InstallUpdate => "Komponenten installieren / aktualisieren",
            Self::BuildRelease => "Gesamten Stack bauen (Release)",
            Self::BuildDebug => "Gesamten Stack bauen (Debug)",
            Self::StartCli => "BitShit CLI starten",
            Self::OpenGui => "Grafischen Installer starten",
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

fn component_status(registry: &ComponentRegistry) {
    println!("\nKomponentenstatus\n────────────────────────────────────────────");
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
        println!("{:<10} {:<14} {}", name, if installed { "installiert" } else { "fehlt" }, revision);
    }
    println!("────────────────────────────────────────────");
}

fn build_stack(registry: &ComponentRegistry, release: bool) -> Result<()> {
    for name in ORDER {
        let dir = registry.component_dir(name);
        if !dir.join("Cargo.toml").is_file() {
            return Err(anyhow!("Komponente '{name}' fehlt. Zuerst installieren oder aktualisieren."));
        }
        let mut command = Command::new("cargo");
        command.arg("build");
        if release { command.arg("--release"); }
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
    command.current_dir(cli_dir);
    run_checked(&mut command, "Starte BitShit CLI")
}

fn open_gui(repo_root: &Path) -> Result<()> {
    let mut command = Command::new("cargo");
    command.args(["run", "--release", "--bin", "bitshit-installer"]);
    command.current_dir(repo_root);
    run_checked(&mut command, "Starte grafischen Installer")
}

fn clean_stack(registry: &ComponentRegistry) -> Result<()> {
    if !Confirm::new("Wirklich alle Cargo-Build-Artefakte löschen?").with_default(false).prompt()? {
        return Ok(());
    }
    for name in ORDER {
        let dir = registry.component_dir(name);
        if dir.join("Cargo.toml").is_file() {
            let mut command = Command::new("cargo");
            command.arg("clean").current_dir(&dir);
            run_checked(&mut command, &format!("Bereinige {name}"))?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let registry = ComponentRegistry::load(&repo_root.join("package.json"), repo_root.join("components"))?;
    let runtime = tokio::runtime::Runtime::new()?;

    println!("\n╔══════════════════════════════════════════════╗");
    println!("║          BitShit Component Manager           ║");
    println!("╚══════════════════════════════════════════════╝");
    println!("Version {} · {} Komponenten", registry.manifest.version, registry.manifest.components.len());

    loop {
        let action = Select::new("Aktion auswählen:", vec![
            MenuAction::Status,
            MenuAction::InstallUpdate,
            MenuAction::BuildRelease,
            MenuAction::BuildDebug,
            MenuAction::StartCli,
            MenuAction::OpenGui,
            MenuAction::Clean,
            MenuAction::Quit,
        ])
        .with_help_message("Pfeiltasten bewegen · Enter auswählen · Esc beendet")
        .prompt()
        .unwrap_or(MenuAction::Quit);

        let result = match action {
            MenuAction::Status => { component_status(&registry); Ok(()) }
            MenuAction::InstallUpdate => runtime.block_on(async {
                let dirs = registry.ensure_all().await?;
                println!("\n[Manager] Komponenten bereit:");
                for dir in dirs { println!("  {}", dir.display()); }
                Ok(())
            }),
            MenuAction::BuildRelease => build_stack(&registry, true),
            MenuAction::BuildDebug => build_stack(&registry, false),
            MenuAction::StartCli => start_cli(&registry),
            MenuAction::OpenGui => open_gui(&repo_root),
            MenuAction::Clean => clean_stack(&registry),
            MenuAction::Quit => break,
        };
        if let Err(error) = result { eprintln!("\n[Manager] FEHLER: {error:#}"); }
    }

    println!("[Manager] Beendet.");
    Ok(())
}
