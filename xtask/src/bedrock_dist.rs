//! `bedrock-dist`: everything a test of the fork needs, in `release/` (docs/bedrock-release.md).
//!
//! 1. The game (`bedrock-build`): `release/horizon-bedrock/horizon-client.exe`, which the
//!    official Horizon launcher imports ("Import a game build") and starts with the account
//!    signed in there: the fork never signs anyone in itself.
//! 2. `release/horizon-bedrock.zip`: that program and the folder's `GUIDE.md`, the archive the
//!    launcher also imports and a tester downloads.
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
    let zip = repo.join("release/horizon-bedrock.zip");
    let exe = format!("horizon-client{}", std::env::consts::EXE_SUFFIX);
    let mut files = vec![exe];
    if folder.join("GUIDE.md").is_file() {
        files.push("GUIDE.md".to_owned());
    }
    if zip.exists() {
        std::fs::remove_file(&zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    }
    // bsdtar (`tar` on Windows 10 and later, and macOS) writes a zip from the extension.
    let status = Command::new("tar")
        .current_dir(&folder)
        .arg("-a")
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
