//! Runs detection over samples, mapping tool labels to entity types and restarting a worker that crashed.

use std::collections::BTreeMap;

use parda_spec::entity::EntityType;
use parda_spec::protocol::adapter::DetectedEntity;
use parda_spec::record::{DetectionRecord, DetectionResult, PredictedEntity, PredictedSpan};
use parda_spec::sample::Sample;

use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::process::Output;
use crate::tool::Tool;

pub struct DetectionRunner<'a> {
    tool: &'a Tool,
    output: Output,
    adapter: Option<Adapter>,
}

impl<'a> DetectionRunner<'a> {
    /// Starts the tool.
    ///
    /// # Errors
    /// Fails if the tool cannot start or fails its handshake.
    pub async fn start(tool: &'a Tool, output: Output) -> Result<Self, AdapterError> {
        let adapter = Adapter::start(tool, &output).await?;
        Ok(Self {
            tool,
            output,
            adapter: Some(adapter),
        })
    }

    /// Detects entities in one sample. A tool error becomes a `Failed` record; a transport failure (crash, timeout,
    /// garbled reply) also restarts the worker so later samples are unaffected.
    ///
    /// # Errors
    /// Fails only if the worker cannot be restarted.
    pub async fn detect(&mut self, sample: &Sample) -> Result<DetectionRecord, AdapterError> {
        let adapter = match self.adapter.as_mut() {
            Some(a) => a,
            None => self
                .adapter
                .insert(Adapter::start(self.tool, &self.output).await?),
        };
        let result = match adapter.detect(&sample.text).await {
            Ok((entities, reply)) => DetectionResult::Detected {
                predicted: map_entities(&entities, &self.tool.manifest.entity_map),
                elapsed_ns: reply.elapsed_ns,
                wall_ns: reply.wall_ns,
            },
            Err(AdapterError::Tool(message)) => DetectionResult::Failed { message },
            Err(e) => {
                tracing::warn!(sample = %sample.id, "restarting worker after: {e}");
                if let Some(broken) = self.adapter.take() {
                    broken.shutdown().await;
                }
                DetectionResult::Failed {
                    message: e.to_string(),
                }
            }
        };
        Ok(DetectionRecord {
            sample_id: sample.id.clone(),
            result,
        })
    }

    pub async fn shutdown(self) {
        if let Some(a) = self.adapter {
            a.shutdown().await;
        }
    }
}

/// Translates tool labels through `entity_map`; unknown labels are kept as `Unmapped`.
#[must_use]
pub fn map_entities(
    entities: &[DetectedEntity],
    entity_map: &BTreeMap<String, EntityType>,
) -> Vec<PredictedSpan> {
    entities
        .iter()
        .map(|e| PredictedSpan {
            start: e.start,
            end: e.end,
            entity: entity_map.get(&e.label).map_or_else(
                || PredictedEntity::Unmapped(e.label.clone()),
                |t| PredictedEntity::Mapped(*t),
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_labels_and_keeps_unknown_ones() {
        let map = BTreeMap::from([("IN_PAN".to_owned(), EntityType::Pan)]);
        let entities = [
            DetectedEntity {
                start: 0,
                end: 10,
                label: "IN_PAN".to_owned(),
                score: Some(0.9),
            },
            DetectedEntity {
                start: 11,
                end: 15,
                label: "ORG".to_owned(),
                score: None,
            },
        ];
        let mapped = map_entities(&entities, &map);
        assert_eq!(mapped[0].entity, PredictedEntity::Mapped(EntityType::Pan));
        assert_eq!(
            mapped[1].entity,
            PredictedEntity::Unmapped("ORG".to_owned())
        );
        assert_eq!((mapped[1].start, mapped[1].end), (11, 15));
    }
}
