use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
    pub fn load(manifest_path: &std::path::Path, install_root: PathBuf) -> Result<Self> {
        let data = std::fs::read_to_string(manifest_path)?;
        let manifest: PackageManifest = serde_json::from_str(&data)?;
        Ok(Self { manifest, install_root })
    }

    pub fn component_dir(&self, name: &str) -> PathBuf {
        self.install_root.join(name)
    }

    pub fn is_installed(&self, name: &str) -> bool {
        self.component_dir(name).join("Cargo.toml").exists()
            || self.component_dir(name).join("package.json").exists()
    }

    pub fn ensure_component(&self, name: &str) -> Result<PathBuf> {
        let dir = self.component_dir(name);
        if dir.exists() {
            info!("[AutoLoader] Component '{}' already present at {:?}", name, dir);
            return Ok(dir);
        }

        let component = self.manifest.components.get(name).ok_or_else(|| {
            anyhow!("Component '{}' not found in package manifest", name)
        })?;

        info!("[AutoLoader] Cloning '{}' from {} ...", name, component.repo);
        std::process::Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--branch",
                "main",
                &component.repo,
                dir.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| anyhow!("git clone failed for '{}': {}", name, e))?;

        Ok(dir)
    }

    pub async fn ensure_all(&self) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        for name in self.manifest.components.keys() {
            dirs.push(self.ensure_component(name)?);
        }
        Ok(dirs)
    }
}
