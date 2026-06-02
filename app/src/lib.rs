mod components;
mod models;

use components::catalog_panel::CatalogPanel;
use components::day_tabs::DayTabs;
use components::exercise_card::ExerciseCard;
use components::session_history::SessionHistory;
use gloo_file::callbacks::{read_as_text, FileReader};
use gloo_file::File as GlooFile;
use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use gloo_timers::callback::Interval;
use js_sys::Date;
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
    let workout = use_state(|| None::<Workout>);
    let error = use_state(|| None::<String>);
    let day_index = use_state(|| 0usize);
    let selected_exercise = use_state(|| 0usize);
    let weight_inputs = use_state(|| HashMap::<String, Vec<String>>::new());
    let reps_inputs = use_state(|| HashMap::<String, Vec<String>>::new());
    let saved_sets = use_state(|| Vec::<CompletedSet>::new());
    let catalog = use_state(|| Vec::<CatalogEntry>::new());
    let catalog_loading = use_state(|| true);
    let timer_running = use_state(|| false);
    let timer_left = use_state(|| 0u32);
    let timer_total = use_state(|| 0u32);
    let timer_handle = use_mut_ref(|| None::<Interval>);
    let reader_task = use_mut_ref(|| None::<FileReader>);

    let _fetch_catalog = {
        let catalog = catalog.clone();
        let error = error.clone();
        let catalog_loading = catalog_loading.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Request::get("schede/catalog.json").send().await {
                        Ok(resp) => {
                            if resp.ok() {
                                match resp.json::<Vec<CatalogEntry>>().await {
                                    Ok(list) => catalog.set(list),
                                    Err(err) => error.set(Some(format!("Errore catalogo: {:?}", err))),
                                }
                            } else {
                                error.set(Some(format!("Errore caricamento catalogo: {}", resp.status())));
                            }
                            catalog_loading.set(false);
                        }
                        Err(err) => {
                            error.set(Some(format!("Errore caricamento catalogo: {:?}", err)));
                            catalog_loading.set(false);
                        }
                    }
                });
                || ()
            },
            (),
        )
    };

    let on_file_change = {
        let workout = workout.clone();
        let error = error.clone();
        let selected_exercise = selected_exercise.clone();
        let saved_sets = saved_sets.clone();
        let reader_task = reader_task.clone();
        Callback::from(move |event: web_sys::Event| {
            let input = event
                .target()
                .and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
            if let Some(input) = input {
                if let Some(file) = input.files().and_then(|files| files.get(0)) {
                    let gloo_file = GlooFile::from(file);
                    let workout = workout.clone();
                    let error = error.clone();
                    let selected_exercise = selected_exercise.clone();
                    let saved_sets = saved_sets.clone();
                    let task = read_as_text(&gloo_file, move |result| match result {
                        Ok(text) => match serde_json::from_str::<Workout>(&text) {
                            Ok(data) => {
                                let saved = data
                                    .giorni
                                    .get(0)
                                    .map(|day| load_session(&session_key(&data.id, &day.giorno)))
                                    .unwrap_or_else(Vec::new);
                                workout.set(Some(data));
                                error.set(None);
                                selected_exercise.set(0);
                                saved_sets.set(saved);
                            }
                            Err(err) => error.set(Some(format!("Errore JSON: {}", err))),
                        },
                        Err(err) => error.set(Some(format!("Errore lettura file: {:?}", err))),
                    });
                    *reader_task.borrow_mut() = Some(task);
                }
            }
        })
    };

    let on_load_catalog_entry = {
        let workout = workout.clone();
        let error = error.clone();
        let selected_exercise = selected_exercise.clone();
        let saved_sets = saved_sets.clone();
        Callback::from(move |entry: CatalogEntry| {
            let workout = workout.clone();
            let error = error.clone();
            let selected_exercise = selected_exercise.clone();
            let saved_sets = saved_sets.clone();
            let file_path = entry.file.clone();
            spawn_local(async move {
                match Request::get(&file_path).send().await {
                    Ok(resp) => {
                        if resp.ok() {
                            match resp.text().await {
                                Ok(text) => match serde_json::from_str::<Workout>(&text) {
                                    Ok(data) => {
                                        let saved = data
                                            .giorni
                                            .get(0)
                                            .map(|day| load_session(&session_key(&data.id, &day.giorno)))
                                            .unwrap_or_else(Vec::new);
                                        workout.set(Some(data));
                                        error.set(None);
                                        selected_exercise.set(0);
                                        saved_sets.set(saved);
                                    }
                                    Err(err) => error.set(Some(format!("Errore JSON: {}", err))),
                                },
                                Err(err) => error.set(Some(format!("Errore caricamento file: {:?}", err))),
                            }
                        } else {
                            error.set(Some(format!("Errore caricamento file: {}", resp.status())));
                        }
                    }
                    Err(err) => error.set(Some(format!("Errore caricamento file: {:?}", err))),
                }
            });
        })
    };

    let on_select_exercise = {
        let selected_exercise = selected_exercise.clone();
        Callback::from(move |idx: usize| selected_exercise.set(idx))
    };

    let on_change_day = {
        let workout = workout.clone();
        let day_index = day_index.clone();
        let saved_sets = saved_sets.clone();
        Callback::from(move |idx: usize| {
            day_index.set(idx);
            if let Some(workout) = &*workout {
                if let Some(day) = workout.giorni.get(idx) {
                    saved_sets.set(load_session(&session_key(&workout.id, &day.giorno)));
                }
            }
        })
    };

    let on_weight_change = {
        let weight_inputs = weight_inputs.clone();
        Callback::from(move |(exercise_id, idx, value): (String, usize, String)| {
            let mut map = (*weight_inputs).clone();
            let entry = map.entry(exercise_id).or_insert_with(Vec::new);
            if entry.len() <= idx {
                entry.resize(idx + 1, String::new());
            }
            entry[idx] = value;
            weight_inputs.set(map);
        })
    };

    let on_reps_change = {
        let reps_inputs = reps_inputs.clone();
        Callback::from(move |(exercise_id, idx, value): (String, usize, String)| {
            let mut map = (*reps_inputs).clone();
            let entry = map.entry(exercise_id).or_insert_with(Vec::new);
            if entry.len() <= idx {
                entry.resize(idx + 1, String::new());
            }
            entry[idx] = value;
            reps_inputs.set(map);
        })
    };

    let on_save_set = {
        let workout = workout.clone();
        let day_index = day_index.clone();
        let selected_exercise = selected_exercise.clone();
        let weight_inputs = weight_inputs.clone();
        let reps_inputs = reps_inputs.clone();
        let saved_sets = saved_sets.clone();
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
                        let timestamp = Date::new_0().to_iso_string().as_string().unwrap_or_default();
                        let mut list = (*saved_sets).clone();
                        if let Some(existing) = list.iter_mut().find(|s| s.exercise_id == exercise.id && s.set_number == set_number) {
                            existing.peso = weight;
                            existing.reps = reps.clone();
                            existing.timestamp = timestamp;
                        } else {
                            list.push(CompletedSet {
                                exercise_id: exercise.id.clone(),
                                nome: exercise.nome.clone(),
                                set_number,
                                peso: weight,
                                reps,
                                timestamp,
                            });
                        }
                        list.sort_by(|a, b| a.set_number.cmp(&b.set_number));
                        let key = session_key(&workout.id, &day.giorno);
                        let _ = LocalStorage::set(&key, &list);
                        saved_sets.set(list);
                    }
                }
            }
        })
    };

    let on_start_timer = {
        let workout_state = workout.clone();
        let day_index = day_index.clone();
        let selected_exercise = selected_exercise.clone();
        let timer_running = timer_running.clone();
        let timer_left = timer_left.clone();
        let timer_total = timer_total.clone();
        let timer_handle = timer_handle.clone();
        let saved_sets = saved_sets.clone();
        let weight_inputs = weight_inputs.clone();
        let reps_inputs = reps_inputs.clone();
        Callback::from(move |_: ()| {
            if *timer_running {
                return;
            }
            if let Some(workout) = &*workout_state {
                if let Some(day) = workout.giorni.get(*day_index) {
                    if let Some(exercise) = day.esercizi.get(*selected_exercise) {
                        let duration = exercise.recupero.unwrap_or(90);
                        timer_left.set(duration);
                        timer_total.set(duration);
                        timer_handle.borrow_mut().take();

                        let timer_left_state = timer_left.clone();
                        let timer_running_inner = timer_running.clone();
                        let timer_handle_inner = timer_handle.clone();
                        let workout_for_timer = workout_state.clone();
                        let day_index_for_timer = day_index.clone();
                        let selected_exercise_for_timer = selected_exercise.clone();
                        let saved_sets_for_timer = saved_sets.clone();
                        let weight_inputs_for_timer = weight_inputs.clone();
                        let reps_inputs_for_timer = reps_inputs.clone();
                        let remaining_counter = Rc::new(Cell::new(duration));
                        let remaining_counter_clone = remaining_counter.clone();

                        let handle = Interval::new(1000, move || {
                            let next = remaining_counter_clone.get().saturating_sub(1);
                            remaining_counter_clone.set(next);
                            timer_left_state.set(next);

                            if next == 0 {
                                timer_running_inner.set(false);
                                if let Some(workout) = &*workout_for_timer {
                                    if let Some(day) = workout.giorni.get(*day_index_for_timer) {
                                        if let Some(exercise) = day.esercizi.get(*selected_exercise_for_timer) {
                                            let existing_numbers: HashSet<u32> = (*saved_sets_for_timer)
                                                .iter()
                                                .filter(|s| s.exercise_id == exercise.id)
                                                .map(|s| s.set_number)
                                                .collect();
                                            let next_set = (1..=exercise.serie)
                                                .find(|n| !existing_numbers.contains(n))
                                                .unwrap_or(existing_numbers.len() as u32 + 1);
                                            let next_index = (next_set - 1) as usize;
                                            let weight = weight_inputs_for_timer
                                                .get(&exercise.id)
                                                .and_then(|v| v.get(next_index))
                                                .and_then(|v| v.parse::<f32>().ok());
                                            let reps = reps_inputs_for_timer
                                                .get(&exercise.id)
                                                .and_then(|v| v.get(next_index))
                                                .cloned();
                                            let entry = CompletedSet {
                                                exercise_id: exercise.id.clone(),
                                                nome: exercise.nome.clone(),
                                                set_number: next_set,
                                                peso: weight,
                                                reps,
                                                timestamp: Date::new_0().to_iso_string().as_string().unwrap_or_default(),
                                            };
                                            let mut list = (*saved_sets_for_timer).clone();
                                            if let Some(existing) = list.iter_mut().find(|s| s.exercise_id == exercise.id && s.set_number == next_set) {
                                                *existing = entry;
                                            } else {
                                                list.push(entry);
                                            }
                                            list.sort_by(|a, b| a.set_number.cmp(&b.set_number));
                                            let key = session_key(&workout.id, &day.giorno);
                                            let _ = LocalStorage::set(&key, &list);
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

    let clear_workout = {
        let workout = workout.clone();
        let error = error.clone();
        let day_index = day_index.clone();
        let selected_exercise = selected_exercise.clone();
        let saved_sets = saved_sets.clone();
        let weight_inputs = weight_inputs.clone();
        let reps_inputs = reps_inputs.clone();
        let timer_running = timer_running.clone();
        let timer_handle = timer_handle.clone();
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
        })
    };

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
