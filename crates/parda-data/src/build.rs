//! Seeded dataset build: every sample's RNG is derived from (seed, template id, index), so adding or editing one
//! template never changes the samples of another.

use std::collections::BTreeSet;

use parda_spec::sample::{Labels, Sample, Source};
use rand::SeedableRng;
use sha2::{Digest, Sha256};

use crate::render::render;
use crate::template::{Template, TemplateError, parse_file};
use crate::{DataRng, GENERATOR_VERSION};

const BUILTIN_TEMPLATE_FILES: [(&str, &str); 5] = [
    ("en.toml", include_str!("../templates/en.toml")),
    ("hi_latn.toml", include_str!("../templates/hi_latn.toml")),
    ("hi_deva.toml", include_str!("../templates/hi_deva.toml")),
    ("mixed.toml", include_str!("../templates/mixed.toml")),
    (
        "negatives.toml",
        include_str!("../templates/negatives.toml"),
    ),
];

/// The templates compiled into this crate, in file order.
///
/// # Errors
/// Fails if a template file does not parse or two templates share an id.
pub fn builtin_templates() -> Result<Vec<Template>, TemplateError> {
    let mut all = Vec::new();
    for (file, source) in BUILTIN_TEMPLATE_FILES {
        all.extend(parse_file(source).map_err(|e| TemplateError {
            id: format!("{file}: {}", e.id),
            ..e
        })?);
    }
    let mut seen = BTreeSet::new();
    if let Some(dup) = all.iter().find(|t| !seen.insert(t.id.as_str())) {
        return Err(TemplateError {
            id: dup.id.clone(),
            message: "duplicate template id".to_owned(),
        });
    }
    Ok(all)
}

/// Renders `per_template` samples from each template, deterministically for a given `seed`.
#[must_use]
pub fn build(templates: &[Template], seed: u64, per_template: usize) -> Vec<Sample> {
    templates
        .iter()
        .flat_map(|t| (0..per_template).map(move |n| sample(t, seed, n)))
        .collect()
}

fn sample(template: &Template, seed: u64, n: usize) -> Sample {
    let mut rng = DataRng::seed_from_u64(sample_seed(seed, &template.id, n));
    let rendered = render(template, &mut rng);
    let labels = match template.decoy() {
        Some(d) => Labels::HardNegative { decoy: d.entity() },
        None => Labels::Positive {
            spans: rendered.spans,
        },
    };
    Sample {
        id: format!("{}-{n:04}", template.id),
        text: rendered.text,
        labels,
        lang: template.lang,
        difficulty: template.difficulty,
        source: Source::Template,
        generator_version: GENERATOR_VERSION.to_owned(),
    }
}

fn sample_seed(seed: u64, template_id: &str, n: usize) -> u64 {
    let digest = Sha256::digest(format!("{seed}/{template_id}/{n}"));
    let mut first8 = [0u8; 8];
    first8.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(first8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn templates() -> Vec<Template> {
        builtin_templates().unwrap()
    }

    #[test]
    fn same_seed_gives_identical_dataset() {
        let t = templates();
        assert_eq!(build(&t, 7, 3), build(&t, 7, 3));
    }

    #[test]
    fn different_seeds_give_different_text() {
        let t = templates();
        assert_ne!(build(&t, 7, 1)[0].text, build(&t, 8, 1)[0].text);
    }

    #[test]
    fn a_template_samples_do_not_depend_on_other_templates() {
        let t = templates();
        let alone = build(&t[3..4], 7, 2);
        let together = build(&t, 7, 2);
        assert_eq!(alone[..], together[6..8]);
    }

    #[test]
    fn builtin_dataset_passes_validation() {
        let samples = build(&templates(), 42, 5);
        let issues = crate::validate::validate(&samples);
        assert!(issues.is_empty(), "{issues:#?}");
    }

    #[test]
    fn builtin_templates_cover_every_entity_type_and_language() {
        let stats = crate::stats::Stats::of(&build(&templates(), 1, 1));
        assert_eq!(
            stats.spans_by_entity.len(),
            15,
            "{:?}",
            stats.spans_by_entity.keys()
        );
        assert_eq!(stats.by_lang.len(), 4);
    }
}
