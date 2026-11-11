//! Bedrock-format resource packs, read only, for a game whose rules stay Java's.
//!
//! This crate reads what a Bedrock pack *shows and plays*: its files, its textures' names, its
//! sounds, its language, and the Molang expressions that drive its models, animations and
//! render controllers. It has no behaviour-pack, script or gameplay half, and no type of the
//! game: the client wires the results in through the [`molang::Host`] trait and the
//! [`pack::Pack`] file view. A behaviour or script module in a manifest is reported by
//! [`pack::Pack::unsupported_modules`] and never read.
//!
//! * [`brarchive`](mod@brarchive): the `.brarchive` container Marketplace-style packs keep their JSON in.
//! * [`geometry`](mod@geometry): models, baked into the game's part trees.
//! * [`jsonc`](mod@jsonc): Bedrock's JSON, with comments and trailing commas.
//! * [`manifest`](mod@manifest): `manifest.json`, its modules and subpacks.
//! * [`pack`](mod@pack): a pack as one file tree: folder, `.mcpack` or `.mcaddon`; `__brarchive/` folders
//!   expanded; the chosen subpack laid over the base.
//! * [`lang`](mod@lang): `texts/*.lang`.
//! * [`settings`](mod@settings): the manifest's pack settings and their chosen values.
//! * [`molang`](mod@molang): the expression language: parser and evaluator.
//!
//! Written from Microsoft's public Molang documentation and from inspecting the file formats.
//! No Bedrock code or assets are part of this crate; tests build their own packs.

/// The query functions Microsoft's Molang documentation lists, one per line (a test in the
/// client checks that each has an answer).
pub const DOCUMENTED_QUERIES: &str = include_str!("documented_queries.txt");

pub mod animation;
pub mod audit;
pub mod brarchive;
pub mod bundle;
pub mod convert;
pub mod entity;
pub mod geometry;
pub mod jsonc;
pub mod lang;
pub mod manifest;
pub mod material;
pub mod molang;
pub mod pack;
pub mod particle;
pub mod settings;
pub mod sounds;
pub mod world_data;

use std::fmt;

/// What can go wrong reading a pack.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Zip(String),
    Json(String),
    /// A `.brarchive` that is not one.
    Archive(String),
    /// A `manifest.json` that is not usable.
    Manifest(String),
    /// A path the pack does not hold.
    Missing(String),
    /// A pack whose files are encrypted (Marketplace content): its key is not ours to have.
    Encrypted(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Zip(e) => write!(f, "zip: {e}"),
            Error::Json(e) => write!(f, "json: {e}"),
            Error::Archive(e) => write!(f, "brarchive: {e}"),
            Error::Manifest(e) => write!(f, "manifest: {e}"),
            Error::Missing(p) => write!(f, "not in the pack: {p}"),
            Error::Encrypted(p) => write!(f, "encrypted pack ({p}): it cannot be read"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Self {
        Error::Zip(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
