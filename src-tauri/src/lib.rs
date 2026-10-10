mod agent;
mod commands;
mod db;
mod error;
mod export;
mod llm;
mod models;
mod partial_json;
mod repo;
mod secrets;
mod srs;
mod state;
mod text;

use tauri::Manager;

use commands::{chats, courses, decks, library, materials, progress, settings, system};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = db::Db::open(&data_dir.join("explorable.sqlite3"))?;
            app.manage(state::AppState::new(db, data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::update_settings,
            settings::set_api_key,
            settings::clear_api_key,
            settings::test_api_key,
            settings::model_catalog,
            courses::list_courses,
            courses::create_course,
            courses::update_course,
            courses::delete_course,
            chats::list_conversations,
            chats::create_conversation,
            chats::get_conversation,
            chats::rename_conversation,
            chats::set_conversation_style,
            chats::delete_conversation,
            chats::send_message,
            chats::answer_choice,
            chats::retry_run,
            chats::cancel_run,
            library::list_artifacts,
            library::get_artifact,
            library::save_artifact,
            library::rename_artifact,
            library::delete_artifact,
            decks::list_decks,
            decks::get_deck,
            decks::create_deck,
            decks::rename_deck,
            decks::delete_deck,
            decks::add_card,
            decks::update_card,
            decks::delete_card,
            decks::due_cards,
            decks::review_card,
            progress::get_progress,
            progress::record_quiz_attempt,
            progress::quiz_attempts,
            materials::list_materials,
            materials::get_material,
            materials::add_material_text,
            materials::import_material,
            materials::delete_material,
            materials::search_materials,
            system::export_item,
            system::write_text_file,
            system::open_external,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Explorable");
}
