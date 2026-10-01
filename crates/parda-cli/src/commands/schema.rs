use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub fn write_schemas(out: &Path) -> Result<()> {
    fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    for (name, schema) in parda_spec::schemas() {
        let path = out.join(format!("{name}.schema.json"));
        let mut json = serde_json::to_string_pretty(&schema)
            .with_context(|| format!("serializing schema {name}"))?;
        json.push('\n');
        fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}
