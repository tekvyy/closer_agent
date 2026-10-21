use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Which agent CLI runs a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Claude Code CLI (`claude`), signed in with a Claude account
    #[default]
    Claude,
    /// OpenAI Codex CLI (`codex`), signed in with a ChatGPT account
    Codex,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelConfig {
    default_model: String,
    models: Vec<ModelOption>,
}

#[derive(Debug, Deserialize)]
struct ModelOption {
    value: String,
    #[serde(default)]
    provider: Provider,
}

fn config() -> &'static ModelConfig {
    static CONFIG: OnceLock<ModelConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/config/claude-models.json"))
            .expect("embedded model configuration must be valid")
    })
}

pub fn default_model() -> &'static str {
    &config().default_model
}

pub fn is_supported_model(model: &str) -> bool {
    config().models.iter().any(|option| option.value == model)
}

/// Provider for a model; unknown models fall back to Claude.
pub fn provider_for(model: &str) -> Provider {
    config()
        .models
        .iter()
        .find(|option| option.value == model)
        .map(|option| option.provider)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{config, default_model, is_supported_model, provider_for, Provider};
    use std::collections::HashSet;

    #[test]
    fn configured_models_are_unique_and_include_default() {
        let config = config();
        let unique: HashSet<_> = config.models.iter().map(|model| &model.value).collect();

        assert_eq!(unique.len(), config.models.len());
        assert!(is_supported_model(default_model()));
    }

    #[test]
    fn rejects_unknown_models() {
        assert!(!is_supported_model("claude-made-up-99"));
    }

    #[test]
    fn resolves_providers() {
        assert_eq!(provider_for("claude-sonnet-5-5"), Provider::Claude);
        assert_eq!(provider_for("gpt-6.1-sol"), Provider::Codex);
        assert_eq!(provider_for("unknown-model"), Provider::Claude);
    }
}
