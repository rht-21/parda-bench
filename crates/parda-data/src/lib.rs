//! Identifier generators and validators, text templates, seeded dataset build, validation and stats.

pub mod build;
pub mod checksum;
pub mod ids;
pub mod io;
pub mod lexicon;
pub mod noise;
pub mod render;
pub mod stats;
pub mod template;
pub mod validate;

/// Deterministic, platform-independent RNG used for every generated value.
pub type DataRng = rand_chacha::ChaCha8Rng;

/// Version that `data build` writes by default; bump it whenever the generated samples change.
pub const DATASET_VERSION: &str = "v0.2.0";

/// Recorded in every sample so a dataset can be traced to the code that produced it.
pub const GENERATOR_VERSION: &str = concat!("parda-data/", env!("CARGO_PKG_VERSION"));
