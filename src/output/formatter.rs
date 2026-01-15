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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_json_formatter() {
        let formatter = Formatter::Json;
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let output = formatter.format(&data).unwrap();
        assert!(output.contains("\"name\": \"test\""));
        assert!(output.contains("\"value\": 42"));
        // Verify it's valid JSON
        assert!(serde_json::from_str::<serde_json::Value>(&output).is_ok());
    }

    #[test]
    fn test_yaml_formatter() {
        let formatter = Formatter::Yaml;
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let output = formatter.format(&data).unwrap();
        assert!(output.contains("name: test"));
        assert!(output.contains("value: 42"));
    }

    #[test]
    fn test_human_formatter() {
        let formatter = Formatter::Human;
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let output = formatter.format(&data).unwrap();
        // For now, human formatter outputs JSON
        assert!(output.contains("\"name\": \"test\""));
        assert!(output.contains("\"value\": 42"));
    }

    #[test]
    fn test_formatter_selection_json() {
        let formatter = get_formatter("json");
        assert!(matches!(formatter, Formatter::Json));
    }

    #[test]
    fn test_formatter_selection_yaml() {
        let formatter = get_formatter("yaml");
        assert!(matches!(formatter, Formatter::Yaml));
    }

    #[test]
    fn test_formatter_selection_human() {
        let formatter = get_formatter("human");
        assert!(matches!(formatter, Formatter::Human));
    }

    #[test]
    fn test_formatter_selection_default() {
        let formatter = get_formatter("unknown");
        assert!(matches!(formatter, Formatter::Human));
    }
}
