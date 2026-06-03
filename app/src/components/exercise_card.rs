use crate::components::progress_bar::ProgressBar;
use crate::models::{CompletedSet, Exercise};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ExerciseCardProps {
    pub exercise:  Exercise,
    pub saved_sets: Vec<CompletedSet>,
    pub is_selected: bool,
    pub on_select:   Callback<()>,
}

#[function_component(ExerciseCard)]
pub fn exercise_card(props: &ExerciseCardProps) -> Html {
    let exercise = &props.exercise;
    let n        = exercise.serie as usize;

    // Completion state per set (for read-only progress bar display)
    let dot_done: Vec<bool> = (0..n).map(|i| {
        let s = (i + 1) as u32;
        props.saved_sets.iter()
            .any(|e| e.exercise_id == exercise.id && e.set_number == s)
    }).collect();

    // Show first incomplete set as "active" in the compact dot row
    let active_hint = dot_done.iter().position(|&done| !done).unwrap_or(0);

    let onclick_card = {
        let on_select = props.on_select.clone();
        Callback::from(move |_: MouseEvent| on_select.emit(()))
    };

    html! {
        <article
            class={classes!("exercise-card", if props.is_selected { Some("selected") } else { None })}
            onclick={onclick_card}
        >
            <div class="exercise-head">
                <div>
                    <h3>{ &exercise.nome }</h3>
                    <div class="exercise-meta">
                        { format!("{} serie · {}", exercise.serie, exercise.reps) }
                    </div>
                </div>
            </div>
            if exercise.recupero.is_some() {
                <div class="exercise-rec">
                    { format!("Recupero: {}s", exercise.recupero.unwrap_or(0)) }
                </div>
            }
            // Read-only progress bar (tapping a dot selects the exercise)
            <ProgressBar
                n={exercise.serie}
                dot_done={dot_done}
                active={active_hint}
                on_select={{
                    let on_sel = props.on_select.clone();
                    Callback::from(move |_: usize| on_sel.emit(()))
                }}
            />
        </article>
    }
}
