//! `bedrock-play [dev|fast|release]`: the fork's game built with a profile and started by the
//! official Horizon launcher, signed in as its account (docs/bedrock-release.md).
//!
//! 1. `cargo build --profile <profile> -p horizon-official`: `dev` (quickest to build, slow to
//!    play), `fast` (the default: optimized, quick to link) or `release` (what is shipped).
//! 2. The program, named `horizon-client`, is imported in the official launcher
//!    (`horizon-launcher import-build`), which selects it; the build this task imported before
//!    is removed, so the launcher's version list keeps one.
//! 3. `horizon-launcher play --version <it>` starts it with the account signed in there: the fork never signs
//!    anyone in itself.
//!
//! The launcher: `HORIZON_LAUNCHER_EXE`, else its per-user install
//! (`%LOCALAPPDATA%\Programs\Horizon Launcher\horizon-launcher.exe`). The instance:
//! `HORIZON_BEDROCK_INSTANCE`, else the one the launcher plays. The game files the build plays
//! on: `HORIZON_BEDROCK_BASE` (an installed release or Nightly), else the launcher's choice.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(profile: &str) -> Result<(), String> {
    let dir = match profile {
        "dev" => "debug",
        "fast" | "release" => profile,
        other => return Err(format!("unknown profile {other}: dev, fast or release")),
    };
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no parent folder")?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(repo)
        .args(["build", "--profile", profile, "-p", "horizon-official"])
        .args(["--bin", "horizon-official"])
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build: {status}"));
    }
    let exe = std::env::consts::EXE_SUFFIX;
    let built = repo
        .join("target")
        .join(dir)
        .join(format!("horizon-official{exe}"));
    let out = repo.join("release").join(format!("play-{profile}"));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let program = out.join(format!("horizon-client{exe}"));
    std::fs::copy(&built, &program).map_err(|e| format!("{}: {e}", built.display()))?;

    let launcher = launcher_exe()?;
    let mut import = Command::new(&launcher);
    import.arg("--json").arg("import-build").arg(&program).arg("--trust-unsigned");
    if let Some(base) = set("HORIZON_BEDROCK_BASE") {
        import.args(["--version", &base]);
    }
    let output = import
        .output()
        .map_err(|e| format!("{}: {e}", launcher.display()))?;
    if !output.status.success() {
        return Err(format!(
            "import-build: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let tag = imported_tag(&String::from_utf8_lossy(&output.stdout));
    let last = repo.join("release/play-last-import.txt");
    if let (Some(tag), Ok(before)) = (&tag, std::fs::read_to_string(&last)) {
        let before = before.trim();
        if !before.is_empty() && before != tag {
            // Best effort: one removed by hand in the launcher is gone already.
            let _ = Command::new(&launcher).args(["remove", before]).status();
        }
    }
    if let Some(tag) = &tag {
        std::fs::write(&last, tag).map_err(|e| format!("{}: {e}", last.display()))?;
        println!("imported {tag} ({profile})");
    }
    let mut play = Command::new(&launcher);
    play.arg("play");
    if let Some(tag) = &tag {
        play.args(["--version", tag]);
    }
    if let Some(instance) = set("HORIZON_BEDROCK_INSTANCE") {
        play.args(["--instance", &instance]);
    }
    let status = play
        .status()
        .map_err(|e| format!("{}: {e}", launcher.display()))?;
    if !status.success() {
        return Err(format!("play: {status}"));
    }
    Ok(())
}

fn set(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn launcher_exe() -> Result<PathBuf, String> {
    if let Some(path) = set("HORIZON_LAUNCHER_EXE") {
        return Ok(PathBuf::from(path));
    }
    let local = std::env::var_os("LOCALAPPDATA").ok_or(
        "set HORIZON_LAUNCHER_EXE to the official Horizon launcher (horizon-launcher.exe)",
    )?;
    let path = PathBuf::from(local).join("Programs/Horizon Launcher/horizon-launcher.exe");
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no Horizon launcher at {}: install it from playhorizon.dev, or set HORIZON_LAUNCHER_EXE",
            path.display()
        ))
    }
}

/// The version `import-build --json` made: `{"imported": "<tag>", ...}`.
fn imported_tag(json: &str) -> Option<String> {
    let at = json.find("\"imported\"")?;
    let rest = &json[at + "\"imported\"".len()..];
    let start = rest.find('"')? + 1;
    let end = start + rest[start..].find('"')?;
    Some(rest[start..end].to_owned())
}
