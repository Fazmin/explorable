use chrono::Utc;
use tauri::State;

use crate::error::AppResult;
use crate::models::{Card, CardInput, Deck, DeckDetail, DueCard};
use crate::repo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_decks(state: State<'_, AppState>, course_id: Option<String>) -> AppResult<Vec<Deck>> {
    state.db.with(|c| repo::decks::list(c, course_id.as_deref()))
}

#[tauri::command]
pub async fn get_deck(state: State<'_, AppState>, id: String) -> AppResult<DeckDetail> {
    state.db.with(|c| repo::decks::detail(c, &id))
}

#[tauri::command]
pub async fn create_deck(
    state: State<'_, AppState>,
    course_id: Option<String>,
    title: String,
    description: String,
    cards: Vec<CardInput>,
) -> AppResult<Deck> {
    if cards.len() > 500 {
        return Err(crate::error::AppError::invalid("A deck can have at most 500 cards."));
    }
    state
        .db
        .with_tx(|tx| repo::decks::create(tx, course_id.as_deref(), None, &title, &description, &cards))
}

#[tauri::command]
pub async fn rename_deck(state: State<'_, AppState>, id: String, title: String) -> AppResult<()> {
    state.db.with(|c| repo::decks::rename(c, &id, &title))
}

#[tauri::command]
pub async fn delete_deck(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.with(|c| repo::decks::delete(c, &id))
}

#[tauri::command]
pub async fn add_card(state: State<'_, AppState>, deck_id: String, input: CardInput) -> AppResult<Card> {
    state.db.with(|c| repo::decks::add_card(c, &deck_id, &input))
}

#[tauri::command]
pub async fn update_card(state: State<'_, AppState>, id: String, input: CardInput) -> AppResult<Card> {
    state.db.with(|c| repo::decks::update_card(c, &id, &input))
}

#[tauri::command]
pub async fn delete_card(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.with(|c| repo::decks::delete_card(c, &id))
}

#[tauri::command]
pub async fn due_cards(
    state: State<'_, AppState>,
    course_id: Option<String>,
    deck_id: Option<String>,
    limit: Option<usize>,
) -> AppResult<Vec<DueCard>> {
    let limit = limit.unwrap_or(50).clamp(1, 500);
    state
        .db
        .with(|c| repo::decks::due(c, course_id.as_deref(), deck_id.as_deref(), limit, 20))
}

#[tauri::command]
pub async fn review_card(state: State<'_, AppState>, card_id: String, rating: i64) -> AppResult<Card> {
    state.db.with_tx(|tx| repo::decks::review(tx, &card_id, rating, Utc::now()))
}
