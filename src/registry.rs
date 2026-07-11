use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentManifest {
    pub name: String,
    pub repo: String,
    pub version: String,
    pub min_supported: Option<String>,
    pub manifest_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub components: std::collections::HashMap<String, ComponentManifest>,
}

#[derive(Debug, Clone)]
pub struct ComponentRegistry {
    pub manifest: PackageManifest,
    pub install_root: PathBuf,
}

impl ComponentRegistry {
    pub fn load(manifest_path: &Path, install_root: PathBuf) -> Result<Self> {
        let data = std::fs::read_to_string(manifest_path)
            .map_err(|error| anyhow!("failed to read {}: {}", manifest_path.display(), error))?;
        let manifest: PackageManifest = serde_json::from_str(&data)
            .map_err(|error| anyhow!("invalid package manifest {}: {}", manifest_path.display(), error))?;
        Ok(Self { manifest, install_root })
    }

    pub fn component_dir(&self, name: &str) -> PathBuf {
        self.install_root.join(name)
    }

    pub fn is_installed(&self, name: &str) -> bool {
        let dir = self.component_dir(name);
        dir.join(".git").exists()
            && (dir.join("Cargo.toml").exists() || dir.join("package.json").exists())
    }

    fn run_git(dir: Option<&Path>, args: &[&str], label: &str) -> Result<()> {
        let mut command = Command::new("git");
        if let Some(dir) = dir {
            command.current_dir(dir);
        }
        let status = command
            .args(args)
            .status()
            .map_err(|error| anyhow!("{}: failed to start git: {}", label, error))?;
        if !status.success() {
            return Err(anyhow!("{}: git exited with {}", label, status));
        }
        Ok(())
    }

    pub fn ensure_component(&self, name: &str) -> Result<PathBuf> {
        let component = self
            .manifest
            .components
            .get(name)
            .ok_or_else(|| anyhow!("Component '{}' not found in package manifest", name))?;
        let dir = self.component_dir(name);

        if self.is_installed(name) {
            info!("[AutoLoader] Updating '{}' at {:?}", name, dir);
            Self::run_git(Some(&dir), &["fetch", "origin", "--prune"], "component fetch")?;
            Self::run_git(
                Some(&dir),
                &["reset", "--hard", &format!("origin/{}", component.version)],
                "component reset",
            )?;
            return Ok(dir);
        }

        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|error| {
                anyhow!("failed to remove incomplete component directory {}: {}", dir.display(), error)
            })?;
        }
        std::fs::create_dir_all(&self.install_root)?;

        info!("[AutoLoader] Cloning '{}' from {} ...", name, component.repo);
        let dir_string = dir.to_string_lossy().into_owned();
        Self::run_git(
            None,
            &[
                "clone",
                "--depth",
                "1",
                "--branch",
                &component.version,
                &component.repo,
                &dir_string,
            ],
            "component clone",
        )?;

        if !self.is_installed(name) {
            return Err(anyhow!(
                "component '{}' was cloned but contains no usable Cargo.toml or package.json",
                name
            ));
        }

        Ok(dir)
    }

    pub async fn ensure_all(&self) -> Result<Vec<PathBuf>> {
        let order = ["kernel", "drivers", "engine", "cli"];
        let mut dirs = Vec::new();

        for name in order {
            if self.manifest.components.contains_key(name) {
                dirs.push(self.ensure_component(name)?);
            }
        }

        for name in self.manifest.components.keys() {
            if !order.contains(&name.as_str()) {
                dirs.push(self.ensure_component(name)?);
            }
        }

        Ok(dirs)
    }
}
