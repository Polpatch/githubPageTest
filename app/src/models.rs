use gloo_storage::{LocalStorage, Storage};
use js_sys::Date as JsDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub video: Option<String>,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub durata: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub file: String,
    pub nome: String,
    pub numero: Option<String>,
    pub mese: Option<String>,
    pub anno: Option<String>,
    pub preferita: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CompletedSet {
    pub exercise_id: String,
    pub nome: String,
    pub set_number: u32,
    pub peso: Option<f32>,
    pub reps: Option<String>,
    pub timestamp: String,
    #[serde(default)]
    pub durata_min: Option<u32>,
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

// ── User preferred scheda ────────────────────────────────────────────────────

pub fn load_user_preferred() -> Option<String> {
    LocalStorage::get::<String>("user_preferred_scheda").ok()
}

pub fn save_user_preferred(file: Option<&str>) {
    match file {
        Some(f) => { let _ = LocalStorage::set("user_preferred_scheda", f); }
        None    => { LocalStorage::delete("user_preferred_scheda"); }
    }
}

// ── Schedule storage ─────────────────────────────────────────────────────────

pub fn load_schedules() -> Vec<Workout> {
    LocalStorage::get("schedules").unwrap_or_default()
}

fn save_schedules(schedules: &[Workout]) {
    let _ = LocalStorage::set("schedules", schedules);
}

/// Replace an existing schedule (matched by id) with the fresh version, or insert if new.
/// Called every time a scheda is loaded — ensures localStorage always holds
/// the latest structure (new fields like `video`) without touching session data.
pub fn upsert_schedule(workout: &Workout) {
    let mut schedules = load_schedules();
    let id = workout.id.clone();
    upsert_by(&mut schedules, workout.clone(), |s| s.id == id);
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
    let id = meta.id.clone(); // capture before meta is moved
    upsert_by(&mut index, meta, |m| m.id == id);
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

/// Find the most recent non-terminated session for workout+day.
/// Returns None if no open session exists (use `create_session_for_day` to create one).
pub fn find_open_session(workout_id: &str, day_label: &str) -> Option<(String, Vec<CompletedSet>, usize)> {
    load_sessions(workout_id)
        .into_iter()
        .filter(|s| s.day == day_label && !s.done)
        .max_by(|a, b| a.updated.cmp(&b.updated))
        .map(|s| (s.id, s.sets, s.active_exercise))
}

/// All non-terminated session metas for a specific workout+day (for disambiguation).
pub fn open_sessions_for_day(workout_id: &str, day_label: &str) -> Vec<SessionMeta> {
    load_sessions_index()
        .into_iter()
        .filter(|m| m.workout_id == workout_id && m.day == day_label && !m.done)
        .collect()
}

/// Create and persist a brand-new session for a day. Called lazily on first set.
/// Idempotent: if an open session already exists for this day, returns its id.
pub fn create_session_for_day(workout: &Workout, day_idx: usize) -> String {
    let day = match workout.giorni.get(day_idx) {
        Some(d) => d,
        None => return new_id(),
    };
    // Safety net: don't create a second session if one already exists
    if let Some((existing_id, _, _)) = find_open_session(&workout.id, &day.giorno) {
        return existing_id;
    }
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
    let mut sessions = load_sessions(&workout.id);
    sessions.push(session);
    save_sessions(&workout.id, &sessions);
    id
}

/// Delete all non-terminated sessions for a specific workout+day.
pub fn delete_sessions_for_day(workout_id: &str, day_label: &str) {
    let mut sessions = load_sessions(workout_id);
    let before = sessions.len();
    sessions.retain(|s| !(s.day == day_label && !s.done));
    if sessions.len() != before {
        save_sessions(workout_id, &sessions);
        let mut index = load_sessions_index();
        index.retain(|m| !(m.workout_id == workout_id && m.day == day_label && !m.done));
        save_sessions_index(&index);
    }
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
        let pct = s.completion_pct(total_expected);
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

/// Mark a single session as terminated (done).
pub fn terminate_session(workout_id: &str, session_id: &str) {
    let mut sessions = load_sessions(workout_id);
    let now = now_iso();
    if let Some(s) = sessions.iter_mut().find(|s| s.id == session_id) {
        s.done = true;
        s.updated = now.clone();
        save_sessions(workout_id, &sessions);
    }
    let mut index = load_sessions_index();
    if let Some(m) = index.iter_mut().find(|m| m.id == session_id) {
        m.done = true;
        m.updated = now.clone();
    }
    save_sessions_index(&index);
}

/// Remove a session entirely from storage.
pub fn delete_session(workout_id: &str, session_id: &str) {
    let mut sessions = load_sessions(workout_id);
    sessions.retain(|s| s.id != session_id);
    save_sessions(workout_id, &sessions);
    let mut index = load_sessions_index();
    index.retain(|m| m.id != session_id);
    save_sessions_index(&index);
}

/// Insert or replace an item in a Vec. Uses `position` to avoid double-move.
fn upsert_by<T>(vec: &mut Vec<T>, item: T, matches: impl Fn(&T) -> bool) {
    match vec.iter().position(|x| matches(x)) {
        Some(i) => vec[i] = item,
        None    => vec.push(item),
    }
}

impl CatalogEntry {
    pub fn display_name(&self) -> String {
        let num = self.numero.clone().unwrap_or_default();
        if num.is_empty() { self.nome.clone() } else { format!("{} {}", self.nome, num) }
    }
    pub fn date_label(&self) -> String {
        format!("{} / {}",
            self.mese.clone().unwrap_or_default(),
            self.anno.clone().unwrap_or_default())
    }
}

/// Timer state passed as a single prop to ExerciseCard.
#[derive(Clone, PartialEq)]
pub struct TimerState {
    pub running: bool,
    pub left:    u32,
    pub total:   u32,
}

impl Session {
    pub fn completion_pct(&self, total_expected: u32) -> f32 {
        if total_expected == 0 { return 0.0; }
        (self.sets.len() as f32 / total_expected as f32 * 100.0).min(100.0)
    }
}

impl TimerState {
    #[allow(dead_code)]
    pub fn idle() -> Self { Self { running: false, left: 0, total: 0 } }
}

// ── Pure logic helpers ────────────────────────────────────────────────────────

/// Insert or update a CompletedSet in `list`. Sorts by set_number before returning.
pub fn upsert_completed_set(
    mut list: Vec<CompletedSet>,
    exercise: &Exercise,
    set_number: u32,
    peso: Option<f32>,
    reps: Option<String>,
    durata_min: Option<u32>,
) -> Vec<CompletedSet> {
    let timestamp = now_iso();
    if let Some(e) = list.iter_mut().find(|s| {
        s.exercise_id == exercise.id && s.set_number == set_number
    }) {
        e.peso       = peso;
        e.reps       = reps;
        e.durata_min = durata_min;
        e.timestamp  = timestamp;
    } else {
        list.push(CompletedSet {
            exercise_id: exercise.id.clone(),
            nome:        exercise.nome.clone(),
            set_number,
            peso,
            reps,
            durata_min,
            timestamp,
        });
    }
    list.sort_by_key(|s| s.set_number);
    list
}

/// Return the index of the next exercise in `day` that still has incomplete sets,
/// searching forward (wrapping) from `current_idx`.
/// Returns `current_idx` if all exercises are complete.
pub fn next_incomplete_exercise(
    day: &Day,
    sets: &[CompletedSet],
    current_idx: usize,
) -> usize {
    let n = day.esercizi.len();
    (1..n)
        .map(|off| (current_idx + off) % n)
        .find(|&i| {
            let ex = &day.esercizi[i];
            sets.iter().filter(|s| s.exercise_id == ex.id).count() < ex.serie as usize
        })
        .unwrap_or(current_idx)
}

/// Read the input value for `exercise_id` at `idx`, falling back to the most
/// recent non-empty value at a lower index, then to `default`.
pub fn get_input_with_fallback(
    map: &HashMap<String, Vec<String>>,
    exercise_id: &str,
    idx: usize,
    default: &str,
) -> String {
    map.get(exercise_id)
        .and_then(|v| v.get(idx).cloned())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            (0..idx).rev().find_map(|prev| {
                map.get(exercise_id)
                    .and_then(|v| v.get(prev).cloned())
                    .filter(|v| !v.is_empty())
            })
        })
        .unwrap_or_else(|| default.to_string())
}

/// Resize the inner Vec for `exercise_id` if needed, then set the value at `idx`.
pub fn update_input_map(
    mut map: HashMap<String, Vec<String>>,
    exercise_id: String,
    idx: usize,
    value: String,
) -> HashMap<String, Vec<String>> {
    let entry = map.entry(exercise_id).or_default();
    if entry.len() <= idx { entry.resize(idx + 1, String::new()); }
    entry[idx] = value;
    map
}

// ── Weight history ────────────────────────────────────────────────────────────

/// One data point for the weight-progression chart.
#[derive(Clone)]
pub struct WeightPoint {
    pub date:       String,  // "YYYY-MM-DD"
    pub max_weight: f32,
}

/// Collect the max weight used per terminated session for an exercise.
/// Searches all sessions for the workout, regardless of day.
pub fn weight_history_for_exercise(workout_id: &str, exercise_id: &str) -> Vec<WeightPoint> {
    let mut points: Vec<WeightPoint> = load_sessions(workout_id)
        .into_iter()
        .filter(|s| s.done)
        .filter_map(|s| {
            let max_w = s.sets.iter()
                .filter(|set| set.exercise_id == exercise_id)
                .filter_map(|set| set.peso)
                .filter(|w| *w > 0.0)
                .fold(f32::NEG_INFINITY, f32::max);
            if max_w == f32::NEG_INFINITY { return None; }
            let date = s.started.get(..10).unwrap_or(&s.started).to_string();
            Some(WeightPoint { date, max_weight: max_w })
        })
        .collect();
    points.sort_by(|a, b| a.date.cmp(&b.date));
    points
}

/// All terminated sessions for a workout+day, newest first.
pub fn terminated_sessions_for_day(workout_id: &str, day_label: &str) -> Vec<Session> {
    let mut sessions: Vec<Session> = load_sessions(workout_id)
        .into_iter()
        .filter(|s| s.day == day_label && s.done)
        .collect();
    sessions.sort_by(|a, b| b.updated.cmp(&a.updated));
    sessions
}

// ── Export / Import ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug)]
pub struct ExportData {
    pub version: u32,
    pub exported_at: String,
    pub schedules: Vec<Workout>,
    pub sessions_index: Vec<SessionMeta>,
    pub sessions: std::collections::HashMap<String, Vec<Session>>,
}

/// Serialize all localStorage data into a pretty-printed JSON string.
pub fn export_all_data() -> String {
    let schedules      = load_schedules();
    let sessions_index = load_sessions_index();
    // Collect sessions for every known workout_id
    let ids: std::collections::HashSet<String> = sessions_index
        .iter().map(|m| m.workout_id.clone()).collect();
    let mut sessions = std::collections::HashMap::new();
    for id in ids {
        let s = load_sessions(&id);
        if !s.is_empty() { sessions.insert(id, s); }
    }
    let data = ExportData {
        version: 1,
        exported_at: now_iso(),
        schedules,
        sessions_index,
        sessions,
    };
    serde_json::to_string_pretty(&data).unwrap_or_default()
}

/// Parse an export file and overwrite all localStorage data.
pub fn import_all_data(json: &str) -> Result<(), String> {
    let data: ExportData = serde_json::from_str(json)
        .map_err(|e| format!("Formato non riconosciuto: {}", e))?;
    save_schedules(&data.schedules);
    save_sessions_index(&data.sessions_index);
    for (workout_id, s) in &data.sessions {
        save_sessions(workout_id, s);
    }
    Ok(())
}

// ── Calendar / suggestion ────────────────────────────────────────────────────

/// Display info shown on the calendar's "next workout" CTA button.
#[derive(Clone, PartialEq)]
pub struct SuggestionInfo {
    pub workout_nome: String,
    pub day_label: String,
}

pub fn find_session_by_id(workout_id: &str, session_id: &str) -> Option<Session> {
    load_sessions(workout_id).into_iter().find(|s| s.id == session_id)
}

/// Returns (Workout, day_index) for the next suggested training based on the
/// preferred scheda. Bridges catalog entry → workout via `nome` field matching.
/// Falls back to day 0 when: no sessions for this scheda, or last session > 30 days ago.
pub fn compute_suggestion_workout(
    sessions: &[SessionMeta],
    schedules: &[Workout],
    catalog: &[CatalogEntry],
    user_preferred: &Option<String>,
) -> Option<(Workout, usize)> {
    let pref = match user_preferred {
        Some(file) => catalog.iter().find(|e| &e.file == file),
        None       => catalog.iter().find(|e| e.preferita.unwrap_or(false)),
    }?;

    // Fuzzy match: exact nome, or one is a prefix of the other.
    // Handles cases where catalog.json uses a shorter display name than the
    // full nome in the workout JSON (e.g. "Scheda X" vs "Scheda X (v2)").
    let workout = schedules.iter().find(|w| {
        w.nome == pref.nome
            || w.nome.starts_with(pref.nome.as_str())
            || pref.nome.starts_with(w.nome.as_str())
    })?.clone();

    let mut done: Vec<&SessionMeta> = sessions.iter()
        .filter(|s| s.done && s.workout_id == workout.id)
        .collect();
    done.sort_by(|a, b| a.started.cmp(&b.started));

    let day_idx = if let Some(last) = done.last() {
        let days_ago = (JsDate::now() - JsDate::parse(&last.started)) / 86_400_000.0;
        if days_ago > 30.0 {
            0
        } else {
            let cur = workout.giorni.iter().position(|d| d.giorno == last.day).unwrap_or(0);
            (cur + 1) % workout.giorni.len().max(1)
        }
    } else {
        0
    };

    Some((workout, day_idx))
}

pub fn compute_suggestion(
    sessions: &[SessionMeta],
    schedules: &[Workout],
    catalog: &[CatalogEntry],
    user_preferred: &Option<String>,
) -> Option<SuggestionInfo> {
    let (workout, day_idx) = compute_suggestion_workout(sessions, schedules, catalog, user_preferred)?;
    let day = workout.giorni.get(day_idx)?;
    Some(SuggestionInfo {
        workout_nome: workout.nome.clone(),
        day_label: day.etichetta.clone().unwrap_or_else(|| day.giorno.clone()),
    })
}

// ── Reps helpers ─────────────────────────────────────────────────────────────

/// Parse a reps target string like "8-10" or "12" into (min, max).
pub fn parse_reps_range(reps: &str) -> (i32, i32) {
    let clean = reps.trim();
    if let Some((a, b)) = clean.split_once('-') {
        let lo = a.trim().parse().unwrap_or(0);
        let hi = b.trim().parse().unwrap_or(lo);
        (lo, hi)
    } else {
        let n = clean.parse().unwrap_or(0);
        (n, n)
    }
}

