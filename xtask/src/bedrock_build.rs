//! `bedrock-build`: the fork's game as the launcher imports it (docs/bedrock-release.md).
//!
//! Builds `horizon-official` with the release profile, as the release workflow does: the game
//! with Horizon's own parts (friends, presence, official servers), which play on the account the
//! launcher signed in and hands to the game (`--session-stdin`). The program is copied as
//! `release/horizon-bedrock/horizon-client.exe`: the launcher's "Import a game build" takes a
//! program by that name (or an archive holding it) and plays it on an installed Nightly's game
//! files. Nothing starts it but the launcher: no script goes with it.

use std::path::Path;
use std::process::Command;

pub fn run() -> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no parent folder")?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(repo)
        .args([
            "build",
            "--release",
            "--locked",
            "-p",
            "horizon-official",
            "--bin",
            "horizon-official",
        ])
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build: {status}"));
    }
    let exe = std::env::consts::EXE_SUFFIX;
    let built = repo
        .join("target/release")
        .join(format!("horizon-official{exe}"));
    let out = repo.join("release/horizon-bedrock");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let to = out.join(format!("horizon-client{exe}"));
    std::fs::copy(&built, &to).map_err(|e| format!("{}: {e}", built.display()))?;
    println!("{}", to.display());
    Ok(())
}
