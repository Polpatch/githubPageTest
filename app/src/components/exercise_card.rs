use crate::models::{CompletedSet, Exercise};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, TouchEvent};
use yew::prelude::*;

const MAX_VISIBLE: usize = 5;
const STEP_PX:  f64 = 54.0; // DOT (34px) + GAP (20px)
const HALF_DOT: f64 = 17.0; // DOT / 2 — offset to align dot centre on track origin

#[derive(Properties, PartialEq)]
pub struct ExerciseCardProps {
    pub exercise: Exercise,
    pub saved_sets: Vec<CompletedSet>,
    pub weight_inputs: HashMap<String, Vec<String>>,
    pub reps_inputs: HashMap<String, Vec<String>>,
    pub is_selected: bool,
    pub on_select: Callback<()>,
    pub on_save_set: Callback<usize>,
    pub on_weight_change: Callback<(String, usize, String)>,
    pub on_reps_change: Callback<(String, usize, String)>,
    pub on_start_timer: Callback<()>,
    pub timer_running: bool,
    pub timer_left: u32,
    pub timer_total: u32,
}

#[function_component(ExerciseCard)]
pub fn exercise_card(props: &ExerciseCardProps) -> Html {
    let active_set  = use_state(|| 0usize);
    // base_offset: how far the track's left edge is from the outer centre.
    // offset = HALF_DOT centres dot 0; offset = idx*STEP_PX + HALF_DOT centres dot idx.
    let base_offset = use_state(|| HALF_DOT);
    let is_dragging = use_state(|| false);
    let drag_start: Rc<RefCell<Option<(f64, f64)>>> = use_mut_ref(|| None);

    let exercise    = &props.exercise;
    let exercise_id = exercise.id.clone();
    let n           = exercise.serie as usize;
    // offset range: [HALF_DOT, (n-1)*STEP + HALF_DOT]
    let max_offset  = (n.saturating_sub(1)) as f64 * STEP_PX + HALF_DOT;
    let needs_carousel = n > MAX_VISIBLE;

    let clamped    = (*active_set).min(n.saturating_sub(1));
    let set_number = (clamped + 1) as u32;
    let completed  = props.saved_sets.iter()
        .any(|s| s.exercise_id == exercise.id && s.set_number == set_number);

    // ── Centre active dot when selection changes ────────────────────────────
    {
        let bo  = base_offset.clone();
        let max = max_offset;
        use_effect_with_deps(
            move |idx: &usize| {
                // offset = idx * STEP + HALF_DOT  →  dot `idx` centre at left:50%
                let target = *idx as f64 * STEP_PX + HALF_DOT;
                bo.set(target.max(HALF_DOT).min(max));
                || ()
            },
            *active_set,
        );
    }

    // ── Snap: round to nearest slot, restore is_dragging ───────────────────
    let snap = {
        let bo  = base_offset.clone();
        let ds  = drag_start.clone();
        let isd = is_dragging.clone();
        let max = max_offset;
        move || {
            let raw     = *bo;
            // nearest dot index, then back to offset
            let idx     = ((raw - HALF_DOT) / STEP_PX).round();
            let snapped = idx * STEP_PX + HALF_DOT;
            bo.set(snapped.max(HALF_DOT).min(max));
            *ds.borrow_mut() = None;
            isd.set(false);
        }
    };

    // ── Drag: mouse ────────────────────────────────────────────────────────
    let on_mouse_down = {
        let bo  = base_offset.clone();
        let ds  = drag_start.clone();
        let isd = is_dragging.clone();
        Callback::from(move |e: MouseEvent| {
            *ds.borrow_mut() = Some((e.client_x() as f64, *bo));
            isd.set(true);
        })
    };

    let on_mouse_move = {
        let bo   = base_offset.clone();
        let ds   = drag_start.clone();
        let snap = snap.clone();
        let max  = max_offset;
        Callback::from(move |e: MouseEvent| {
            if e.buttons() == 0 {
                if ds.borrow().is_some() { snap(); }
                return;
            }
            if let Some((sx, so)) = *ds.borrow() {
                let new_off = (so + sx - e.client_x() as f64).max(HALF_DOT).min(max);
                bo.set(new_off);
            }
        })
    };

    let on_mouse_up = {
        let snap = snap.clone();
        Callback::from(move |_: MouseEvent| snap())
    };

    // ── Drag: touch ────────────────────────────────────────────────────────
    let on_touch_start = {
        let bo  = base_offset.clone();
        let ds  = drag_start.clone();
        let isd = is_dragging.clone();
        Callback::from(move |e: TouchEvent| {
            if let Some(t) = e.touches().get(0) {
                *ds.borrow_mut() = Some((t.client_x() as f64, *bo));
                isd.set(true);
            }
        })
    };

    let on_touch_move = {
        let bo   = base_offset.clone();
        let ds   = drag_start.clone();
        let max  = max_offset;
        Callback::from(move |e: TouchEvent| {
            if let Some(t) = e.touches().get(0) {
                if let Some((sx, so)) = *ds.borrow() {
                    let new_off = (so + sx - t.client_x() as f64).max(HALF_DOT).min(max);
                    bo.set(new_off);
                }
            }
        })
    };

    let on_touch_end = {
        let snap = snap.clone();
        Callback::from(move |_: TouchEvent| snap())
    };

    // ── Card click ─────────────────────────────────────────────────────────
    let onclick_card = {
        let on_select   = props.on_select.clone();
        let is_selected = props.is_selected;
        Callback::from(move |_: MouseEvent| {
            if !is_selected { on_select.emit(()); }
        })
    };

    // ── Input values ───────────────────────────────────────────────────────
    let weight_value = props.weight_inputs
        .get(&exercise_id)
        .and_then(|v| v.get(clamped).cloned())
        .filter(|v| !v.is_empty())
        .or_else(|| (0..clamped).rev().find_map(|p|
            props.weight_inputs.get(&exercise_id)
                .and_then(|v| v.get(p).cloned())
                .filter(|v| !v.is_empty())
        ))
        .unwrap_or_default();

    let reps_value = props.reps_inputs
        .get(&exercise_id)
        .and_then(|v| v.get(clamped).cloned())
        .filter(|v| !v.is_empty())
        .or_else(|| (0..clamped).rev().find_map(|p|
            props.reps_inputs.get(&exercise_id)
                .and_then(|v| v.get(p).cloned())
                .filter(|v| !v.is_empty())
        ))
        .unwrap_or_else(|| exercise.reps.clone());

    // ── Dot completion state ───────────────────────────────────────────────
    let dot_done: Vec<bool> = (0..n).map(|i| {
        let s = (i + 1) as u32;
        props.saved_sets.iter().any(|e| e.exercise_id == exercise.id && e.set_number == s)
    }).collect();

    // ── Progress bar ───────────────────────────────────────────────────────
    let progress_bar = if n == 1 {
        let is_done = dot_done.first().copied().unwrap_or(false);
        html! {
            <div class="series-progress">
                <div class={classes!(
                    "series-capsule",
                    if is_done { Some("completed") } else { Some("active") }
                )}></div>
            </div>
        }
    } else if !needs_carousel {
        // Simple row: dots + flex lines filling available width
        let items: Vec<Html> = (0..n).flat_map(|idx| {
            let is_done   = dot_done[idx];
            let is_active = clamped == idx;
            let asc       = active_set.clone();
            let dot = html! {
                <button
                    class={classes!(
                        "series-dot",
                        if is_done   { Some("completed") } else { None },
                        if is_active { Some("active")    } else { None }
                    )}
                    onclick={Callback::from(move |e: MouseEvent| {
                        e.stop_propagation();
                        asc.set(idx);
                    })}
                >{ (idx + 1).to_string() }</button>
            };
            if idx < n - 1 {
                vec![dot, html! { <div class="series-line"></div> }]
            } else {
                vec![dot]
            }
        }).collect();
        html! {
            <div class="series-progress">
                { for items.into_iter() }
            </div>
        }
    } else {
        // Carousel: block container (no flex!) clips the track.
        // left:50% on the track + translateX(-base_offset) centres dot `idx`
        // when base_offset = idx*STEP + HALF_DOT, with no percentage CSS on the track.
        let translate = format!(
            "transform: translateX(-{}px) translateY(-50%)",
            *base_offset
        );
        let track_class = if *is_dragging {
            "carousel-track"
        } else {
            "carousel-track carousel-track--snap"
        };

        let items: Vec<Html> = (0..n).flat_map(|idx| {
            let is_done   = dot_done[idx];
            let is_active = clamped == idx;
            let asc       = active_set.clone();
            let dot = html! {
                <button
                    class={classes!(
                        "series-dot",
                        if is_done   { Some("completed") } else { None },
                        if is_active { Some("active")    } else { None }
                    )}
                    onclick={Callback::from(move |e: MouseEvent| {
                        e.stop_propagation();
                        asc.set(idx);
                    })}
                >{ (idx + 1).to_string() }</button>
            };
            if idx < n - 1 {
                vec![dot, html! { <div class="series-line series-line--c"></div> }]
            } else {
                vec![dot]
            }
        }).collect();

        html! {
            <div
                class="carousel-outer"
                onmousedown={on_mouse_down}
                onmousemove={on_mouse_move}
                onmouseup={on_mouse_up}
                onmouseleave={Callback::from({
                    let snap = snap.clone();
                    move |_: MouseEvent| snap()
                })}
                ontouchstart={on_touch_start}
                ontouchmove={on_touch_move}
                ontouchend={on_touch_end}
            >
                <div class={track_class} style={translate}>
                    { for items.into_iter() }
                </div>
            </div>
        }
    };

    // ── Full render ────────────────────────────────────────────────────────
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
            if let Some(note) = &exercise.note {
                <p class="exercise-note">{ note.clone() }</p>
            }
            if props.is_selected {
                <div class="expanded-body">
                    { progress_bar }

                    <div class="series-row">
                        <div class="series-row-header">
                            <span>{ format!("Serie {}", set_number) }</span>
                            { if completed {
                                html! { <span class="series-status">{"Completata"}</span> }
                            } else {
                                html! { <span class="series-status pending">{"In attesa"}</span> }
                            } }
                        </div>
                        <div class="input-row">
                            <label>
                                {"Peso (kg)"}
                                <input
                                    value={weight_value}
                                    placeholder="es. 80"
                                    oninput={{
                                        let cb  = props.on_weight_change.clone();
                                        let eid = exercise_id.clone();
                                        Callback::from(move |e: InputEvent| {
                                            if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                                                cb.emit((eid.clone(), clamped, i.value()));
                                            }
                                        })
                                    }}
                                />
                            </label>
                            <label>
                                {"Reps"}
                                <input
                                    value={reps_value}
                                    placeholder={exercise.reps.clone()}
                                    oninput={{
                                        let cb  = props.on_reps_change.clone();
                                        let eid = exercise_id.clone();
                                        Callback::from(move |e: InputEvent| {
                                            if let Some(i) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                                                cb.emit((eid.clone(), clamped, i.value()));
                                            }
                                        })
                                    }}
                                />
                            </label>
                        </div>
                    </div>

                    <div class="action-row">
                        <button class="primary-button" onclick={{
                            let save = props.on_save_set.clone();
                            Callback::from(move |_: MouseEvent| save.emit(clamped))
                        }}>
                            { if completed { "Aggiorna serie" } else { "Registra serie" } }
                        </button>
                        if exercise.recupero.is_some() {
                            <button class="secondary-button" onclick={{
                                let cb = props.on_start_timer.clone();
                                Callback::from(move |_: MouseEvent| cb.emit(()))
                            }}>
                                { if props.timer_running { "Timer in corso" } else { "Avvia recupero" } }
                            </button>
                        }
                    </div>

                    { if props.timer_running {
                        let pct = if props.timer_total > 0 {
                            (props.timer_left as f32 / props.timer_total as f32) * 100.0
                        } else { 0.0 };
                        html! {
                            <div class="timer-card">
                                <div class="timer-label">{"Recupero in corso"}</div>
                                <div class="timer-value">{ format!("{}s", props.timer_left) }</div>
                                <div class="timer-bar">
                                    <div class="timer-bar-fill" style={format!("width:{}%", pct)}></div>
                                </div>
                            </div>
                        }
                    } else { html! {} } }
                </div>
            }
        </article>
    }
}
