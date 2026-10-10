//! Query functions grouped by table. Each takes a `&Connection` so callers can
//! compose them inside one transaction.

pub mod artifacts;
pub mod conversations;
pub mod courses;
pub mod decks;
pub mod materials;
pub mod messages;
pub mod progress;
pub mod settings;

#[cfg(test)]
mod tests;
