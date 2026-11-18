//! The official game executable (docs/layers.md, "How a Horizon crate plugs in").
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: horizon_game::Allocator = horizon_game::ALLOCATOR;

/// The mod API level this game serves, for the launcher to read without running it; `main`
/// reads it, so the linker keeps it.
static MOD_API_LEVEL: [u8; horizon_game::MOD_API_LEVEL.len()] = horizon_game::MOD_API_LEVEL;

/// The game identity newer official launchers look for before they import a build (upstream's
/// `horizon_mod_package::game_identity::MARK`): without it they call the program "not a Horizon
/// game". Kept beside the API level, which older launchers read.
static GAME_IDENTITY: [u8; 24] = *b"horizon-game-executable;";

fn main() {
    std::hint::black_box(&MOD_API_LEVEL);
    std::hint::black_box(&GAME_IDENTITY);
    horizon_official::register();
    horizon_game::main();
}
