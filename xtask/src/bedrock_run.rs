//! `bedrock-run [dev|fast|release]`: the game started directly (no launcher, no account), for
//! debugging, on whichever installed launcher version holds the prepared game files.
//!
//! The launcher moves the prepared files (`.local/horizon-client/26.3`) between versions as it
//! imports builds, so a path written into a run configuration stops working after an import.
//! This looks them up at each start: the selected version first, then any other.
//!
//! Folders: `HORIZON_BEDROCK_GAME_DIR` (default `%APPDATA%\horizon\instances\bedrock-v1`) and
//! `HORIZON_BEDROCK_HOME` (default `%APPDATA%\horizon`).

use std::path::{Path, PathBuf};
use std::process::Command;

const PREPARED: &str = ".local/horizon-client/26.3";

pub fn run(profile: &str) -> Result<(), String> {
    if !matches!(profile, "dev" | "fast" | "release") {
        return Err(format!("unknown profile {profile}: dev, fast or release"));
    }
    let appdata = PathBuf::from(std::env::var_os("APPDATA").ok_or("APPDATA is not set")?);
    let resources = prepared_files(&appdata.join("horizon-launcher"))?;
    let game_dir = set("HORIZON_BEDROCK_GAME_DIR").map_or_else(
        || appdata.join("horizon/instances/bedrock-v1"),
        PathBuf::from,
    );
    let home = set("HORIZON_BEDROCK_HOME").map_or_else(|| appdata.join("horizon"), PathBuf::from);
    std::fs::create_dir_all(&game_dir).map_err(|e| format!("{}: {e}", game_dir.display()))?;
    println!("resources: {}", resources.display());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no parent folder")?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(&game_dir)
        .arg("run")
        .arg("--manifest-path")
        .arg(repo.join("Cargo.toml"))
        .args(["--profile", profile, "-p", "horizon-client", "--"])
        .arg("--resources")
        .arg(&resources)
        .arg("--game-dir")
        .arg(&game_dir)
        .arg("--home")
        .arg(&home)
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err(format!("game: {status}"));
    }
    Ok(())
}

fn set(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// The prepared game files: the launcher's selected version's, else any installed version's.
fn prepared_files(launcher: &Path) -> Result<PathBuf, String> {
    let versions = launcher.join("versions");
    let selected = std::fs::read_to_string(launcher.join("settings.json"))
        .ok()
        .and_then(|s| {
            let at = s.find("\"selected\"")?;
            let rest = &s[at + "\"selected\"".len()..];
            let start = rest.find('"')? + 1;
            let end = start + rest[start..].find('"')?;
            Some(rest[start..end].to_owned())
        });
    let mut found: Vec<PathBuf> = std::fs::read_dir(&versions)
        .map_err(|e| format!("{}: {e}", versions.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|v| v.join(PREPARED).join("assets").is_dir())
        .collect();
    found.sort();
    let pick = found
        .iter()
        .find(|v| v.file_name().and_then(|n| n.to_str()) == selected.as_deref())
        .or_else(|| found.last())
        .ok_or(
            "no prepared game files in any launcher version: play a Nightly once in the launcher",
        )?;
    Ok(pick.join(PREPARED))
}
