use anyhow::Result;
use bitshit_auto_loader::ComponentRegistry;
use std::path::PathBuf;

const PACKAGE_MANIFEST: &str = include_str!("../package.json");

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("package.json");
    let install_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("components");

    let registry = ComponentRegistry::load(&manifest_path, install_root)?;

    println!("[AutoLoader] bitshit Auto-Loader v{}", registry.manifest.version);
    println!("[AutoLoader] Components: {}", registry.manifest.components.len());

    let rt = tokio::runtime::Runtime::new()?;
    let dirs = rt.block_on(registry.ensure_all())?;

    for dir in &dirs {
        println!("[AutoLoader] Ready: {:?}", dir);
    }

    println!("[AutoLoader] All components installed. Use bitshit-cli to run.");
    Ok(())
}
