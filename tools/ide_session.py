#!/usr/bin/env python3
"""Get the IDE's run configurations ready to start the game as the installed launcher would.

Runs as the "before launch" step of the IntelliJ configurations "Horizon (debug|fast|release)",
after Cargo built the game:

- `.local/ide/resources` links to the prepared game files of the launcher's selected version
  (the launcher moves them between versions as it installs or imports builds, so they are
  looked up at each start);
- `.local/ide/session.json` is a fresh, read-once session of the account signed in to the
  launcher (`horizon-launcher account session`), which the game reads with `--session-file`
  and deletes. The game refuses one older than two minutes, so this runs after the build.

- a watcher, left running in the background for up to two minutes, moves the game the IDE
  starts next (`target/*/horizon-official`, also under the debugger) into a systemd user scope
  capped at `HORIZON_MEMORY_MAX` (default 8G, no swap): past it the game alone is killed
  instead of the whole machine freezing. `HORIZON_MEMORY_MAX=0` turns the cap off.

The launcher: `HORIZON_LAUNCHER_EXE`, else `horizon-launcher` on PATH, else
~/Downloads/horizon-launcher. The account: the first argument (name or UUID), else the one
selected in the launcher.
"""

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / ".local" / "ide"
PREPARED = Path(".local/horizon-client/26.3")
GAME = "horizon-official"
WATCH_SECONDS = 120


def size_bytes(text: str) -> int:
    text = text.strip().upper()
    units = {"K": 1 << 10, "M": 1 << 20, "G": 1 << 30, "T": 1 << 40}
    if text and text[-1] in units:
        return int(float(text[:-1]) * units[text[-1]])
    return int(text)


def game_pids() -> set[int]:
    """This repository's built games that are running (any profile)."""
    target = REPO / "target"
    found = set()
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            exe = Path(os.readlink(entry / "exe"))
        except OSError:
            continue
        if exe.name == GAME and target in exe.parents:
            found.add(int(entry.name))
    return found


def cap(pid: int, limit: int) -> None:
    """Moves `pid` (and what it starts later) into a new user scope with a memory cap."""
    unit = f"horizon-game-{pid}-{int(time.time())}.scope"
    subprocess.run(
        ["busctl", "--user", "call", "org.freedesktop.systemd1", "/org/freedesktop/systemd1",
         "org.freedesktop.systemd1.Manager", "StartTransientUnit", "ssa(sv)a(sa(sv))",
         unit, "fail", "3",
         "PIDs", "au", "1", str(pid),
         "MemoryMax", "t", str(limit),
         "MemorySwapMax", "t", "0",
         "0"],
        check=True, capture_output=True,
    )
    # systemd moves it in a job: wait for it, so the cap is in place when this returns.
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            if unit in Path(f"/proc/{pid}/cgroup").read_text():
                return
        except OSError:
            return
        time.sleep(0.02)


def watch(limit: int, before: set[int]) -> None:
    deadline = time.monotonic() + WATCH_SECONDS
    while time.monotonic() < deadline:
        new = game_pids() - before
        if new:
            for pid in new:
                cap(pid, limit)
            return
        time.sleep(0.05)


def start_watcher() -> None:
    limit = size_bytes(os.environ.get("HORIZON_MEMORY_MAX", "8G") or "0")
    if limit <= 0:
        return
    if shutil.which("busctl") is None:
        print("memory cap: busctl not found, the game runs without one")
        return
    subprocess.Popen(
        [sys.executable, __file__, "--watch", str(limit), ",".join(map(str, game_pids()))],
        start_new_session=True,
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    print(f"memory cap: {limit >> 20} MiB for the next game started")


def data_dir() -> Path:
    return Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")


def launcher_exe() -> Path:
    given = os.environ.get("HORIZON_LAUNCHER_EXE", "").strip()
    if given:
        return Path(given)
    on_path = shutil.which("horizon-launcher")
    if on_path:
        return Path(on_path)
    fallback = Path.home() / "Downloads/horizon-launcher"
    if fallback.is_file():
        return fallback
    sys.exit("no Horizon launcher found: set HORIZON_LAUNCHER_EXE")


def prepared_files(launcher_home: Path) -> Path:
    try:
        selected = json.loads((launcher_home / "settings.json").read_text()).get("selected")
    except (OSError, ValueError):
        selected = None
    versions = launcher_home / "versions"
    found = sorted(
        v for v in versions.iterdir() if (v / PREPARED / "assets").is_dir()
    ) if versions.is_dir() else []
    pick = next((v for v in found if v.name == selected), found[-1] if found else None)
    if pick is None:
        sys.exit("no prepared game files in any launcher version: play once from the launcher")
    print(f"resources: {pick / PREPARED}")
    return pick / PREPARED


def selected_account(launcher_home: Path) -> str:
    try:
        accounts = json.loads((launcher_home / "accounts.json").read_text())
    except (OSError, ValueError):
        sys.exit("no launcher accounts: sign in in the launcher first")
    who = accounts.get("selected")
    if not who:
        sys.exit("no account selected in the launcher")
    return who


def main() -> None:
    if len(sys.argv) > 1 and sys.argv[1] == "--watch":
        before = {int(p) for p in sys.argv[3].split(",") if p} if len(sys.argv) > 3 else set()
        watch(int(sys.argv[2]), before)
        return
    launcher_home = data_dir() / "horizon-launcher"
    OUT.mkdir(parents=True, exist_ok=True)

    link = OUT / "resources"
    target = prepared_files(launcher_home)
    if link.is_symlink() or link.exists():
        link.unlink()
    link.symlink_to(target, target_is_directory=True)

    who = sys.argv[1] if len(sys.argv) > 1 else selected_account(launcher_home)
    env = dict(os.environ, HORIZON_LAUNCHER_NO_SELF_UPDATE="1")
    run = subprocess.run(
        [str(launcher_exe()), "--json", "account", "session", who],
        capture_output=True, text=True, env=env,
    )
    try:
        answer = json.loads(run.stdout)
    except ValueError:
        sys.exit(f"launcher: {run.stdout}{run.stderr}".strip())
    if "session_file" not in answer:
        sys.exit(f"the launcher could not renew the session: {answer.get('authentication', answer)}")

    session = OUT / "session.json"
    # Same folder tree as the launcher's sessions/ on most machines; a move keeps it private (0600).
    shutil.move(answer["session_file"], session)
    os.chmod(session, 0o600)
    print(f"session: {session} (account {who})")
    start_watcher()


if __name__ == "__main__":
    main()
