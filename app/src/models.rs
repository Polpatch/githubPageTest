use gloo_storage::{LocalStorage, Storage};
use js_sys::Date as JsDate;
use serde::{Deserialize, Serialize};

// ── Core workout data ────────────────────────────────────────────────────────

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

// ── Session schema ───────────────────────────────────────────────────────────

/// Full session stored under `sessions__{workout_id}`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Session {
    pub id: String,
    pub workout_id: String,
    pub workout_nome: String,
    pub day: String,
    pub started: String,
    pub updated: String,
    pub done: bool,
    pub active_exercise: usize,
    pub sets: Vec<CompletedSet>,
}

/// Lightweight entry stored in the `sessions_index` key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionMeta {
    pub id: String,
    pub workout_id: String,
    pub workout_nome: String,
    pub day: String,
    pub started: String,
    pub updated: String,
    pub done: bool,
    pub completion_pct: f32,
}

// ── Timestamp helpers ────────────────────────────────────────────────────────

pub fn now_iso() -> String {
    JsDate::new_0().to_iso_string().as_string().unwrap_or_default()
}

fn new_id() -> String {
    (JsDate::now() as u64).to_string()
}

// ── Schedule storage ─────────────────────────────────────────────────────────

pub fn load_schedules() -> Vec<Workout> {
    LocalStorage::get("schedules").unwrap_or_default()
}

fn save_schedules(schedules: &[Workout]) {
    let _ = LocalStorage::set("schedules", schedules);
}

/// Insert or replace a schedule by id.
pub fn upsert_schedule(workout: &Workout) {
    let mut schedules = load_schedules();
    if let Some(e) = schedules.iter_mut().find(|s| s.id == workout.id) {
        *e = workout.clone();
    } else {
        schedules.push(workout.clone());
    }
    save_schedules(&schedules);
}

// ── Sessions storage ─────────────────────────────────────────────────────────

fn sessions_key(workout_id: &str) -> String {
    format!("sessions__{}", workout_id)
}

pub fn load_sessions(workout_id: &str) -> Vec<Session> {
    LocalStorage::get(sessions_key(workout_id)).unwrap_or_default()
}

fn save_sessions(workout_id: &str, sessions: &[Session]) {
    let _ = LocalStorage::set(sessions_key(workout_id), sessions);
}

// ── Sessions index ───────────────────────────────────────────────────────────

pub fn load_sessions_index() -> Vec<SessionMeta> {
    LocalStorage::get("sessions_index").unwrap_or_default()
}

fn save_sessions_index(index: &[SessionMeta]) {
    let _ = LocalStorage::set("sessions_index", index);
}

fn upsert_session_meta(meta: SessionMeta) {
    let mut index = load_sessions_index();
    if let Some(e) = index.iter_mut().find(|m| m.id == meta.id) {
        *e = meta;
    } else {
        index.push(meta);
    }
    save_sessions_index(&index);
}

// ── Session helpers ───────────────────────────────────────────────────────────

/// Total number of sets expected for a day (sum of all `serie`).
pub fn total_day_sets(workout: &Workout, day_label: &str) -> u32 {
    workout.giorni.iter()
        .find(|d| d.giorno == day_label)
        .map(|d| d.esercizi.iter().map(|e| e.serie).sum())
        .unwrap_or(0)
}

/// Return the most recent non-terminated session for workout+day,
/// or create and persist a brand-new one.
/// Returns (session_id, existing_sets, active_exercise_index).
pub fn find_or_create_session(workout: &Workout, day_idx: usize) -> (String, Vec<CompletedSet>, usize) {
    let day = match workout.giorni.get(day_idx) {
        Some(d) => d,
        None => return (new_id(), vec![], 0),
    };
    let mut sessions = load_sessions(&workout.id);

    // Most recently updated open session for this day
    let existing = sessions.iter()
        .filter(|s| s.day == day.giorno && !s.done)
        .max_by(|a, b| a.updated.cmp(&b.updated))
        .map(|s| (s.id.clone(), s.sets.clone(), s.active_exercise));

    if let Some(found) = existing {
        return found;
    }

    // Create a new session
    let id  = new_id();
    let now = now_iso();
    let session = Session {
        id: id.clone(),
        workout_id: workout.id.clone(),
        workout_nome: workout.nome.clone(),
        day: day.giorno.clone(),
        started: now.clone(),
        updated: now.clone(),
        done: false,
        active_exercise: 0,
        sets: vec![],
    };
    upsert_session_meta(SessionMeta {
        id: id.clone(),
        workout_id: workout.id.clone(),
        workout_nome: workout.nome.clone(),
        day: day.giorno.clone(),
        started: now.clone(),
        updated: now,
        done: false,
        completion_pct: 0.0,
    });
    sessions.push(session);
    save_sessions(&workout.id, &sessions);
    (id, vec![], 0)
}

/// Persist updated sets (and active_exercise) for an existing session,
/// and refresh the sessions_index entry.
pub fn update_session_sets(
    workout_id: &str,
    session_id: &str,
    sets: &[CompletedSet],
    active_exercise: usize,
    total_expected: u32,
) {
    let mut sessions = load_sessions(workout_id);
    if let Some(s) = sessions.iter_mut().find(|s| s.id == session_id) {
        s.sets = sets.to_vec();
        s.active_exercise = active_exercise;
        s.updated = now_iso();
        let pct = if total_expected > 0 {
            (sets.len() as f32 / total_expected as f32 * 100.0).min(100.0)
        } else {
            0.0
        };
        upsert_session_meta(SessionMeta {
            id: session_id.to_string(),
            workout_id: workout_id.to_string(),
            workout_nome: s.workout_nome.clone(),
            day: s.day.clone(),
            started: s.started.clone(),
            updated: s.updated.clone(),
            done: s.done,
            completion_pct: pct,
        });
        save_sessions(workout_id, &sessions);
    }
}
