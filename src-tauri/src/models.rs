//! Data shapes shared with the frontend. Field names serialize as camelCase.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub id: String,
    pub name: String,
    pub code: String,
    pub level: String,
    pub discipline: String,
    pub description: String,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseInput {
    pub name: String,
    #[serde(default)]
    pub code: String,
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default)]
    pub discipline: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_level() -> String {
    "undergraduate".into()
}
fn default_color() -> String {
    "blue".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Learn,
    Teach,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Learn => "learn",
            Mode::Teach => "teach",
        }
    }
    pub fn parse(s: &str) -> Mode {
        if s == "teach" {
            Mode::Teach
        } else {
            Mode::Learn
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    Direct,
    Socratic,
}

impl Style {
    pub fn as_str(&self) -> &'static str {
        match self {
            Style::Direct => "direct",
            Style::Socratic => "socratic",
        }
    }
    pub fn parse(s: &str) -> Style {
        if s == "socratic" {
            Style::Socratic
        } else {
            Style::Direct
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub course_id: Option<String>,
    pub mode: Mode,
    pub style: Style,
    pub title: String,
    /// Present while the assistant waits for the user to pick an option.
    pub pending: Option<PendingChoice>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingChoice {
    pub tool_use_id: String,
    /// Results for other tools called in the same assistant message; they are
    /// sent together with the user's answer in one user message.
    #[serde(default)]
    pub other_results: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub conversation_id: String,
    pub seq: i64,
    pub role: String,
    /// Anthropic-format content blocks, stored verbatim so thinking signatures
    /// and server-tool blocks round-trip unchanged.
    pub content: Value,
    pub meta: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    pub conversation: Conversation,
    pub messages: Vec<StoredMessage>,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub course_id: Option<String>,
    pub conversation_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub payload: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactInput {
    pub course_id: Option<String>,
    pub conversation_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub id: String,
    pub course_id: String,
    pub title: String,
    pub source_name: String,
    pub word_count: i64,
    pub chunk_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialHit {
    pub material_id: String,
    pub title: String,
    pub ordinal: i64,
    /// The whole chunk, given to the model.
    pub content: String,
    /// A short highlighted excerpt for display.
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deck {
    pub id: String,
    pub course_id: Option<String>,
    pub title: String,
    pub description: String,
    pub card_count: i64,
    pub due_count: i64,
    pub new_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub deck_id: String,
    pub position: i64,
    pub front: String,
    pub back: String,
    pub hint: String,
    pub ease: f64,
    pub interval_days: f64,
    pub reps: i64,
    pub lapses: i64,
    pub due_at: String,
    pub last_reviewed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckDetail {
    pub deck: Deck,
    pub cards: Vec<Card>,
}

/// A card ready for review, with the interval each rating would schedule.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DueCard {
    pub card: Card,
    pub deck_title: String,
    /// Labels for Again, Hard, Good, Easy, e.g. "10m", "1d", "4d".
    pub previews: [String; 4],
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardInput {
    pub front: String,
    pub back: String,
    #[serde(default)]
    pub hint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizAttemptInput {
    pub course_id: Option<String>,
    pub conversation_id: Option<String>,
    pub quiz_key: String,
    pub title: String,
    #[serde(default)]
    pub topic: String,
    pub score: f64,
    pub max_score: f64,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizAttempt {
    pub id: String,
    pub course_id: Option<String>,
    pub quiz_key: String,
    pub title: String,
    pub topic: String,
    pub score: f64,
    pub max_score: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMastery {
    pub topic: String,
    pub attempts: i64,
    pub accuracy: f64,
    pub last_attempt_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    pub date: String,
    pub reviews: i64,
    pub questions: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub quizzes_taken: i64,
    pub average_score: Option<f64>,
    pub cards_total: i64,
    pub cards_due: i64,
    pub cards_learned: i64,
    pub reviews_last7: i64,
    pub streak_days: i64,
    pub topics: Vec<TopicMastery>,
    pub activity: Vec<DailyActivity>,
    pub recent_attempts: Vec<QuizAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    /// "anthropic", "openai", "gemini", or "demo".
    pub provider: String,
    /// Claude model for answers.
    pub model: String,
    /// Claude model for routing and titles.
    pub router_model: String,
    pub openai_model: String,
    pub openai_router_model: String,
    /// Empty for the official endpoint.
    pub openai_base_url: String,
    pub gemini_model: String,
    pub gemini_router_model: String,
    /// Empty for the official endpoint.
    pub gemini_base_url: String,
    /// "low", "medium", or "high"; each provider maps it to its own control.
    pub effort: String,
    pub router_enabled: bool,
    pub refusal_fallback: bool,
    pub default_style: Style,
    pub onboarded: bool,
    // Accessibility
    /// Whole-app zoom: 1.0 to 1.5.
    pub text_scale: f64,
    pub high_contrast: bool,
    /// Turns off animation even if the OS setting is off.
    pub reduce_motion: bool,
    /// Wider line, letter, and word spacing.
    pub relaxed_spacing: bool,
    /// "hyperlegible", "serif", or "system".
    pub reading_font: String,
    /// Show read-aloud buttons on answers.
    pub read_aloud: bool,
}

impl Settings {
    /// The (answer model, router model) pair for a provider.
    pub fn models_for(&self, provider: &str) -> (String, String) {
        match provider {
            "openai" => (self.openai_model.clone(), self.openai_router_model.clone()),
            "gemini" => (self.gemini_model.clone(), self.gemini_router_model.clone()),
            _ => (self.model.clone(), self.router_model.clone()),
        }
    }

    pub fn base_url_for(&self, provider: &str) -> Option<String> {
        let url = match provider {
            "openai" => &self.openai_base_url,
            "gemini" => &self.gemini_base_url,
            _ => return None,
        };
        let url = url.trim().trim_end_matches('/');
        (!url.is_empty()).then(|| url.to_string())
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "system".into(),
            provider: "demo".into(),
            model: "claude-opus-5-5".into(),
            router_model: "claude-haiku-5-5".into(),
            openai_model: crate::llm::catalog::OPENAI_DEFAULT_MODEL.into(),
            openai_router_model: crate::llm::catalog::OPENAI_DEFAULT_ROUTER.into(),
            openai_base_url: String::new(),
            gemini_model: crate::llm::catalog::GEMINI_DEFAULT_MODEL.into(),
            gemini_router_model: crate::llm::catalog::GEMINI_DEFAULT_ROUTER.into(),
            gemini_base_url: String::new(),
            effort: "medium".into(),
            router_enabled: true,
            refusal_fallback: true,
            default_style: Style::Direct,
            onboarded: false,
            text_scale: 1.0,
            high_contrast: false,
            reduce_motion: false,
            relaxed_spacing: false,
            reading_font: "hyperlegible".into(),
            read_aloud: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ApiKeySources {
    pub anthropic: Option<String>,
    pub openai: Option<String>,
    pub gemini: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    /// Where each provider's key comes from: "keychain", "env", or null.
    pub api_keys: ApiKeySources,
    pub data_dir: String,
    pub version: String,
}
