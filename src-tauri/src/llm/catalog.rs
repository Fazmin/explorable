//! Models offered in Settings, per provider (checked against the providers'
//! documentation in October 2026). Any other model ID can be entered under
//! Settings → Advanced.

use serde::Serialize;

pub const OPENAI_DEFAULT_MODEL: &str = "gpt-6.1-sol";
pub const OPENAI_DEFAULT_ROUTER: &str = "gpt-6-luna";
pub const GEMINI_DEFAULT_MODEL: &str = "gemini-3.8-flash";
pub const GEMINI_DEFAULT_ROUTER: &str = "gemini-3.1-flash-lite";

#[derive(Serialize)]
pub struct CatalogModel {
    pub id: &'static str,
    pub label: &'static str,
    pub note: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub company: &'static str,
    /// Short description for the provider card.
    pub tagline: &'static str,
    pub key_placeholder: &'static str,
    pub key_url: &'static str,
    pub env_var: &'static str,
    pub models: Vec<CatalogModel>,
    pub router_models: Vec<CatalogModel>,
}

const fn m(id: &'static str, label: &'static str, note: &'static str) -> CatalogModel {
    CatalogModel { id, label, note }
}

pub fn catalog() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "anthropic",
            label: "Claude",
            company: "Anthropic",
            tagline: "Claude models by Anthropic",
            key_placeholder: "sk-ant-…",
            key_url: "https://console.anthropic.com/settings/keys",
            env_var: "ANTHROPIC_API_KEY",
            models: vec![
                m("claude-opus-5-5", "Claude Opus 5.5", "Most capable; best for rich interactives (default)"),
                m("claude-sonnet-5-5", "Claude Sonnet 5.5", "Faster and lower cost"),
                m("claude-haiku-5-5", "Claude Haiku 5.5", "Fastest and cheapest; simpler answers"),
                m("claude-haiku-4-5", "Claude Haiku 4.5", "Previous-generation Haiku"),
            ],
            router_models: vec![
                m("claude-haiku-5-5", "Claude Haiku 5.5", "Fast and inexpensive (default)"),
                m("claude-haiku-4-5", "Claude Haiku 4.5", "Previous-generation Haiku"),
                m("claude-sonnet-5-5", "Claude Sonnet 5.5", "More careful, slower"),
            ],
        },
        ProviderInfo {
            id: "openai",
            label: "OpenAI",
            company: "OpenAI",
            tagline: "GPT models by OpenAI",
            key_placeholder: "sk-…",
            key_url: "https://platform.openai.com/api-keys",
            env_var: "OPENAI_API_KEY",
            models: vec![
                m("gpt-6-astra", "GPT-6 Astra", "Most capable; highest cost"),
                m("gpt-6.1-sol", "GPT-6.1 Sol", "Balanced capability and cost (default)"),
                m("gpt-6-luna", "GPT-6 Luna", "Fastest and cheapest"),
                m("gpt-5.6-sol", "GPT-5.6 Sol", "GPT-5.6 flagship"),
                m("gpt-5.6-terra", "GPT-5.6 Terra", "GPT-5.6, balanced capability and cost"),
                m("gpt-5.6-luna", "GPT-5.6 Luna", "GPT-5.6, fast and low cost"),
                m("gpt-5.5", "GPT-5.5", "GPT-5.5 flagship"),
                m("gpt-5.5-pro", "GPT-5.5 Pro", "Extra compute; no live streaming; answers can take minutes; very high cost"),
            ],
            router_models: vec![
                m("gpt-6-luna", "GPT-6 Luna", "Fast and inexpensive (default)"),
                m("gpt-5.6-luna", "GPT-5.6 Luna", "Fast and low cost"),
                m("gpt-5.6-terra", "GPT-5.6 Terra", "More careful, slower"),
                m("gpt-6.1-sol", "GPT-6.1 Sol", "Most careful, slowest"),
            ],
        },
        ProviderInfo {
            id: "gemini",
            label: "Gemini",
            company: "Google",
            tagline: "Gemini models by Google",
            key_placeholder: "AIza…",
            key_url: "https://aistudio.google.com/apikey",
            env_var: "GEMINI_API_KEY",
            models: vec![
                m("gemini-3.1-pro-preview", "Gemini 3.1 Pro (preview)", "Most capable; no free tier"),
                m("gemini-3.8-flash", "Gemini 3.8 Flash", "Fast and capable (default)"),
                m("gemini-3.5-flash-lite", "Gemini 3.5 Flash-Lite", "Fastest and cheapest"),
            ],
            router_models: vec![
                m("gemini-3.1-flash-lite", "Gemini 3.1 Flash-Lite", "Fast and inexpensive (default)"),
                m("gemini-3.5-flash-lite", "Gemini 3.5 Flash-Lite", "Newer lightweight model"),
                m("gemini-3.8-flash", "Gemini 3.8 Flash", "More careful, slower"),
            ],
        },
    ]
}

pub fn provider_label(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "Anthropic",
        "openai" => "OpenAI",
        "gemini" => "Gemini",
        _ => "AI provider",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_in_the_catalog() {
        let cat = catalog();
        let has = |p: &str, id: &str, router: bool| {
            cat.iter()
                .find(|x| x.id == p)
                .map(|x| if router { &x.router_models } else { &x.models }.iter().any(|m| m.id == id))
                .unwrap_or(false)
        };
        assert!(has("openai", OPENAI_DEFAULT_MODEL, false));
        assert!(has("openai", OPENAI_DEFAULT_ROUTER, true));
        assert!(has("gemini", GEMINI_DEFAULT_MODEL, false));
        assert!(has("gemini", GEMINI_DEFAULT_ROUTER, true));
        assert!(has("anthropic", "claude-opus-5-5", false));
        assert!(has("anthropic", "claude-haiku-5-5", true));
        assert!(has("openai", "gpt-5.5-pro", false));
    }
}
