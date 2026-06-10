mod components;
mod models;
mod recovery_timer;

use components::bottom_sheet::BottomSheet;
use components::calendar::Calendar;
use components::catalog_panel::CatalogPanel;
use components::workout_view::WorkoutView;
use components::icons::*;
use gloo_file::callbacks::{read_as_text, FileReader};
use gloo_file::File as GlooFile;
use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use models::{load_user_preferred, save_user_preferred, TimerState, *};
use recovery_timer::use_recovery_timer;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::HtmlInputElement;
use yew::prelude::*;

// ── Wake Lock helpers ─────────────────────────────────────────────────────────

/// Request a screen wake lock and store the sentinel in `slot`.
/// Silently does nothing if the API is unavailable (old browser / HTTP context).
fn acquire_wake_lock(slot: Rc<RefCell<Option<JsValue>>>) {
    spawn_local(async move {
        let Some(window) = web_sys::window() else { return };
        let nav  = window.navigator();
        let Ok(wl) = js_sys::Reflect::get(&nav, &"wakeLock".into()) else { return };
        if wl.is_undefined() || wl.is_null() { return }
        let Ok(req) = js_sys::Reflect::get(&wl, &"request".into()) else { return };
        let Some(req_fn) = req.dyn_ref::<js_sys::Function>() else { return };
        let Ok(promise) = req_fn.call1(&wl, &"screen".into()) else { return };
        if let Ok(sentinel) = JsFuture::from(js_sys::Promise::from(promise)).await {
            *slot.borrow_mut() = Some(sentinel);
        }
    });
}

/// Release a previously acquired wake lock sentinel.
fn release_wake_lock(slot: &Rc<RefCell<Option<JsValue>>>) {
    if let Some(sentinel) = slot.borrow_mut().take() {
        if let Ok(rel) = js_sys::Reflect::get(&sentinel, &"release".into()) {
            if let Some(rel_fn) = rel.dyn_ref::<js_sys::Function>() {
                let _ = rel_fn.call0(&sentinel);
            }
        }
    }
}


/// Build weight_inputs / reps_inputs maps from a saved-sets list so that
/// input fields are pre-filled when a session is loaded or resumed.
fn inputs_from_sets(sets: &[CompletedSet]) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    let mut w: HashMap<String, Vec<String>> = HashMap::new();
    let mut r: HashMap<String, Vec<String>> = HashMap::new();
    for s in sets {
        let idx = s.set_number.saturating_sub(1) as usize;
        let wv = w.entry(s.exercise_id.clone()).or_default();
        if wv.len() <= idx { wv.resize(idx + 1, String::new()); }
        wv[idx] = s.peso.map(|p| p.to_string()).unwrap_or_default();
        let rv = r.entry(s.exercise_id.clone()).or_default();
        if rv.len() <= idx { rv.resize(idx + 1, String::new()); }
        rv[idx] = s.reps.clone().unwrap_or_default();
    }
    (w, r)
}

/// JSON Schema for validating custom schede (VS Code / editor autocomplete).
const SCHEDA_SCHEMA: &str = include_str!("../../examples/schede/workout_schema.json");

/// Template JSON shown to the user as a starting point for custom schede.
const TEMPLATE_SCHEDA: &str = r#"{
  "id": "mia_scheda",
  "nome": "La mia scheda personalizzata",
  "descrizione": "Descrizione opzionale — appare nella pagina dell'allenamento",
  "categoria": "Ipertrofia / Forza / Dimagrimento",
  "giorni": [
    {
      "giorno": "A",
      "etichetta": "Giorno A — Spinta",
      "esercizi": [
        {
          "id": "esercizio_1",
          "nome": "Panca Piana",
          "serie": 4,
          "reps": "8-10",
          "recupero": 120,
          "note": "Nota opzionale: tecnica, variante, focus muscolare...",
          "video": "https://www.youtube.com/watch?v=vcBig73ojpE"
        },
        {
          "id": "esercizio_2",
          "nome": "Lento Avanti",
          "serie": 3,
          "reps": "10-12",
          "recupero": 90
        },
        {
          "id": "esercizio_3",
          "nome": "French Press",
          "serie": 3,
          "reps": "12-15",
          "recupero": 60
        }
      ]
    },
    {
      "giorno": "B",
      "etichetta": "Giorno B — Tirata",
      "esercizi": [
        {
          "id": "esercizio_4",
          "nome": "Stacco da Terra",
          "serie": 4,
          "reps": "5-6",
          "recupero": 180
        },
        {
          "id": "esercizio_5",
          "nome": "Rematore con Bilanciere",
          "serie": 4,
          "reps": "8-10",
          "recupero": 120
        },
        {
          "id": "esercizio_6",
          "nome": "Curl con Bilanciere",
          "serie": 3,
          "reps": "10-12",
          "recupero": 60
        }
      ]
    },
    {
      "giorno": "C",
      "etichetta": "Giorno C — Gambe",
      "esercizi": [
        {
          "id": "esercizio_7",
          "nome": "Squat",
          "serie": 4,
          "reps": "8-10",
          "recupero": 150
        },
        {
          "id": "esercizio_8",
          "nome": "Leg Press",
          "serie": 3,
          "reps": "12-15",
          "recupero": 90
        },
        {
          "id": "esercizio_9",
          "nome": "Leg Curl",
          "serie": 3,
          "reps": "10-12",
          "recupero": 60
        }
      ]
    }
  ]
}
"#;

/// Trigger a browser file-download for a JSON string.
/// The anchor is intentionally NOT appended to the DOM: clicking a detached
/// element doesn't bubble through Yew's event tree, avoiding the
/// "closure invoked recursively" wasm-bindgen panic.
fn trigger_download(filename: &str, content: &str) {
    let Some(window)   = web_sys::window()  else { return };
    let Some(document) = window.document()  else { return };
    let arr  = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(content));
    let Ok(blob) = web_sys::Blob::new_with_str_sequence(&arr) else { return };
    let Ok(url)  = web_sys::Url::create_object_url_with_blob(&blob) else { return };
    let anchor: web_sys::HtmlAnchorElement = match document
        .create_element("a").ok()
        .and_then(|el| el.dyn_into::<web_sys::HtmlAnchorElement>().ok())
    {
        Some(a) => a,
        None    => { let _ = web_sys::Url::revoke_object_url(&url); return }
    };
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click(); // detached → no DOM bubbling → no Yew re-entrancy
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Live mirror of the state the recovery-timer's at-zero save needs. Refreshed
/// every render so the save closure — captured when the timer STARTS — reads
/// current data instead of a stale snapshot from the starting render.
#[derive(Default)]
struct RegLive {
    saved_sets: Vec<CompletedSet>,
    weight_inputs: HashMap<String, Vec<String>>,
    reps_inputs: HashMap<String, Vec<String>>,
    session_id: String,
    selected_exercise: usize,
    viewing_history: bool,
}

#[function_component(App)]
fn app() -> Html {
    let workout            = use_state(|| None::<Workout>);
    let error              = use_state(|| None::<String>);
    let day_index          = use_state(|| 0usize);
    let selected_exercise  = use_state(|| 0usize);
    let weight_inputs      = use_state(|| HashMap::<String, Vec<String>>::new());
    let reps_inputs        = use_state(|| HashMap::<String, Vec<String>>::new());
    let saved_sets         = use_state(|| Vec::<CompletedSet>::new());
    let catalog            = use_state(|| Vec::<CatalogEntry>::new());
    let catalog_loading    = use_state(|| true);
    let timer              = use_recovery_timer();
    let reader_task        = use_mut_ref(|| None::<FileReader>);
    let import_reader      = use_mut_ref(|| None::<FileReader>);
    let menu_open               = use_state(|| false);
    let history_open            = use_state(|| false);
    let viewing_history         = use_state(|| false);
    let show_completion         = use_state(|| false);
    let expand_trigger          = use_state(|| 0usize);
    let session_elapsed         = use_state(|| 0u32);
    let session_elapsed_handle  = use_mut_ref(|| None::<Interval>);
    let desc_expanded           = use_state(|| false);
    let user_preferred          = use_state(load_user_preferred);
    let cardio_elapsed          = use_state(|| 0u32);
    let cardio_running          = use_state(|| false);
    let cardio_handle           = use_mut_ref(|| None::<Interval>);
    // ID of the currently active session (empty = no workout loaded)
    let current_session_id  = use_state(|| String::new());
    // Non-empty when multiple open sessions exist and user must choose
    let resume_candidates: UseStateHandle<Vec<SessionMeta>> = use_state(Vec::new);

    // Exercise the recovery timer is counting down FOR. Set once on a fresh
    // start and kept across pause/resume + exercise navigation, so the at-zero
    // save always completes the right exercise's set.
    let timer_target = use_mut_ref(|| 0usize);

    // Live mirror (see RegLive) — refreshed below on every render.
    let reg_live = use_mut_ref(RegLive::default);
    *reg_live.borrow_mut() = RegLive {
        saved_sets: (*saved_sets).clone(),
        weight_inputs: (*weight_inputs).clone(),
        reps_inputs: (*reps_inputs).clone(),
        session_id: (*current_session_id).clone(),
        selected_exercise: *selected_exercise,
        viewing_history: *viewing_history,
    };

    // ── Export / Import ──────────────────────────────────────────────────────
    let on_export = Callback::from(move |_| {
        let json     = export_all_data();
        let filename = format!("allenamento_backup_{}.json", &now_iso()[..10]);
        trigger_download(&filename, &json);
    });

    let on_download_template = Callback::from(|_| {
        trigger_download("template_scheda.json", TEMPLATE_SCHEDA);
    });

    let on_download_schema = Callback::from(|_| {
        trigger_download("workout_schema.json", SCHEDA_SCHEMA);
    });

    let on_set_preferred = {
        let up = user_preferred.clone();
        Callback::from(move |file: Option<String>| {
            save_user_preferred(file.as_deref());
            up.set(file);
        })
    };

    let on_import_file = {
        let error         = error.clone();
        let import_reader = import_reader.clone();
        Callback::from(move |event: web_sys::Event| {
            let input = event.target()
                .and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
            if let Some(input) = input {
                if let Some(file) = input.files().and_then(|f| f.get(0)) {
                    let gloo_file = GlooFile::from(file);
                    let error     = error.clone();
                    let task = read_as_text(&gloo_file, move |result| match result {
                        Ok(json) => match import_all_data(&json) {
                            Ok(()) => {
                                // Reload the page so the restored data takes effect
                                if let Some(w) = web_sys::window() {
                                    let _ = w.location().reload();
                                }
                            }
                            Err(e) => error.set(Some(format!("Errore import: {}", e))),
                        },
                        Err(e) => error.set(Some(format!("Errore lettura file: {:?}", e))),
                    });
                    *import_reader.borrow_mut() = Some(task);
                }
            }
        })
    };

    // ── Fetch catalog ────────────────────────────────────────────────────────
    let _fetch_catalog = {
        let catalog = catalog.clone();
        let error   = error.clone();
        let loading = catalog_loading.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Request::get("schede/catalog.json").send().await {
                        Ok(resp) => {
                            if resp.ok() {
                                match resp.json::<Vec<CatalogEntry>>().await {
                                    Ok(list) => catalog.set(list),
                                    Err(e)   => error.set(Some(format!("Errore catalogo: {:?}", e))),
                                }
                            } else {
                                error.set(Some(format!("Errore caricamento catalogo: {}", resp.status())));
                            }
                            loading.set(false);
                        }
                        Err(e) => {
                            error.set(Some(format!("Errore caricamento catalogo: {:?}", e)));
                            loading.set(false);
                        }
                    }
                });
                || ()
            },
            (),
        )
    };

    // ── Shared session-state application ─────────────────────────────────────
    // Mirror a resolved DaySession into the day-level state handles. Single
    // source of truth for the "0 / 1 / many open sessions" branch that used to
    // be copy-pasted across auto-resume, day change, calendar select, and
    // suggestion entry points.
    let apply_day_session: Rc<dyn Fn(usize, DaySession)> = {
        let timer              = timer.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let current_session_id = current_session_id.clone();
        let resume_candidates  = resume_candidates.clone();
        Rc::new(move |day_idx: usize, outcome: DaySession| {
            // Switching day/session context abandons any running recovery:
            // its at-zero save would otherwise target an exercise index in the
            // NEW day (same bug class as the timer-target fix, one level up).
            timer.stop();
            day_index.set(day_idx);
            match outcome {
                DaySession::Fresh => {
                    selected_exercise.set(0);
                    saved_sets.set(vec![]);
                    weight_inputs.set(HashMap::new());
                    reps_inputs.set(HashMap::new());
                    current_session_id.set(String::new());
                    resume_candidates.set(vec![]);
                }
                DaySession::Resume { session_id, sets, active_exercise } => {
                    let (wi, ri) = inputs_from_sets(&sets);
                    selected_exercise.set(active_exercise);
                    saved_sets.set(sets);
                    weight_inputs.set(wi);
                    reps_inputs.set(ri);
                    current_session_id.set(session_id);
                    resume_candidates.set(vec![]);
                }
                DaySession::Disambiguate(candidates) => {
                    selected_exercise.set(0);
                    saved_sets.set(vec![]);
                    weight_inputs.set(HashMap::new());
                    reps_inputs.set(HashMap::new());
                    current_session_id.set(String::new());
                    resume_candidates.set(candidates);
                }
            }
        })
    };

    // Open a freshly-loaded workout at `day_idx`: resolve its session state and
    // clear any prior error. Used by file/catalog load, auto-resume, calendar
    // select, and suggestion entry points.
    let open_workout_fn: Rc<dyn Fn(Workout, usize)> = {
        let workout = workout.clone();
        let error   = error.clone();
        let apply   = apply_day_session.clone();
        Rc::new(move |data: Workout, day_idx: usize| {
            let day_label = data.giorni.get(day_idx)
                .map(|d| d.giorno.clone()).unwrap_or_default();
            apply(day_idx, resolve_day_session(&data.id, &day_label));
            workout.set(Some(data));
            error.set(None);
        })
    };

    // ── Auto-resume on mount ─────────────────────────────────────────────────
    // On startup, if the user has an open (unfinished) session, restore the
    // workout + day + sets from localStorage — no network request needed because
    // upsert_schedule() caches every loaded workout in "schedules".
    {
        let open_workout_fn = open_workout_fn.clone();
        use_effect_with_deps(
            move |_| {
                let open: Vec<SessionMeta> = load_sessions_index()
                    .into_iter()
                    .filter(|s| !s.done)
                    .collect();

                if let Some(meta) = open.iter().max_by_key(|s| &s.updated) {
                    if let Some(data) = load_schedules().into_iter().find(|w| w.id == meta.workout_id) {
                        let day_idx = data.giorni.iter()
                            .position(|d| d.giorno == meta.day).unwrap_or(0);
                        open_workout_fn(data, day_idx);
                    }
                }
                || ()
            },
            (),
        );
    }

    // Shared: parse a JSON string into a Workout and apply it to the app state.
    // Used by both on_file_change and on_load_catalog_entry.
    let apply_json: Rc<dyn Fn(String)> = {
        let error           = error.clone();
        let open_workout_fn = open_workout_fn.clone();
        Rc::new(move |text: String| {
            match serde_json::from_str::<Workout>(&text) {
                Ok(data) => {
                    upsert_schedule(&data);
                    open_workout_fn(data, 0);
                }
                Err(e) => error.set(Some(format!("Errore JSON: {}", e))),
            }
        })
    };

    // ── File upload ──────────────────────────────────────────────────────────
    let on_file_change = {
        let apply_json  = apply_json.clone();
        let error       = error.clone();
        let reader_task = reader_task.clone();
        Callback::from(move |event: web_sys::Event| {
            let input = event.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
            if let Some(input) = input {
                if let Some(file) = input.files().and_then(|f| f.get(0)) {
                    let gloo_file  = GlooFile::from(file);
                    let apply_json = apply_json.clone();
                    let error      = error.clone();
                    let task = read_as_text(&gloo_file, move |result| match result {
                        Ok(text) => apply_json(text),
                        Err(e)   => error.set(Some(format!("Errore lettura file: {:?}", e))),
                    });
                    *reader_task.borrow_mut() = Some(task);
                }
            }
        })
    };

    // ── Catalog entry ────────────────────────────────────────────────────────
    let on_load_catalog_entry = {
        let apply_json = apply_json.clone();
        let error      = error.clone();
        Callback::from(move |entry: CatalogEntry| {
            let apply_json = apply_json.clone();
            let error      = error.clone();
            let file_path  = entry.file.clone();
            spawn_local(async move {
                match Request::get(&file_path).send().await {
                    Ok(resp) if resp.ok() => {
                        match resp.text().await {
                            Ok(text) => apply_json(text),
                            Err(e)   => error.set(Some(format!("Errore caricamento file: {:?}", e))),
                        }
                    }
                    Ok(resp) => error.set(Some(format!("Errore caricamento file: {}", resp.status()))),
                    Err(e)   => error.set(Some(format!("Errore caricamento file: {:?}", e))),
                }
            });
        })
    };

    // ── Exercise / day selection ─────────────────────────────────────────────
    let on_select_exercise = {
        let selected_exercise = selected_exercise.clone();
        let expand_trigger    = expand_trigger.clone();
        let cardio_elapsed    = cardio_elapsed.clone();
        let cardio_running    = cardio_running.clone();
        let cardio_handle     = cardio_handle.clone();
        let workout           = workout.clone();
        let day_index         = day_index.clone();
        let saved_sets        = saved_sets.clone();
        Callback::from(move |idx: usize| {
            cardio_handle.borrow_mut().take();
            cardio_running.set(false);
            // Restore saved duration if this is a completed cardio exercise
            let restored = workout.as_ref()
                .and_then(|w| w.giorni.get(*day_index))
                .and_then(|d| d.esercizi.get(idx))
                .filter(|e| e.tipo.as_deref() == Some("cardio"))
                .and_then(|e| saved_sets.iter().find(|s| s.exercise_id == e.id && s.set_number == 1))
                .and_then(|s| s.durata_min)
                .map(|m| m * 60)
                .unwrap_or(0);
            cardio_elapsed.set(restored);
            selected_exercise.set(idx);
            expand_trigger.set(*expand_trigger + 1);
        })
    };

    let on_change_day = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let apply_day_session  = apply_day_session.clone();
        let cardio_elapsed     = cardio_elapsed.clone();
        let cardio_running     = cardio_running.clone();
        let cardio_handle      = cardio_handle.clone();
        Callback::from(move |idx: usize| {
            cardio_handle.borrow_mut().take();
            cardio_running.set(false);
            cardio_elapsed.set(0);
            match workout.as_ref()
                .and_then(|w| w.giorni.get(idx).map(|d| (w.id.clone(), d.giorno.clone())))
            {
                Some((wid, day)) => apply_day_session(idx, resolve_day_session(&wid, &day)),
                None             => day_index.set(idx),
            }
        })
    };

    // ── Input helpers ────────────────────────────────────────────────────────
    let on_weight_change = {
        let weight_inputs = weight_inputs.clone();
        Callback::from(move |(exercise_id, idx, value): (String, usize, String)| {
            weight_inputs.set(update_input_map((*weight_inputs).clone(), exercise_id, idx, value));
        })
    };

    let on_reps_change = {
        let reps_inputs = reps_inputs.clone();
        Callback::from(move |(exercise_id, idx, value): (String, usize, String)| {
            reps_inputs.set(update_input_map((*reps_inputs).clone(), exercise_id, idx, value));
        })
    };

    // ── Save set ─────────────────────────────────────────────────────────────
    let on_save_set = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let saved_sets         = saved_sets.clone();
        let current_session_id = current_session_id.clone();
        Callback::from(move |set_index: usize| {
            if let Some(workout) = &*workout {
                if let Some(day) = workout.giorni.get(*day_index) {
                    if let Some(exercise) = day.esercizi.get(*selected_exercise) {
                        // Manual save: the user explicitly picked this set, so use the
                        // weight exactly as typed (no fallback to a previous set).
                        let weight_str = weight_inputs
                            .get(&exercise.id)
                            .and_then(|v| v.get(set_index))
                            .cloned()
                            .unwrap_or_default();
                        let peso = weight_str.parse::<f32>().ok();
                        let reps = reps_inputs
                            .get(&exercise.id)
                            .and_then(|v| v.get(set_index))
                            .cloned();
                        let current_idx = *selected_exercise;
                        let reg = register_set(
                            workout, day, *day_index, exercise, current_idx,
                            (set_index + 1) as u32, peso, reps, &weight_str,
                            (*saved_sets).clone(), &weight_inputs, &current_session_id,
                        );
                        if reg.session_created { current_session_id.set(reg.session_id); }
                        if reg.next_active_exercise != current_idx {
                            selected_exercise.set(reg.next_active_exercise);
                        }
                        if let Some((idx, val)) = reg.prefill_weight {
                            weight_inputs.set(update_input_map(
                                (*weight_inputs).clone(), exercise.id.clone(), idx, val,
                            ));
                        }
                        saved_sets.set(reg.sets);
                    }
                }
            }
        })
    };

    // ── Cardio stopwatch: toggle (start / pause / resume) ───────────────────
    let on_cardio_toggle = {
        let cardio_elapsed = cardio_elapsed.clone();
        let cardio_running = cardio_running.clone();
        let cardio_handle  = cardio_handle.clone();
        Callback::from(move |_: ()| {
            if *cardio_running {
                cardio_handle.borrow_mut().take();
                cardio_running.set(false);
            } else {
                // Wall-clock anchored (same method as the recovery timer's clock):
                // recompute from `Date.now()` each tick so the elapsed time stays
                // correct after the device sleeps / screen locks in background.
                let base = *cardio_elapsed;
                let anchor = js_sys::Date::now();
                let ce = cardio_elapsed.clone();
                let h = Interval::new(1000, move || {
                    let n = base + ((js_sys::Date::now() - anchor).max(0.0) / 1000.0) as u32;
                    ce.set(n);
                });
                *cardio_handle.borrow_mut() = Some(h);
                cardio_running.set(true);
            }
        })
    };

    // ── Cardio stopwatch: stop + save ────────────────────────────────────────
    let on_cardio_stop = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let current_session_id = current_session_id.clone();
        let cardio_elapsed     = cardio_elapsed.clone();
        let cardio_running     = cardio_running.clone();
        let cardio_handle      = cardio_handle.clone();
        Callback::from(move |_: ()| {
            cardio_handle.borrow_mut().take();
            cardio_running.set(false);
            let elapsed = *cardio_elapsed;
            if let Some(workout) = &*workout {
                if let Some(day) = workout.giorni.get(*day_index) {
                    if let Some(exercise) = day.esercizi.get(*selected_exercise) {
                        let durata_min = Some(elapsed / 60);
                        let list = upsert_completed_set(
                            (*saved_sets).clone(), exercise, 1, None, None, durata_min,
                        );
                        let current_idx = *selected_exercise;
                        let ex_done = list.iter()
                            .filter(|s| s.exercise_id == exercise.id)
                            .count() >= exercise.serie as usize;
                        let next_active = if ex_done {
                            let next = next_incomplete_exercise(day, &list, current_idx);
                            if next != current_idx { selected_exercise.set(next); }
                            next
                        } else { current_idx };
                        let sid = {
                            let cur = (*current_session_id).clone();
                            if cur.is_empty() {
                                let new_sid = create_session_for_day(workout, *day_index);
                                current_session_id.set(new_sid.clone());
                                new_sid
                            } else { cur }
                        };
                        let total = total_day_sets(workout, &day.giorno);
                        update_session_sets(&workout.id, &sid, &list, next_active, total);
                        saved_sets.set(list);
                    }
                }
            }
            cardio_elapsed.set(0);
        })
    };

    // ── Register the next incomplete set of a SPECIFIC exercise ───────────────
    // Shared by skip-timer and the recovery timer's at-zero auto-save. Takes the
    // target exercise index explicitly (the recovery timer captures it when it
    // STARTS, so the right set is completed even if the user has since navigated
    // to another exercise). Reads current data from `reg_live` (not a stale
    // render snapshot) and only moves the view if the user is still on `ex_idx`.
    let register_set_for: Rc<dyn Fn(usize)> = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let weight_inputs      = weight_inputs.clone();
        let current_session_id = current_session_id.clone();
        let reg_live           = reg_live.clone();
        Rc::new(move |ex_idx: usize| {
            let Some(workout) = &*workout else { return };
            let Some(day) = workout.giorni.get(*day_index) else { return };
            let Some(exercise) = day.esercizi.get(ex_idx) else { return };

            // Snapshot the live state once (current as of the last render).
            let (sets, wi_live, ri_live, sid_live, cur_selected) = {
                let live = reg_live.borrow();
                // Never auto-save while viewing a terminated session: the live
                // session_id points at the HISTORY session and writing to it
                // would corrupt it. (Belt-and-braces: entering history also
                // stops the timer.)
                if live.viewing_history { return; }
                (live.saved_sets.clone(), live.weight_inputs.clone(),
                 live.reps_inputs.clone(), live.session_id.clone(),
                 live.selected_exercise)
            };

            let existing: HashSet<u32> = sets.iter()
                .filter(|s| s.exercise_id == exercise.id)
                .map(|s| s.set_number)
                .collect();
            let next_set = (1..=exercise.serie)
                .find(|n| !existing.contains(n))
                .unwrap_or(existing.len() as u32 + 1);
            if next_set > exercise.serie { return; }
            let next_idx = (next_set - 1) as usize;
            // Auto-save: the user didn't necessarily touch the field, so fall
            // back to the most recent weight entered for this exercise.
            let weight_str = get_input_with_fallback(&wi_live, &exercise.id, next_idx, "");
            let peso = weight_str.parse::<f32>().ok();
            let reps = ri_live.get(&exercise.id)
                .and_then(|v| v.get(next_idx))
                .cloned();
            let reg = register_set(
                workout, day, *day_index, exercise, ex_idx,
                next_set, peso, reps, &weight_str,
                sets, &wi_live, &sid_live,
            );
            if reg.session_created { current_session_id.set(reg.session_id); }
            // Only advance the view if the user is still on the exercise saved.
            if cur_selected == ex_idx && reg.next_active_exercise != ex_idx {
                selected_exercise.set(reg.next_active_exercise);
            }
            if let Some((idx, val)) = reg.prefill_weight {
                weight_inputs.set(update_input_map(
                    wi_live, exercise.id.clone(), idx, val,
                ));
            }
            saved_sets.set(reg.sets);
        })
    };

    // ── Cancel timer (called by ExerciseCard when set is manually registered) ──
    let on_cancel_timer = {
        let timer = timer.clone();
        Callback::from(move |_: ()| timer.stop())
    };

    // ── Skip timer — stop + immediately save the set being recovered ──────────
    let on_skip_timer = {
        let timer        = timer.clone();
        let register     = register_set_for.clone();
        let timer_target = timer_target.clone();
        Callback::from(move |_: ()| {
            let target = *timer_target.borrow();
            timer.stop();
            register(target);
        })
    };

    // ── Recovery timer (toggle: start / pause / resume) ──────────────────────
    let on_start_timer = {
        let timer             = timer.clone();
        let workout_state     = workout.clone();
        let day_index         = day_index.clone();
        let selected_exercise = selected_exercise.clone();
        let register          = register_set_for.clone();
        let timer_target      = timer_target.clone();
        Callback::from(move |_: ()| {
            // Only a FRESH start (re)binds the target. Pause and resume keep the
            // exercise the timer was originally started for, so navigating away
            // (or pausing to peek at another exercise) never re-targets it.
            let is_fresh = !timer.running && timer.left == 0;
            if is_fresh {
                *timer_target.borrow_mut() = *selected_exercise;
            }
            let target = *timer_target.borrow();
            let fresh_secs = workout_state.as_ref()
                .and_then(|w| w.giorni.get(*day_index))
                .and_then(|d| d.esercizi.get(target))
                .and_then(|e| e.recupero)
                .unwrap_or(90);
            let register = register.clone();
            let tt = timer_target.clone();
            timer.toggle(fresh_secs, move || register(*tt.borrow()));
        })
    };

    // ── Dialog: resume a specific session ───────────────────────────────────
    let on_resume_session = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let current_session_id = current_session_id.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let resume_candidates  = resume_candidates.clone();
        Callback::from(move |meta: SessionMeta| {
            if let Some(w) = &*workout {
                // Delete other open sessions for this same day (keep only the chosen one)
                let others: Vec<_> = open_sessions_for_day(&w.id, &meta.day)
                    .into_iter()
                    .filter(|m| m.id != meta.id)
                    .collect();
                for m in others { delete_session(&w.id, &m.id); }

                let (sid, sets, active_ex) = find_open_session(&w.id, &meta.day)
                    .unwrap_or((String::new(), vec![], 0));
                let day_idx = w.giorni.iter()
                    .position(|d| d.giorno == meta.day)
                    .unwrap_or(0);
                let (wi, ri) = inputs_from_sets(&sets);
                day_index.set(day_idx);
                selected_exercise.set(active_ex);
                saved_sets.set(sets);
                current_session_id.set(sid);
                weight_inputs.set(wi);
                reps_inputs.set(ri);
            }
            resume_candidates.set(vec![]);
        })
    };

    // ── Dialog: delete sessions for this day and start fresh ─────────────────
    let on_discard_and_new = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let current_session_id = current_session_id.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let resume_candidates  = resume_candidates.clone();
        Callback::from(move |_| {
            if let Some(w) = &*workout {
                // Delete only the open sessions for the day being disambiguated
                if let Some(candidate) = (*resume_candidates).first() {
                    let day_label = candidate.day.clone();
                    delete_sessions_for_day(&w.id, &day_label);
                    let day_idx = w.giorni.iter()
                        .position(|d| d.giorno == day_label)
                        .unwrap_or(0);
                    day_index.set(day_idx);
                }
                // Fresh start: session will be created lazily on first set
                selected_exercise.set(0);
                saved_sets.set(vec![]);
                current_session_id.set(String::new());
                weight_inputs.set(HashMap::new());
                reps_inputs.set(HashMap::new());
            }
            resume_candidates.set(vec![]);
        })
    };

    // Shared: reset all workout-related state handles to their initial values.
    // Used by on_save_and_finish, on_delete_workout, and clear_workout.
    let reset_workout_state: Rc<dyn Fn()> = {
        let timer                  = timer.clone();
        let workout                = workout.clone();
        let error                  = error.clone();
        let day_index              = day_index.clone();
        let selected_exercise      = selected_exercise.clone();
        let saved_sets             = saved_sets.clone();
        let weight_inputs          = weight_inputs.clone();
        let reps_inputs            = reps_inputs.clone();
        let current_session_id     = current_session_id.clone();
        let resume_candidates      = resume_candidates.clone();
        let viewing_history        = viewing_history.clone();
        let show_completion        = show_completion.clone();
        let session_elapsed        = session_elapsed.clone();
        let session_elapsed_handle = session_elapsed_handle.clone();
        let desc_expanded          = desc_expanded.clone();
        let cardio_elapsed         = cardio_elapsed.clone();
        let cardio_running         = cardio_running.clone();
        let cardio_handle          = cardio_handle.clone();
        Rc::new(move || {
            timer.stop();
            cardio_handle.borrow_mut().take();
            cardio_running.set(false);
            cardio_elapsed.set(0);
            session_elapsed_handle.borrow_mut().take();
            session_elapsed.set(0);
            workout.set(None);
            error.set(None);
            day_index.set(0);
            selected_exercise.set(0);
            saved_sets.set(Vec::new());
            weight_inputs.set(HashMap::new());
            reps_inputs.set(HashMap::new());
            current_session_id.set(String::new());
            resume_candidates.set(vec![]);
            viewing_history.set(false);
            show_completion.set(false);
            desc_expanded.set(false);
        })
    };

    // ── Save and finish ──────────────────────────────────────────────────────
    let on_save_and_finish = {
        let workout            = workout.clone();
        let current_session_id = current_session_id.clone();
        let reset              = reset_workout_state.clone();
        Callback::from(move |_| {
            if let Some(w) = &*workout {
                let sid = (*current_session_id).clone();
                if !sid.is_empty() { terminate_session(&w.id, &sid); }
            }
            reset();
        })
    };

    // ── Cancel / delete current workout session ──────────────────────────────
    let on_delete_workout = {
        let workout            = workout.clone();
        let current_session_id = current_session_id.clone();
        let reset              = reset_workout_state.clone();
        Callback::from(move |_| {
            if let Some(w) = &*workout {
                let sid = (*current_session_id).clone();
                if !sid.is_empty() { delete_session(&w.id, &sid); }
            }
            reset();
        })
    };

    // ── Clear workout (go back to catalog without terminating the session) ───
    let clear_workout = {
        let reset = reset_workout_state.clone();
        Callback::from(move |_| reset())
    };

    // ── History callbacks ────────────────────────────────────────────────────
    let on_open_history = {
        let menu_open    = menu_open.clone();
        let history_open = history_open.clone();
        Callback::from(move |_| { menu_open.set(false); history_open.set(true); })
    };

    let on_close_history = {
        let history_open = history_open.clone();
        Callback::from(move |_| history_open.set(false))
    };

    let on_view_session = {
        let timer              = timer.clone();
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let current_session_id = current_session_id.clone();
        let viewing_history    = viewing_history.clone();
        let history_open       = history_open.clone();
        Callback::from(move |session: Session| {
            // Entering history abandons any running recovery (its auto-save
            // would write into the terminated session being viewed).
            timer.stop();
            if let Some(w) = &*workout {
                let day_idx = w.giorni.iter()
                    .position(|d| d.giorno == session.day)
                    .unwrap_or(0);
                day_index.set(day_idx);
            }
            let (wi, ri) = inputs_from_sets(&session.sets);
            selected_exercise.set(session.active_exercise);
            saved_sets.set(session.sets.clone());
            weight_inputs.set(wi);
            reps_inputs.set(ri);
            current_session_id.set(session.id.clone());
            viewing_history.set(true);
            history_open.set(false);
        })
    };

    let on_exit_history = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let current_session_id = current_session_id.clone();
        let viewing_history    = viewing_history.clone();
        Callback::from(move |_| {
            if let Some(w) = &*workout {
                if let Some(day) = w.giorni.get(*day_index) {
                    match find_open_session(&w.id, &day.giorno) {
                        Some((sid, sets, active_ex)) => {
                            let (wi, ri) = inputs_from_sets(&sets);
                            saved_sets.set(sets);
                            weight_inputs.set(wi);
                            reps_inputs.set(ri);
                            current_session_id.set(sid);
                            selected_exercise.set(active_ex);
                        }
                        None => {
                            saved_sets.set(vec![]);
                            weight_inputs.set(HashMap::new());
                            reps_inputs.set(HashMap::new());
                            current_session_id.set(String::new());
                            selected_exercise.set(0);
                        }
                    }
                }
            }
            viewing_history.set(false);
        })
    };

    // ── Calendar: select session from dot / CTA ──────────────────────────────
    let on_select_session_meta = {
        let timer              = timer.clone();
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let current_session_id = current_session_id.clone();
        let viewing_history    = viewing_history.clone();
        let error              = error.clone();
        let open_workout_fn    = open_workout_fn.clone();
        Callback::from(move |meta: SessionMeta| {
            let Some(data) = load_schedules().into_iter().find(|w| w.id == meta.workout_id)
                else { return };
            let day_idx = data.giorni.iter().position(|d| d.giorno == meta.day).unwrap_or(0);
            if meta.done {
                if let Some(session) = find_session_by_id(&meta.workout_id, &meta.id) {
                    timer.stop(); // entering history view — abandon any recovery
                    let (wi, ri) = inputs_from_sets(&session.sets);
                    day_index.set(day_idx);
                    selected_exercise.set(session.active_exercise);
                    saved_sets.set(session.sets.clone());
                    weight_inputs.set(wi);
                    reps_inputs.set(ri);
                    current_session_id.set(session.id.clone());
                    viewing_history.set(true);
                    workout.set(Some(data));
                    error.set(None);
                }
            } else {
                open_workout_fn(data, day_idx);
            }
        })
    };

    let on_open_suggestion = {
        let catalog         = catalog.clone();
        let user_preferred  = user_preferred.clone();
        let open_workout_fn = open_workout_fn.clone();
        Callback::from(move |_| {
            let sessions  = load_sessions_index();
            let schedules = load_schedules();
            let Some((data, day_idx)) = compute_suggestion_workout(
                &sessions, &schedules, &catalog, &user_preferred,
            ) else { return };
            open_workout_fn(data, day_idx);
        })
    };

    // ── Wake Lock ────────────────────────────────────────────────────────────
    // Acquire when a workout is loaded, release when the user exits.
    // The browser auto-releases the lock whenever the page goes hidden (screen
    // off, app switch), so a visibilitychange listener re-acquires it on
    // return to foreground — otherwise the lock is silently lost for the rest
    // of the session after the first screen-off.
    {
        let slot: Rc<RefCell<Option<JsValue>>> = use_mut_ref(|| None);
        use_effect_with_deps(
            move |is_loaded: &bool| {
                let mut vis_cb: Option<Closure<dyn Fn()>> = None;
                if *is_loaded {
                    acquire_wake_lock(slot.clone());
                    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                        let slot2 = slot.clone();
                        let cb = Closure::<dyn Fn()>::wrap(Box::new(move || {
                            let visible = web_sys::window()
                                .and_then(|w| w.document())
                                .map(|d| !d.hidden())
                                .unwrap_or(false);
                            if visible { acquire_wake_lock(slot2.clone()); }
                        }));
                        let _ = doc.add_event_listener_with_callback(
                            "visibilitychange",
                            cb.as_ref().unchecked_ref(),
                        );
                        vis_cb = Some(cb);
                    }
                } else {
                    release_wake_lock(&slot);
                }
                move || {
                    if let Some(cb) = vis_cb {
                        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                            let _ = doc.remove_event_listener_with_callback(
                                "visibilitychange",
                                cb.as_ref().unchecked_ref(),
                            );
                        }
                    }
                }
            },
            workout.is_some(),
        );
    }


    // ── Session elapsed timer ────────────────────────────────────────────────
    {
        let elapsed = session_elapsed.clone();
        let handle  = session_elapsed_handle.clone();
        let wk      = workout.clone();
        use_effect_with_deps(
            move |(sid, in_history): &(String, bool)| {
                handle.borrow_mut().take();
                if sid.is_empty() || *in_history {
                    elapsed.set(0);
                } else {
                    let session_info = wk.as_ref()
                        .and_then(|w| {
                            load_sessions(&w.id)
                                .into_iter()
                                .find(|s| &s.id == sid)
                                .map(|s| {
                                    let started_ms = js_sys::Date::parse(&s.started);
                                    let initial = ((js_sys::Date::now() - started_ms).max(0.0) / 1000.0) as u32;
                                    (initial, started_ms)
                                })
                        });
                    let (initial, started_ms) = session_info.unwrap_or((0, 0.0));
                    elapsed.set(initial);
                    let elapsed2 = elapsed.clone();
                    let h = Interval::new(1000, move || {
                        let n = ((js_sys::Date::now() - started_ms) / 1000.0) as u32;
                        elapsed2.set(n);
                    });
                    *handle.borrow_mut() = Some(h);
                }
                || ()
            },
            ((*current_session_id).clone(), *viewing_history),
        );
    }


    // Pre-compute session progress (done / total sets for current day)
    let session_done: usize = saved_sets.len();
    let session_total: usize = workout.as_ref()
        .and_then(|w| w.giorni.get(*day_index))
        .map(|d| d.esercizi.iter().map(|e| e.serie as usize).sum())
        .unwrap_or(0);
    let all_done = session_total > 0
        && session_done >= session_total
        && !(*current_session_id).is_empty()
        && !*viewing_history;

    // Pre-compute completion stats (only when all sets done)
    let completion_duration: String = if all_done {
        let wid = workout.as_ref().map(|w| w.id.clone()).unwrap_or_default();
        let sid = (*current_session_id).clone();
        load_sessions(&wid)
            .into_iter()
            .find(|s| s.id == sid)
            .map(|s| {
                let diff_ms = (js_sys::Date::now() - js_sys::Date::parse(&s.started)).max(0.0);
                let mins = (diff_ms / 60000.0) as u32;
                let secs = ((diff_ms % 60000.0) / 1000.0) as u32;
                format!("{}:{:02}", mins, secs)
            })
            .unwrap_or_else(|| "-".to_string())
    } else {
        String::new()
    };
    let completion_weight: f32 = if all_done {
        saved_sets.iter().filter_map(|s| s.peso).sum()
    } else { 0.0 };

    // Auto-open completion modal when the last set is registered
    {
        let sc = show_completion.clone();
        use_effect_with_deps(
            move |done: &bool| {
                if *done { sc.set(true); }
                || ()
            },
            all_done,
        );
    }

    // Pre-compute session elapsed display string
    let elapsed_str: String = {
        let s = *session_elapsed;
        if s > 0 { format!("{}:{:02}", s / 60, s % 60) } else { String::new() }
    };

    // Pre-compute timer circle dashoffset (2π × r=19 ≈ 119.38)
    let timer_dashoffset: String = {
        const CIRCUM: f64 = 119.38;
        if timer.total > 0 {
            let frac = timer.left as f64 / timer.total as f64;
            format!("{:.2}", CIRCUM * (1.0 - frac))
        } else {
            format!("{:.2}", CIRCUM)
        }
    };

    // Pre-compute calendar data (only when on catalog screen to avoid unnecessary reads)
    let cal_all_sessions = if workout.is_none() { load_sessions_index() } else { vec![] };
    let cal_open_session = cal_all_sessions.iter()
        .filter(|s| !s.done)
        .max_by_key(|s| &s.updated)
        .cloned();
    let cal_suggestion = if workout.is_none() {
        compute_suggestion(&cal_all_sessions, &load_schedules(), &catalog, &user_preferred)
    } else { None };

    // Pre-compute selected exercise for bottom sheet
    let sheet_exercise: Option<crate::models::Exercise> = workout.as_ref()
        .and_then(|w| w.giorni.get(*day_index))
        .and_then(|d| d.esercizi.get(*selected_exercise))
        .cloned();
    let sheet_day: Option<crate::models::Day> = workout.as_ref()
        .and_then(|w| w.giorni.get(*day_index))
        .cloned();
    let sheet_workout_id: String = workout.as_ref()
        .map(|w| w.id.clone())
        .unwrap_or_default();

    // Pre-compute history sessions (needed in render, can't use let inside html!)
    let history_sessions: Vec<Session> = if *history_open {
        if let Some(w) = &*workout {
            if let Some(day) = w.giorni.get(*day_index) {
                terminated_sessions_for_day(&w.id, &day.giorno)
            } else { vec![] }
        } else { vec![] }
    } else { vec![] };

    let on_close_completion = {
        let sc = show_completion.clone();
        Callback::from(move |_: MouseEvent| sc.set(false))
    };

    let on_toggle_desc = {
        let de = desc_expanded.clone();
        Callback::from(move |_: MouseEvent| de.set(!*de))
    };

    // ── Render ───────────────────────────────────────────────────────────────
    let open_menu  = { let m = menu_open.clone(); Callback::from(move |_| m.set(true))  };
    let close_menu = { let m = menu_open.clone(); Callback::from(move |_| m.set(false)) };

    html! {
        <div class="app-shell">
            <header class="app-header">
                <button class="burger-btn" onclick={open_menu} title="Menu">
                    <span></span><span></span><span></span>
                </button>
                <div>
                    <h1>{"Allenamento WASM"}</h1>
                    <p>{"Carica una scheda JSON e segui l'allenamento in tempo reale."}</p>
                </div>
                if workout.is_some() {
                    <button class="clear-button" onclick={clear_workout}>{"Carica un'altra scheda"}</button>
                }
            </header>

            <main class="app-main">
                {
                    if let Some(workout_data) = &*workout {
                        if !resume_candidates.is_empty() {
                            // ── Resume-session dialog (rendered inside app-main) ──
                            html! {
                                <div class="resume-dialog-wrap">
                                    <div class="resume-dialog">
                                        <h2>{"Sessioni in corso"}</h2>
                                        <p class="resume-dialog-sub">
                                            { format!("Hai {} sessioni non terminate per «{}». Scegli cosa fare:",
                                              resume_candidates.len(), workout_data.nome) }
                                        </p>
                                        <div class="resume-options">
                                            { for resume_candidates.iter().map(|meta| {
                                                let m = meta.clone();
                                                let on_resume = on_resume_session.clone();
                                                let date = meta.started.get(..16)
                                                    .unwrap_or(&meta.started)
                                                    .replace('T', " ");
                                                html! {
                                                    <div class="resume-option">
                                                        <div class="resume-option-info">
                                                            <div class="resume-option-day">{ &meta.day }</div>
                                                            <div class="resume-option-meta">
                                                                { format!("{} · {:.0}% completato", date, meta.completion_pct) }
                                                            </div>
                                                        </div>
                                                        <button class="primary-button"
                                                            onclick={Callback::from(move |_| on_resume.emit(m.clone()))}>
                                                            {"Riprendi"}
                                                        </button>
                                                    </div>
                                                }
                                            }) }
                                        </div>
                                        <button class="secondary-button" style="width:100%;margin-top:8px;"
                                            onclick={on_discard_and_new.clone()}>
                                            {"Inizia nuovo allenamento"}
                                        </button>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <WorkoutView
                                    workout={workout_data.clone()}
                                    day_index={*day_index}
                                    selected_exercise={*selected_exercise}
                                    saved_sets={(*saved_sets).clone()}
                                    viewing_history={*viewing_history}
                                    desc_expanded={*desc_expanded}
                                    session_done={session_done}
                                    session_total={session_total}
                                    all_done={all_done}
                                    elapsed_str={elapsed_str.clone()}
                                    on_exit_history={on_exit_history.clone()}
                                    on_toggle_desc={on_toggle_desc.clone()}
                                    on_change_day={on_change_day.clone()}
                                    on_select_exercise={on_select_exercise.clone()}
                                    on_save_and_finish={on_save_and_finish.clone()}
                                    on_delete_workout={on_delete_workout.clone()}
                                />
                            }
                        } // close else (not resume dialog)
                    } else {
                        html! {
                            <>
                            <Calendar
                                sessions={cal_all_sessions}
                                open_session={cal_open_session}
                                suggestion={cal_suggestion}
                                on_select_session={on_select_session_meta.clone()}
                                on_open_suggestion={on_open_suggestion.clone()}
                            />
                            <CatalogPanel
                                catalog={(*catalog).clone()}
                                catalog_loading={*catalog_loading}
                                on_load_catalog_entry={on_load_catalog_entry}
                                on_file_change={on_file_change}
                                user_preferred={(*user_preferred).clone()}
                                on_set_preferred={on_set_preferred}
                            />
                            </>
                        }
                    }
                }
                if let Some(error_msg) = &*error {
                    <div class="error-banner">{ error_msg }</div>
                }
            </main>

            // ── Bottom sheet — selected exercise detail ───────────────────────
            <BottomSheet
                exercise={sheet_exercise}
                day={sheet_day}
                saved_sets={(*saved_sets).clone()}
                weight_inputs={(*weight_inputs).clone()}
                reps_inputs={(*reps_inputs).clone()}
                on_save_set={on_save_set.clone()}
                on_weight_change={on_weight_change.clone()}
                on_reps_change={on_reps_change.clone()}
                on_start_timer={on_start_timer.clone()}
                on_cancel_timer={on_cancel_timer.clone()}
                timer={TimerState { running: timer.running, left: timer.left, total: timer.total }}
                history_mode={*viewing_history}
                workout_id={sheet_workout_id}
                selected_exercise_idx={*selected_exercise}
                on_select_exercise={on_select_exercise.clone()}
                expand_trigger={*expand_trigger}
                cardio_elapsed={*cardio_elapsed}
                cardio_running={*cardio_running}
                on_cardio_toggle={on_cardio_toggle}
                on_cardio_stop={on_cardio_stop}
            />

            // ── Burger menu modal ────────────────────────────────────────────
            if *menu_open {
                <div class="menu-overlay" onclick={close_menu.clone()}>
                    <div class="menu-modal"
                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <div class="menu-modal-header">
                            <span class="menu-modal-title">{"Menu"}</span>
                            <button class="menu-close-btn" onclick={close_menu}>{ icon_x() }</button>
                        </div>
                        if workout.is_some() {
                            <>
                                <div class="menu-section-title">{"Sessione"}</div>
                                <button class="menu-action-btn"
                                    onclick={on_open_history.clone()}>
                                    <span class="menu-action-icon">{ icon_clock() }</span>
                                    {"Storico sessioni"}
                                </button>
                            </>
                        }
                        <div class="menu-section-title">{"Portabilità dati"}</div>
                        <button class="menu-action-btn" onclick={on_export}>
                            <span class="menu-action-icon">{ icon_download() }</span>
                            {"Esporta backup"}
                        </button>
                        <label class="menu-action-btn menu-action-btn--file">
                            <span class="menu-action-icon">{ icon_upload() }</span>
                            <span>{"Importa backup"}</span>
                            <input type="file" accept=".json" onchange={on_import_file} />
                        </label>
                        <p class="menu-hint">
                            {"Esporta tutte le schede e sessioni. L'importazione sovrascrive i dati esistenti."}
                        </p>
                        <div class="menu-section-title">{"Crea scheda"}</div>
                        <button class="menu-action-btn" onclick={on_download_template}>
                            <span class="menu-action-icon">{ icon_document() }</span>
                            {"Scarica template JSON"}
                        </button>
                        <button class="menu-action-btn" onclick={on_download_schema}>
                            <span class="menu-action-icon">{ icon_code() }</span>
                            {"Scarica JSON Schema"}
                        </button>
                        <p class="menu-hint">
                            {"Il template è un esempio completo da modificare. Lo schema permette la validazione e l'autocomplete in VS Code."}
                        </p>
                        <div class="local-data-indicator">
                            <span class="local-data-dot"></span>
                            {"Dati salvati localmente sul dispositivo"}
                        </div>
                    </div>
                </div>
            }

            // ── Timer toast (fixed, top of viewport) ─────────────────────────
            if timer.running || timer.left > 0 {
                <div class="timer-toast">
                    // Circular countdown
                    <svg viewBox="0 0 50 50" width="50" height="50" style="flex-shrink:0">
                        <circle class="timer-ring-bg" cx="25" cy="25" r="19"
                                fill="none" stroke="#dbeafe" stroke-width="3.5"/>
                        <circle class="timer-ring-arc" cx="25" cy="25" r="19"
                                fill="none" stroke="#2563eb" stroke-width="3.5"
                                stroke-linecap="round"
                                stroke-dasharray="119.38"
                                stroke-dashoffset={timer_dashoffset}
                                transform="rotate(-90, 25, 25)"/>
                        <text class="timer-ring-text" x="25" y="30" text-anchor="middle"
                              font-size="13" font-weight="700" fill="#111">
                            { format!("{}s", timer.left) }
                        </text>
                    </svg>
                    <span class="timer-toast-label">
                        { if timer.running { "Recupero" } else { "In pausa" } }
                    </span>
                    <div class="timer-toast-actions">
                        // Pause / Resume
                        <button class="timer-action-btn" title="Pausa / Riprendi"
                                onclick={{ let cb = on_start_timer.clone(); Callback::from(move |_| cb.emit(())) }}>
                            { if timer.running { icon_pause() } else { icon_play() } }
                        </button>
                        <button class="timer-action-btn timer-action-btn--skip" title="Salta e registra"
                                onclick={{ let cb = on_skip_timer.clone(); Callback::from(move |_| cb.emit(())) }}>
                            { icon_skip() }
                        </button>
                        <button class="timer-action-btn timer-action-btn--stop" title="Annulla"
                                onclick={{ let cb = on_cancel_timer.clone(); Callback::from(move |_| cb.emit(())) }}>
                            { icon_x() }
                        </button>
                    </div>
                </div>
            }

            // ── Workout completion modal ──────────────────────────────────────
            if *show_completion {
                <div class="completion-overlay"
                    onclick={on_close_completion.clone()}>
                    <div class="completion-modal"
                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <div class="completion-modal-title">{"Allenamento completato!"}</div>
                        <div class="completion-stats">
                            <div class="completion-stat">
                                <span class="completion-stat-value">{ session_done.to_string() }</span>
                                <span class="completion-stat-label">{"Serie"}</span>
                            </div>
                            <div class="completion-stat">
                                <span class="completion-stat-value">
                                    { format!("{:.0}kg", completion_weight) }
                                </span>
                                <span class="completion-stat-label">{"Peso totale"}</span>
                            </div>
                            <div class="completion-stat">
                                <span class="completion-stat-value">{ completion_duration.clone() }</span>
                                <span class="completion-stat-label">{"Durata"}</span>
                            </div>
                        </div>
                        <div class="completion-modal-actions">
                            <button class="primary-button completion-save-btn"
                                onclick={{
                                    let close = on_close_completion.clone();
                                    let save  = on_save_and_finish.clone();
                                    Callback::from(move |e: MouseEvent| {
                                        close.emit(e.clone());
                                        save.emit(e);
                                    })
                                }}>
                                {"Salva e termina"}
                            </button>
                            <button class="secondary-button" onclick={on_close_completion.clone()}>
                                {"Rivedi l'allenamento"}
                            </button>
                        </div>
                    </div>
                </div>
            }

            // ── History sessions modal ────────────────────────────────────────
            if *history_open {
                <div class="menu-overlay" onclick={on_close_history.clone()}>
                    <div class="menu-modal"
                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <div class="menu-modal-header">
                            <span class="menu-modal-title">{"Storico sessioni"}</span>
                            <button class="menu-close-btn"
                                onclick={on_close_history}>{ icon_x() }</button>
                        </div>
                        { if history_sessions.is_empty() {
                            html! { <p class="menu-hint">{"Nessuna sessione terminata per questo giorno."}</p> }
                        } else {
                            html! {
                                <div class="history-session-list">
                                    { for history_sessions.into_iter().map(|session| {
                                        let on_view = on_view_session.clone();
                                        let date = session.started
                                            .get(..16).unwrap_or(&session.started)
                                            .replace('T', " ");
                                        let count = session.sets.len();
                                        let s = session.clone();
                                        html! {
                                            <button class="history-session-item"
                                                onclick={Callback::from(move |_| on_view.emit(s.clone()))}>
                                                <div class="history-session-date">{ date }</div>
                                                <div class="history-session-meta">
                                                    { format!("{} serie registrate", count) }
                                                </div>
                                            </button>
                                        }
                                    }) }
                                </div>
                            }
                        } }
                    </div>
                </div>
            }
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    yew::Renderer::<App>::new().render();
}
