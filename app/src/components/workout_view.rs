//! The main workout screen: scheda meta, day tabs, exercise list and footer.
//!
//! Extracted from `App` so it memoizes on its `PartialEq` props — during a
//! recovery-timer tick (which re-renders `App` every second) this component is
//! skipped entirely when none of its inputs changed. For that skip to actually
//! happen the callbacks passed in must be stable across renders (see the
//! `use_callback`s in `lib.rs`), since `Callback` compares by identity.

use crate::components::day_tabs::DayTabs;
use crate::components::exercise_card::ExerciseCard;
use crate::models::{CompletedSet, Workout};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct WorkoutViewProps {
    pub workout: Workout,
    pub day_index: usize,
    pub selected_exercise: usize,
    pub saved_sets: Vec<CompletedSet>,
    pub viewing_history: bool,
    pub desc_expanded: bool,
    pub session_done: usize,
    pub session_total: usize,
    pub all_done: bool,
    pub elapsed_str: String,
    pub on_exit_history: Callback<MouseEvent>,
    pub on_toggle_desc: Callback<MouseEvent>,
    pub on_change_day: Callback<usize>,
    pub on_select_exercise: Callback<usize>,
    pub on_save_and_finish: Callback<MouseEvent>,
    pub on_delete_workout: Callback<MouseEvent>,
}

#[function_component(WorkoutView)]
pub fn workout_view(props: &WorkoutViewProps) -> Html {
    let workout = &props.workout;
    let day = workout.giorni.get(props.day_index);

    html! {
        <div class="workout-details">
            if props.viewing_history {
                <div class="history-banner">
                    <span>{"Stai visualizzando uno storico"}</span>
                    <button onclick={props.on_exit_history.clone()}>
                        {"← Torna all'allenamento"}
                    </button>
                </div>
            }
            <section class="workout-meta">
                <div class="meta-label">{ format!("Scheda: {}", workout.nome) }</div>
                if let Some(desc) = &workout.descrizione {
                    if desc.len() > 100 {
                        <p class={if props.desc_expanded { "meta-desc" } else { "meta-desc meta-desc--clamped" }}>
                            { desc.clone() }
                        </p>
                        <button class="meta-expand-btn" onclick={props.on_toggle_desc.clone()}>
                            { if props.desc_expanded { "Mostra meno ↑" } else { "Mostra di più ↓" } }
                        </button>
                    } else {
                        <p class="meta-desc">{ desc.clone() }</p>
                    }
                }
                if let Some(cat) = &workout.categoria {
                    <div class="meta-tag">{ cat.clone() }</div>
                }
            </section>

            <DayTabs
                giorni={workout.giorni.clone()}
                day_index={props.day_index}
                on_change_day={props.on_change_day.clone()}
            />

            { if let Some(day) = day {
                let selected = props.selected_exercise;
                html! {
                    <>
                        <div class="day-header">
                            <div class="day-header-row">
                                <h2>{ day.etichetta.clone().unwrap_or_else(|| day.giorno.clone()) }</h2>
                                if props.session_total > 0 {
                                    <span class={classes!(
                                        "session-progress-badge",
                                        if props.all_done { Some("session-progress-badge--done") } else { None }
                                    )}>
                                        { format!("{} / {} serie", props.session_done, props.session_total) }
                                    </span>
                                }
                            </div>
                            <p>
                                { format!("{} esercizi", day.esercizi.len()) }
                                if !props.elapsed_str.is_empty() {
                                    { format!(" · {}", props.elapsed_str.clone()) }
                                }
                            </p>
                        </div>
                        <section class="exercise-list" key={props.day_index}>
                            { for day.esercizi.iter().enumerate().map(|(idx, exercise)| {
                                let on_select_exercise = props.on_select_exercise.clone();
                                html! {
                                    <ExerciseCard
                                        exercise={exercise.clone()}
                                        saved_sets={props.saved_sets.clone()}
                                        is_selected={selected == idx}
                                        on_select={Callback::from(move |_: ()| on_select_exercise.emit(idx))}
                                    />
                                }
                            }) }
                        </section>
                        <div class="workout-footer">
                            <button class={classes!(
                                "footer-btn",
                                "footer-btn--save",
                                if props.all_done { Some("footer-btn--save--done") } else { None }
                            )}
                                onclick={props.on_save_and_finish.clone()}>
                                {"Salva e termina"}
                            </button>
                            <button class="footer-btn footer-btn--delete"
                                onclick={props.on_delete_workout.clone()}>
                                {"Cancella allenamento"}
                            </button>
                        </div>
                    </>
                }
            } else {
                html! { <p>{"Giorno non trovato."}</p> }
            } }
        </div>
    }
}
