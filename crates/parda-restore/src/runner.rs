//! Runs scenarios against a target and scores each one.

use parda_adapters::adapter::Adapter;
use parda_adapters::process::{Output, spawn_service, wait_until_listening};
use parda_adapters::tool::Tool;
use parda_adapters::{AdapterError, adapter::STARTUP_TIMEOUT};
use parda_spec::manifest::{Capability, Transport};
use parda_spec::protocol::driver::{DriverRequest, DriverTurn, Observation, ObservedToolCall};
use parda_spec::record::{CheckFailure, RestoreOutcome, RestoreRecord};
use parda_spec::scenario::{Category, CheckKind, Scenario, TurnInput};

use crate::align::render_input;
use crate::checks::evaluate;
use crate::driver::{Driver, DriverError};
use crate::engine::{Engine, ScriptedReply};
use crate::mock::MockUpstream;

pub const MODEL: &str = "parda-mock";

/// Label used as the driver of a library-tool run, where the harness calls the tool directly.
pub const LIBRARY_DRIVER: &str = "library";

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error("starting the mock upstream: {0}")]
    Mock(std::io::Error),
}

/// How a tool takes part in the restore suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRole {
    /// Sits between client and upstream; exercised through drivers.
    Proxy,
    /// Called by the harness around the upstream request.
    Library,
    /// Cannot restore values; every scenario is not applicable.
    RedactOnly,
}

#[must_use]
pub fn role(tool: &Tool) -> ToolRole {
    if tool.has(Capability::Proxy) {
        ToolRole::Proxy
    } else if tool.has(Capability::Mask) && tool.has(Capability::Unmask) {
        ToolRole::Library
    } else {
        ToolRole::RedactOnly
    }
}

/// Every scenario through `driver` into a proxy tool, which forwards to the mock upstream.
///
/// # Errors
/// Fails if the mock or the proxy cannot start, or the driver process breaks.
pub async fn run_proxy(
    tool: &Tool,
    driver: &mut Driver,
    scenarios: &[Scenario],
    output: &Output,
) -> Result<Vec<RestoreRecord>, RunError> {
    let Transport::HttpProxy {
        start,
        base_url,
        upstream_env,
    } = &tool.manifest.transport
    else {
        return Err(AdapterError::WrongTransport("stdio or http-api").into());
    };
    let mock = MockUpstream::start().await.map_err(RunError::Mock)?;
    let mut env = tool.env();
    env.insert(upstream_env.clone(), mock.base_url());
    let mut proxy = spawn_service(&tool.dir, start, &env, output)?;
    wait_until_listening(base_url, &mut proxy, STARTUP_TIMEOUT).await?;
    let records = run_with_driver(driver, &mock, base_url, scenarios).await;
    proxy.terminate().await;
    records
}

/// Every scenario through `driver` straight into the mock, with no tool: checks the drivers and the suite itself.
/// Every `no_leak_upstream` check is expected to fail; mangling scenarios are skipped because their script alters
/// the raw value itself, which says nothing about a driver.
///
/// # Errors
/// Fails if the mock cannot start or the driver process breaks.
pub async fn run_control(
    driver: &mut Driver,
    scenarios: &[Scenario],
) -> Result<Vec<RestoreRecord>, RunError> {
    let mock = MockUpstream::start().await.map_err(RunError::Mock)?;
    let (mangling, runnable): (Vec<&Scenario>, Vec<&Scenario>) = scenarios
        .iter()
        .partition(|s| s.category == Category::PlaceholderMangling);
    let runnable: Vec<Scenario> = runnable.into_iter().cloned().collect();
    let mut records = run_with_driver(driver, &mock, &mock.base_url(), &runnable).await?;
    records.extend(mangling.into_iter().map(|s| RestoreRecord {
        scenario_id: s.id.clone(),
        outcome: not_applicable("control run: the script alters the raw value itself".to_owned()),
    }));
    records.sort_by(|a, b| a.scenario_id.cmp(&b.scenario_id));
    Ok(records)
}

async fn run_with_driver(
    driver: &mut Driver,
    mock: &MockUpstream,
    base_url: &str,
    scenarios: &[Scenario],
) -> Result<Vec<RestoreRecord>, RunError> {
    let mut records = Vec::new();
    for (n, scenario) in scenarios.iter().enumerate() {
        let outcome = if driver.supports(scenario.api) {
            mock.load(scenario.clone());
            let request = driver_request(n as u64 + 1, base_url, scenario);
            let observations = driver.run(&request).await?.observations;
            let log = mock.finish().ok_or_else(|| {
                RunError::Mock(std::io::Error::other(
                    "scenario was unloaded during its run",
                ))
            })?;
            evaluate(scenario, &log, &observations)
        } else {
            not_applicable(format!(
                "driver {} does not speak {:?}",
                driver.name(),
                scenario.api
            ))
        };
        records.push(RestoreRecord {
            scenario_id: scenario.id.clone(),
            outcome,
        });
    }
    Ok(records)
}

/// Every scenario through a library tool: the harness masks the input, plays the upstream in-process, and
/// unmasks the reply (chunk by chunk with `unmask_stream` when the scenario streams).
///
/// # Errors
/// Fails if the tool cannot start; a crash mid-run fails the remaining scenarios instead.
pub async fn run_library(
    tool: &Tool,
    scenarios: &[Scenario],
    output: &Output,
) -> Result<Vec<RestoreRecord>, RunError> {
    let mut adapter = Some(Adapter::start(tool, output).await?);
    let mut records = Vec::new();
    for (n, scenario) in scenarios.iter().enumerate() {
        let outcome = if let Some(reason) = library_not_applicable(tool, scenario) {
            not_applicable(reason)
        } else {
            let a = match adapter.as_mut() {
                Some(a) => a,
                None => adapter.insert(Adapter::start(tool, output).await?),
            };
            let session = format!("{}#{n}", scenario.id);
            match run_library_scenario(a, &session, scenario).await {
                Ok((log, observations)) => evaluate(scenario, &log, &observations),
                Err(e) => {
                    tracing::warn!(scenario = %scenario.id, "restarting tool after: {e}");
                    if let Some(broken) = adapter.take() {
                        broken.shutdown().await;
                    }
                    harness_failure(&format!("tool stopped responding: {e}"))
                }
            }
        };
        records.push(RestoreRecord {
            scenario_id: scenario.id.clone(),
            outcome,
        });
    }
    if let Some(a) = adapter {
        a.shutdown().await;
    }
    Ok(records)
}

fn library_not_applicable(tool: &Tool, scenario: &Scenario) -> Option<String> {
    if scenario.stream && !tool.has(Capability::UnmaskStream) {
        return Some("streaming scenario; tool has no unmask_stream".to_owned());
    }
    if scenario.checks.contains(&CheckKind::FailsClosed) {
        return Some(
            "failure behaviour is a property of proxies; a library tool has no upstream to fail"
                .to_owned(),
        );
    }
    None
}

/// Transport failures end the scenario; tool-reported errors become `Errored` observations.
async fn run_library_scenario(
    adapter: &mut Adapter,
    session: &str,
    scenario: &Scenario,
) -> Result<(crate::engine::UpstreamLog, Vec<Observation>), AdapterError> {
    let mut engine = Engine::new(scenario.clone());
    let mut observations = Vec::new();
    for (turn, script) in scenario.turns.iter().enumerate() {
        let segments = match &script.input {
            TurnInput::User { message } => message,
            TurnInput::ToolResult { content, .. } => content,
        };
        let masked = match adapter
            .mask(session, &render_input(segments, &scenario.pii))
            .await
        {
            Ok(m) => m,
            Err(AdapterError::Tool(message)) => {
                observations.push(Observation::Errored {
                    message,
                    http_status: None,
                });
                continue;
            }
            Err(e) => return Err(e),
        };
        engine.record_payload(serde_json::Value::String(masked.clone()));
        let observation = match engine.reply(turn, &masked) {
            ScriptedReply::Text { chunks } => restore(adapter, session, scenario.stream, &chunks)
                .await?
                .map_or_else(tool_error, |text| Observation::Completed {
                    text,
                    tool_calls: vec![],
                }),
            ScriptedReply::ToolCall {
                call_id,
                name,
                argument_chunks,
            } => restore(adapter, session, scenario.stream, &argument_chunks)
                .await?
                .map_or_else(tool_error, |arguments| Observation::Completed {
                    text: String::new(),
                    tool_calls: vec![ObservedToolCall {
                        id: call_id,
                        name,
                        arguments,
                    }],
                }),
            ScriptedReply::HttpError { status, body } => Observation::Errored {
                message: body,
                http_status: Some(status),
            },
            ScriptedReply::MockError { message } => Observation::Errored {
                message,
                http_status: None,
            },
        };
        observations.push(observation);
    }
    Ok((engine.into_log(), observations))
}

/// Unmasks a reply: whole with `unmask`, or chunk by chunk with `unmask_stream`. The inner `Err` is a
/// tool-reported error message.
async fn restore(
    adapter: &mut Adapter,
    session: &str,
    stream: bool,
    chunks: &[String],
) -> Result<Result<String, String>, AdapterError> {
    let result = if stream {
        let mut out = String::new();
        let mut result = Ok(());
        for (i, chunk) in chunks.iter().enumerate() {
            match adapter
                .unmask_stream(session, chunk, i + 1 == chunks.len())
                .await
            {
                Ok(text) => out.push_str(&text),
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        result.map(|()| out)
    } else {
        adapter.unmask(session, &chunks.concat()).await
    };
    match result {
        Ok(text) => Ok(Ok(text)),
        Err(AdapterError::Tool(message)) => Ok(Err(message)),
        Err(e) => Err(e),
    }
}

fn tool_error(message: String) -> Observation {
    Observation::Errored {
        message,
        http_status: None,
    }
}

/// The conversation a driver plays: each turn's input with real values filled in.
#[must_use]
pub fn driver_request(id: u64, base_url: &str, scenario: &Scenario) -> DriverRequest {
    let turns = scenario
        .turns
        .iter()
        .map(|t| match &t.input {
            TurnInput::User { message } => DriverTurn::User {
                text: render_input(message, &scenario.pii),
            },
            TurnInput::ToolResult { call_id, content } => DriverTurn::ToolResult {
                call_id: call_id.clone(),
                content: render_input(content, &scenario.pii),
            },
        })
        .collect();
    DriverRequest {
        id,
        base_url: base_url.to_owned(),
        api: scenario.api,
        model: MODEL.to_owned(),
        stream: scenario.stream,
        tools: scenario.tools.clone(),
        turns,
    }
}

/// Outcome for every scenario of a tool that cannot restore values.
#[must_use]
pub fn redact_only(scenarios: &[Scenario]) -> Vec<RestoreRecord> {
    scenarios
        .iter()
        .map(|s| RestoreRecord {
            scenario_id: s.id.clone(),
            outcome: not_applicable("tool cannot unmask (redact-only)".to_owned()),
        })
        .collect()
}

fn not_applicable(reason: String) -> RestoreOutcome {
    RestoreOutcome::NotApplicable { reason }
}

fn harness_failure(detail: &str) -> RestoreOutcome {
    RestoreOutcome::Fail {
        failures: vec![CheckFailure {
            check: CheckKind::RestoredExact,
            detail: detail.to_owned(),
        }],
        upstream_excerpt: String::new(),
        client_excerpt: String::new(),
    }
}
