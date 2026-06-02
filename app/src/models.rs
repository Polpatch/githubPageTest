use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Workout {
    pub id: String,
    pub nome: String,
    pub descrizione: Option<String>,
    pub categoria: Option<String>,
    pub giorni: Vec<Day>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Day {
    pub giorno: String,
    pub etichetta: Option<String>,
    pub esercizi: Vec<Exercise>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Exercise {
    pub id: String,
    pub nome: String,
    pub serie: u32,
    pub reps: String,
    pub recupero: Option<u32>,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub file: String,
    pub nome: String,
    pub numero: Option<String>,
    pub mese: Option<String>,
    pub anno: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CompletedSet {
    pub exercise_id: String,
    pub nome: String,
    pub set_number: u32,
    pub peso: Option<f32>,
    pub reps: Option<String>,
    pub timestamp: String,
}

pub fn session_key(workout_id: &str, day_label: &str) -> String {
    format!(
        "workout_session__{}__{}", workout_id,
        day_label.replace(' ', "_")
    )
}

pub fn load_session(key: &str) -> Vec<CompletedSet> {
    LocalStorage::get(key).unwrap_or_else(|_| Vec::new())
}
