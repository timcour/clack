use anyhow::Result;
use serde::Serialize;

pub enum Formatter {
    Json,
    Yaml,
    Human,
}

impl Formatter {
    pub fn format<T: Serialize>(&self, data: &T) -> Result<String> {
        match self {
            Formatter::Json => Ok(serde_json::to_string_pretty(data)?),
            Formatter::Yaml => Ok(serde_yaml::to_string(data)?),
            Formatter::Human => {
                // For now, just output JSON - Phase 5 will make this human-friendly
                Ok(serde_json::to_string_pretty(data)?)
            }
        }
    }
}

pub fn get_formatter(format: &str) -> Formatter {
    match format {
        "json" => Formatter::Json,
        "yaml" => Formatter::Yaml,
        _ => Formatter::Human,
    }
}
