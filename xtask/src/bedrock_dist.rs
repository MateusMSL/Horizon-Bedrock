//! `bedrock-dist`: everything a test of the fork needs, in `release/` (docs/bedrock-release.md).
//!
//! 1. The game (`bedrock-build`): `release/horizon-bedrock/horizon-client.exe`, which the
//!    official Horizon launcher imports ("Import a game build") and starts with the account
//!    signed in there: the fork never signs anyone in itself.
//! 2. `release/horizon-bedrock-windows.zip` (Windows) or `release/horizon-bedrock-linux.tar.gz`
//!    (Linux): that program and the system's guide as `GUIDE.md`, the archive the launcher also
//!    imports and a tester downloads.
//! 3. The launcher (`launcher-build`): `release/horizon-launcher/horizon-launcher.exe`, which
//!    never replaces itself with upstream's. It signs in only with the fork's own Azure app
//!    (`HORIZON_MSA_CLIENT_ID`; docs/forking.md forbids Horizon's).
//!
//! Nothing is published: a release is a separate, explicit step.

use std::path::Path;
use std::process::Command;

pub fn run() -> Result<(), String> {
    crate::bedrock_build::run()?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no parent folder")?;
    let folder = repo.join("release/horizon-bedrock");
    // One archive per system, named for it: a Windows build (`.exe`) is not a Linux program, and
    // the launcher looks for `horizon-client` there. A `.tar.gz` keeps the program executable.
    let (system, archive, pack) = if cfg!(windows) {
        ("windows", "horizon-bedrock-windows.zip", "-a")
    } else {
        ("linux", "horizon-bedrock-linux.tar.gz", "-z")
    };
    let zip = repo.join("release").join(archive);
    let exe = format!("horizon-client{}", std::env::consts::EXE_SUFFIX);
    // The system's guide (packaging/<system>/LEIA-ME.md) as `GUIDE.md`.
    let guide = repo.join("packaging").join(system).join("LEIA-ME.md");
    if guide.is_file() {
        std::fs::copy(&guide, folder.join("GUIDE.md"))
            .map_err(|e| format!("{}: {e}", guide.display()))?;
    }
    let mut files = vec![exe];
    if folder.join("GUIDE.md").is_file() {
        files.push("GUIDE.md".to_owned());
    }
    if zip.exists() {
        std::fs::remove_file(&zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    }
    // bsdtar (`tar` on Windows 10 and later, and macOS) writes a zip from the extension (`-a`);
    // GNU tar on Linux a gzipped tar (`-z`).
    let status = Command::new("tar")
        .current_dir(&folder)
        .arg(pack)
        .arg("-cf")
        .arg(&zip)
        .args(&files)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar: {status}"));
    }
    println!("{}", zip.display());
    crate::launcher_build::run()
}
