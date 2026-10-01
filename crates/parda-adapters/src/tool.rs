//! A tool directory: `tools/<name>/tool.toml` plus the chosen named configuration.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use parda_spec::manifest::{Capability, ToolConfig, ToolManifest, Transport};

use crate::error::AdapterError;

/// Env var holding the absolute path of the selected config's `file`, when it has one.
pub const CONFIG_FILE_ENV: &str = "PARDA_TOOL_CONFIG";

#[derive(Debug, Clone)]
pub struct Tool {
    pub dir: PathBuf,
    pub manifest: ToolManifest,
    pub config_name: String,
}

impl Tool {
    /// Loads and checks `dir/tool.toml`, selecting configuration `config_name`.
    ///
    /// # Errors
    /// Fails if the manifest cannot be read, does not parse, lacks a `default` config or the requested one,
    /// or declares capabilities its transport cannot provide.
    pub fn load(dir: &Path, config_name: &str) -> Result<Self, AdapterError> {
        let path = dir.join("tool.toml");
        let source = fs::read_to_string(&path).map_err(|source| AdapterError::ManifestRead {
            path: path.clone(),
            source,
        })?;
        let manifest: ToolManifest =
            toml::from_str(&source).map_err(|e| AdapterError::Manifest {
                path: path.clone(),
                message: e.to_string(),
            })?;
        check_manifest(&manifest).map_err(|message| AdapterError::Manifest {
            path: path.clone(),
            message,
        })?;
        if !manifest.configs.contains_key(config_name) {
            let known: Vec<&String> = manifest.configs.keys().collect();
            return Err(AdapterError::Manifest {
                path,
                message: format!("no config named `{config_name}`; available: {known:?}"),
            });
        }
        let dir = fs::canonicalize(dir).map_err(|source| AdapterError::ManifestRead {
            path: dir.to_owned(),
            source,
        })?;
        Ok(Self {
            dir,
            manifest,
            config_name: config_name.to_owned(),
        })
    }

    #[must_use]
    pub fn config(&self) -> &ToolConfig {
        &self.manifest.configs[&self.config_name]
    }

    #[must_use]
    pub fn has(&self, capability: Capability) -> bool {
        self.manifest.capabilities.contains(&capability)
    }

    /// Environment for the tool's process: the config's `env`, plus `PARDA_TOOL_CONFIG` when it names a file.
    #[must_use]
    pub fn env(&self) -> BTreeMap<String, String> {
        let config = self.config();
        let mut env = config.env.clone();
        if let Some(file) = &config.file {
            env.insert(
                CONFIG_FILE_ENV.to_owned(),
                self.dir.join(file).display().to_string(),
            );
        }
        env
    }
}

fn check_manifest(m: &ToolManifest) -> Result<(), String> {
    if !m.configs.contains_key("default") {
        return Err("`configs.default` is required".to_owned());
    }
    let is_proxy = matches!(m.transport, Transport::HttpProxy { .. });
    if is_proxy != m.capabilities.contains(&Capability::Proxy) {
        return Err(
            "the `proxy` capability goes with, and only with, the `http-proxy` transport"
                .to_owned(),
        );
    }
    let command_is_empty = match &m.transport {
        Transport::Stdio { command } => command.is_empty(),
        Transport::HttpProxy { start, .. } | Transport::HttpApi { start, .. } => start.is_empty(),
    };
    if command_is_empty {
        return Err("the transport's command is empty".to_owned());
    }
    if m.capabilities.contains(&Capability::UnmaskStream)
        && !m.capabilities.contains(&Capability::Mask)
    {
        return Err("`unmask_stream` requires `mask`".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tool(toml: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("tool.toml"), toml).unwrap();
        dir
    }

    const STDIO_TOOL: &str = r#"
name = "t"
version = "1"
commit = "abc"
license = "MIT"
language = "python"
homepage = "https://example.org"
capabilities = ["detect"]
transport = { kind = "stdio", command = ["python", "adapter.py"] }
[entity_map]
IN_PAN = "PAN"
[configs.default]
[configs.tuned]
file = "tuned.yaml"
env = { MODE = "fast" }
"#;

    #[test]
    fn loads_manifest_and_builds_config_env() {
        let dir = write_tool(STDIO_TOOL);
        let tool = Tool::load(dir.path(), "tuned").unwrap();
        let env = tool.env();
        assert_eq!(env["MODE"], "fast");
        assert!(env[CONFIG_FILE_ENV].ends_with("tuned.yaml"));
        assert!(Tool::load(dir.path(), "default").unwrap().env().is_empty());
    }

    #[test]
    fn rejects_unknown_config() {
        let dir = write_tool(STDIO_TOOL);
        let err = Tool::load(dir.path(), "fastest").unwrap_err().to_string();
        assert!(err.contains("fastest"), "{err}");
    }

    #[test]
    fn rejects_proxy_capability_on_stdio_transport() {
        let dir = write_tool(&STDIO_TOOL.replace(r#"["detect"]"#, r#"["detect", "proxy"]"#));
        assert!(Tool::load(dir.path(), "default").is_err());
    }

    #[test]
    fn rejects_manifest_without_default_config() {
        let dir = write_tool(&STDIO_TOOL.replace("[configs.default]\n", ""));
        assert!(Tool::load(dir.path(), "tuned").is_err());
    }
}
