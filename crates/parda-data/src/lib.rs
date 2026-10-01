//! Identifier generators and validators, text templates, seeded dataset build, validation and stats.

pub mod build;
pub mod checksum;
pub mod ids;
pub mod io;
pub mod lexicon;
pub mod render;
pub mod stats;
pub mod template;
pub mod validate;

/// Deterministic, platform-independent RNG used for every generated value.
pub type DataRng = rand_chacha::ChaCha8Rng;

/// Recorded in every sample so a dataset can be traced to the code that produced it.
pub const GENERATOR_VERSION: &str = concat!("parda-data/", env!("CARGO_PKG_VERSION"));
