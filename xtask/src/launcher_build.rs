//! `launcher-build`: the fork's launcher, `horizon-launcher.exe` (docs/bedrock-release.md).
//!
//! Builds `launcher/` (its own workspace) with the release profile, as `.github/workflows/launcher.yml`
//! does, and copies the program to `release/horizon-launcher/`. Built in:
//! - `HORIZON_LAUNCHER_NO_SELF_UPDATE`: always, so the fork's launcher never replaces itself with
//!   upstream's release.
//! - `HORIZON_MSA_CLIENT_ID`: from the environment, the fork's own Azure app (docs/forking.md:
//!   never Horizon's). Unset, the launcher says sign-in is not configured.
//! - `HORIZON_CURSEFORGE_API_KEY`, `HORIZON_LAUNCHER_VERSION`, `HORIZON_LAUNCHER_TAG`: passed on
//!   when set.

use std::path::Path;
use std::process::Command;

pub fn run() -> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no parent folder")?;
    let launcher = repo.join("launcher");
    let set = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    if set("HORIZON_MSA_CLIENT_ID").is_none() {
        eprintln!(
            "warning: HORIZON_MSA_CLIENT_ID is not set: this launcher will say Microsoft sign-in is not configured"
        );
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut build = Command::new(cargo);
    build
        .current_dir(&launcher)
        .args(["build", "--release", "--locked"])
        .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1");
    // Empty values would be built in as empty strings: drop them instead.
    for name in [
        "HORIZON_MSA_CLIENT_ID",
        "HORIZON_CURSEFORGE_API_KEY",
        "HORIZON_LAUNCHER_VERSION",
        "HORIZON_LAUNCHER_TAG",
    ] {
        if set(name).is_none() {
            build.env_remove(name);
        }
    }
    let status = build.status().map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build: {status}"));
    }
    let exe = std::env::consts::EXE_SUFFIX;
    let built = launcher
        .join("target/release")
        .join(format!("horizon-launcher{exe}"));
    let out = repo.join("release/horizon-launcher");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let to = out.join(format!("horizon-launcher{exe}"));
    std::fs::copy(&built, &to).map_err(|e| format!("{}: {e}", built.display()))?;
    println!("{}", to.display());
    Ok(())
}
