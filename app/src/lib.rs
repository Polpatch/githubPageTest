mod components;
mod models;

use components::catalog_panel::CatalogPanel;
use components::day_tabs::DayTabs;
use components::exercise_card::ExerciseCard;
use components::session_history::SessionHistory;
use gloo_file::callbacks::{read_as_text, FileReader};
use gloo_file::File as GlooFile;
use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use models::*;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

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
    let timer_running      = use_state(|| false);
    let timer_left         = use_state(|| 0u32);
    let timer_total        = use_state(|| 0u32);
    let timer_handle       = use_mut_ref(|| None::<Interval>);
    let reader_task        = use_mut_ref(|| None::<FileReader>);
    // ID of the currently active session (empty = no workout loaded)
    let current_session_id = use_state(|| String::new());

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

    // ── Shared workout-open logic ────────────────────────────────────────────
    // Called after a Workout is successfully parsed (file or catalog).
    // Saves/updates the schedule, finds or creates the session for day 0,
    // and updates all relevant state handles.
    macro_rules! open_workout {
        ($data:expr,
         $workout:expr, $error:expr, $selected_exercise:expr,
         $saved_sets:expr, $current_session_id:expr) => {{
            let data: Workout = $data;
            upsert_schedule(&data);
            let (sid, sets, active_ex) = find_or_create_session(&data, 0);
            $workout.set(Some(data));
            $error.set(None);
            $selected_exercise.set(active_ex);
            $saved_sets.set(sets);
            $current_session_id.set(sid);
        }};
    }

    // ── File upload ──────────────────────────────────────────────────────────
    let on_file_change = {
        let workout            = workout.clone();
        let error              = error.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let current_session_id = current_session_id.clone();
        let reader_task        = reader_task.clone();
        Callback::from(move |event: web_sys::Event| {
            let input = event.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
            if let Some(input) = input {
                if let Some(file) = input.files().and_then(|f| f.get(0)) {
                    let gloo_file          = GlooFile::from(file);
                    let workout            = workout.clone();
                    let error              = error.clone();
                    let selected_exercise  = selected_exercise.clone();
                    let saved_sets         = saved_sets.clone();
                    let current_session_id = current_session_id.clone();
                    let task = read_as_text(&gloo_file, move |result| match result {
                        Ok(text) => match serde_json::from_str::<Workout>(&text) {
                            Ok(data) => open_workout!(
                                data, workout, error,
                                selected_exercise, saved_sets, current_session_id
                            ),
                            Err(e) => error.set(Some(format!("Errore JSON: {}", e))),
                        },
                        Err(e) => error.set(Some(format!("Errore lettura file: {:?}", e))),
                    });
                    *reader_task.borrow_mut() = Some(task);
                }
            }
        })
    };

    // ── Catalog entry ────────────────────────────────────────────────────────
    let on_load_catalog_entry = {
        let workout            = workout.clone();
        let error              = error.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let current_session_id = current_session_id.clone();
        Callback::from(move |entry: CatalogEntry| {
            let workout            = workout.clone();
            let error              = error.clone();
            let selected_exercise  = selected_exercise.clone();
            let saved_sets         = saved_sets.clone();
            let current_session_id = current_session_id.clone();
            let file_path = entry.file.clone();
            spawn_local(async move {
                match Request::get(&file_path).send().await {
                    Ok(resp) if resp.ok() => {
                        match resp.text().await {
                            Ok(text) => match serde_json::from_str::<Workout>(&text) {
                                Ok(data) => open_workout!(
                                    data, workout, error,
                                    selected_exercise, saved_sets, current_session_id
                                ),
                                Err(e) => error.set(Some(format!("Errore JSON: {}", e))),
                            },
                            Err(e) => error.set(Some(format!("Errore caricamento file: {:?}", e))),
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
        Callback::from(move |idx: usize| selected_exercise.set(idx))
    };

    let on_change_day = {
        let workout            = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let current_session_id = current_session_id.clone();
        Callback::from(move |idx: usize| {
            day_index.set(idx);
            if let Some(w) = &*workout {
                if w.giorni.get(idx).is_some() {
                    let (sid, sets, active_ex) = find_or_create_session(w, idx);
                    saved_sets.set(sets);
                    current_session_id.set(sid);
                    selected_exercise.set(active_ex);
                }
            }
        })
    };

    // ── Input helpers ────────────────────────────────────────────────────────
    let on_weight_change = {
        let weight_inputs = weight_inputs.clone();
        Callback::from(move |(exercise_id, idx, value): (String, usize, String)| {
            let mut map = (*weight_inputs).clone();
            let entry = map.entry(exercise_id).or_insert_with(Vec::new);
            if entry.len() <= idx { entry.resize(idx + 1, String::new()); }
            entry[idx] = value;
            weight_inputs.set(map);
        })
    };

    let on_reps_change = {
        let reps_inputs = reps_inputs.clone();
        Callback::from(move |(exercise_id, idx, value): (String, usize, String)| {
            let mut map = (*reps_inputs).clone();
            let entry = map.entry(exercise_id).or_insert_with(Vec::new);
            if entry.len() <= idx { entry.resize(idx + 1, String::new()); }
            entry[idx] = value;
            reps_inputs.set(map);
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
                        let weight = weight_inputs
                            .get(&exercise.id)
                            .and_then(|v| v.get(set_index))
                            .and_then(|v| v.parse::<f32>().ok());
                        let reps = reps_inputs
                            .get(&exercise.id)
                            .and_then(|v| v.get(set_index))
                            .cloned();
                        let set_number = (set_index + 1) as u32;
                        let timestamp  = now_iso();
                        let mut list = (*saved_sets).clone();
                        if let Some(e) = list.iter_mut().find(|s| {
                            s.exercise_id == exercise.id && s.set_number == set_number
                        }) {
                            e.peso      = weight;
                            e.reps      = reps.clone();
                            e.timestamp = timestamp;
                        } else {
                            list.push(CompletedSet {
                                exercise_id: exercise.id.clone(),
                                nome:        exercise.nome.clone(),
                                set_number,
                                peso:        weight,
                                reps,
                                timestamp,
                            });
                        }
                        list.sort_by_key(|s| s.set_number);

                        // Auto-advance: if this exercise is now complete, select
                        // the next one that still has incomplete sets.
                        let ex_done = list.iter()
                            .filter(|s| s.exercise_id == exercise.id)
                            .count() >= exercise.serie as usize;
                        let current_idx = *selected_exercise;
                        let next_active = if ex_done {
                            let n = day.esercizi.len();
                            let next = (1..n)
                                .map(|off| (current_idx + off) % n)
                                .find(|&i| {
                                    let ex = &day.esercizi[i];
                                    list.iter().filter(|s| s.exercise_id == ex.id).count()
                                        < ex.serie as usize
                                });
                            if let Some(next_idx) = next {
                                selected_exercise.set(next_idx);
                                next_idx
                            } else {
                                current_idx
                            }
                        } else {
                            current_idx
                        };

                        let sid = (*current_session_id).clone();
                        if !sid.is_empty() {
                            let total = total_day_sets(workout, &day.giorno);
                            update_session_sets(&workout.id, &sid, &list, next_active, total);
                        }
                        saved_sets.set(list);
                    }
                }
            }
        })
    };

    // ── Cancel timer (called by ExerciseCard when set is manually registered) ──
    let on_cancel_timer = {
        let timer_handle  = timer_handle.clone();
        let timer_running = timer_running.clone();
        let timer_left    = timer_left.clone();
        Callback::from(move |_: ()| {
            timer_handle.borrow_mut().take();
            timer_running.set(false);
            timer_left.set(0);
        })
    };

    // ── Recovery timer (toggle: start / pause / resume) ──────────────────────
    let on_start_timer = {
        let workout_state      = workout.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let timer_running      = timer_running.clone();
        let timer_left         = timer_left.clone();
        let timer_total        = timer_total.clone();
        let timer_handle       = timer_handle.clone();
        let saved_sets         = saved_sets.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let current_session_id = current_session_id.clone();
        Callback::from(move |_: ()| {
            if *timer_running {
                // Pause: stop the interval but keep timer_left intact
                timer_handle.borrow_mut().take();
                timer_running.set(false);
                return;
            }
            if let Some(workout) = &*workout_state {
                if let Some(day) = workout.giorni.get(*day_index) {
                    if let Some(exercise) = day.esercizi.get(*selected_exercise) {
                        // Resume from pause or start fresh
                        let start_from = if *timer_left > 0 {
                            *timer_left  // resume
                        } else {
                            let dur = exercise.recupero.unwrap_or(90);
                            timer_total.set(dur);  // only reset total on fresh start
                            dur
                        };
                        timer_left.set(start_from);
                        timer_handle.borrow_mut().take();

                        let timer_left_state           = timer_left.clone();
                        let timer_running_inner        = timer_running.clone();
                        let timer_handle_inner         = timer_handle.clone();
                        let workout_for_timer          = workout_state.clone();
                        let day_index_for_timer        = day_index.clone();
                        let selected_ex_for_timer      = selected_exercise.clone();
                        let saved_sets_for_timer       = saved_sets.clone();
                        let weight_inputs_for_timer    = weight_inputs.clone();
                        let reps_inputs_for_timer      = reps_inputs.clone();
                        let session_id_for_timer       = current_session_id.clone();
                        let remaining = Rc::new(Cell::new(start_from));
                        let remaining_clone = remaining.clone();

                        let handle = Interval::new(1000, move || {
                            let next = remaining_clone.get().saturating_sub(1);
                            remaining_clone.set(next);
                            timer_left_state.set(next);

                            if next == 0 {
                                timer_running_inner.set(false);
                                if let Some(workout) = &*workout_for_timer {
                                    if let Some(day) = workout.giorni.get(*day_index_for_timer) {
                                        if let Some(exercise) = day.esercizi.get(*selected_ex_for_timer) {
                                            let existing: HashSet<u32> = (*saved_sets_for_timer)
                                                .iter()
                                                .filter(|s| s.exercise_id == exercise.id)
                                                .map(|s| s.set_number)
                                                .collect();
                                            let next_set = (1..=exercise.serie)
                                                .find(|n| !existing.contains(n))
                                                .unwrap_or(existing.len() as u32 + 1);
                                            let next_idx = (next_set - 1) as usize;
                                            let weight = weight_inputs_for_timer
                                                .get(&exercise.id)
                                                .and_then(|v| v.get(next_idx))
                                                .and_then(|v| v.parse::<f32>().ok());
                                            let reps = reps_inputs_for_timer
                                                .get(&exercise.id)
                                                .and_then(|v| v.get(next_idx))
                                                .cloned();
                                            let entry = CompletedSet {
                                                exercise_id: exercise.id.clone(),
                                                nome:        exercise.nome.clone(),
                                                set_number:  next_set,
                                                peso:        weight,
                                                reps,
                                                timestamp:   now_iso(),
                                            };
                                            let mut list = (*saved_sets_for_timer).clone();
                                            if let Some(e) = list.iter_mut().find(|s| {
                                                s.exercise_id == exercise.id && s.set_number == next_set
                                            }) {
                                                *e = entry;
                                            } else {
                                                list.push(entry);
                                            }
                                            list.sort_by_key(|s| s.set_number);

                                            // Auto-advance after timer save
                                            let ex_done = list.iter()
                                                .filter(|s| s.exercise_id == exercise.id)
                                                .count() >= exercise.serie as usize;
                                            let current_idx = *selected_ex_for_timer;
                                            let next_active = if ex_done {
                                                let n = day.esercizi.len();
                                                let next = (1..n)
                                                    .map(|off| (current_idx + off) % n)
                                                    .find(|&i| {
                                                        let ex = &day.esercizi[i];
                                                        list.iter().filter(|s| s.exercise_id == ex.id).count()
                                                            < ex.serie as usize
                                                    });
                                                if let Some(next_idx) = next {
                                                    selected_ex_for_timer.set(next_idx);
                                                    next_idx
                                                } else {
                                                    current_idx
                                                }
                                            } else {
                                                current_idx
                                            };

                                            let sid = (*session_id_for_timer).clone();
                                            if !sid.is_empty() {
                                                let total = total_day_sets(workout, &day.giorno);
                                                update_session_sets(
                                                    &workout.id, &sid, &list,
                                                    next_active, total,
                                                );
                                            }
                                            saved_sets_for_timer.set(list);
                                        }
                                    }
                                }
                                timer_handle_inner.borrow_mut().take();
                            }
                        });
                        *timer_handle.borrow_mut() = Some(handle);
                        timer_running.set(true);
                    }
                }
            }
        })
    };

    // ── Clear workout ────────────────────────────────────────────────────────
    let clear_workout = {
        let workout            = workout.clone();
        let error              = error.clone();
        let day_index          = day_index.clone();
        let selected_exercise  = selected_exercise.clone();
        let saved_sets         = saved_sets.clone();
        let weight_inputs      = weight_inputs.clone();
        let reps_inputs        = reps_inputs.clone();
        let timer_running      = timer_running.clone();
        let timer_handle       = timer_handle.clone();
        let current_session_id = current_session_id.clone();
        Callback::from(move |_| {
            timer_handle.borrow_mut().take();
            timer_running.set(false);
            workout.set(None);
            error.set(None);
            day_index.set(0);
            selected_exercise.set(0);
            saved_sets.set(Vec::new());
            weight_inputs.set(HashMap::new());
            reps_inputs.set(HashMap::new());
            current_session_id.set(String::new());
        })
    };

    // ── Render ───────────────────────────────────────────────────────────────
    html! {
        <div class="app-shell">
            <header class="app-header">
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
                        let day = workout_data.giorni.get(*day_index);
                        html! {
                            <div class="workout-details">
                                <section class="workout-meta">
                                    <div class="meta-label">{ format!("Scheda: {}", workout_data.nome) }</div>
                                    if let Some(desc) = &workout_data.descrizione {
                                        <p class="meta-desc">{ desc.clone() }</p>
                                    }
                                    if let Some(cat) = &workout_data.categoria {
                                        <div class="meta-tag">{ cat.clone() }</div>
                                    }
                                </section>

                                <DayTabs
                                    giorni={workout_data.giorni.clone()}
                                    day_index={*day_index}
                                    on_change_day={on_change_day}
                                />

                                { if let Some(day) = day {
                                    let selected = *selected_exercise;
                                    html! {
                                        <>
                                            <div class="day-header">
                                                <h2>{ day.etichetta.clone().unwrap_or_else(|| day.giorno.clone()) }</h2>
                                                <p>{ format!("{} esercizi", day.esercizi.len()) }</p>
                                            </div>
                                            <section class="exercise-list">
                                                { for day.esercizi.iter().enumerate().map(|(idx, exercise)| {
                                                    let on_select_exercise = on_select_exercise.clone();
                                                    html! {
                                                        <ExerciseCard
                                                            exercise={exercise.clone()}
                                                            saved_sets={(*saved_sets).clone()}
                                                            weight_inputs={(*weight_inputs).clone()}
                                                            reps_inputs={(*reps_inputs).clone()}
                                                            is_selected={selected == idx}
                                                            on_select={Callback::from(move |_: ()| on_select_exercise.emit(idx))}
                                                            on_save_set={on_save_set.clone()}
                                                            on_weight_change={on_weight_change.clone()}
                                                            on_reps_change={on_reps_change.clone()}
                                                            on_start_timer={on_start_timer.clone()}
                                                            on_cancel_timer={on_cancel_timer.clone()}
                                                            timer_running={*timer_running}
                                                            timer_left={*timer_left}
                                                            timer_total={*timer_total}
                                                        />
                                                    }
                                                }) }
                                            </section>
                                            <SessionHistory saved_sets={(*saved_sets).clone()} />
                                        </>
                                    }
                                } else {
                                    html! { <p>{"Giorno non trovato."}</p> }
                                } }
                            </div>
                        }
                    } else {
                        html! {
                            <CatalogPanel
                                catalog={(*catalog).clone()}
                                catalog_loading={*catalog_loading}
                                on_load_catalog_entry={on_load_catalog_entry}
                                on_file_change={on_file_change}
                            />
                        }
                    }
                }
                if let Some(error_msg) = &*error {
                    <div class="error-banner">{ error_msg }</div>
                }
            </main>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    yew::Renderer::<App>::new().render();
}
