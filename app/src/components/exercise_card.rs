use crate::components::progress_bar::ProgressBar;

const STEP_VALUES: [f32; 5] = [0.5, 1.0, 2.0, 5.0, 10.0];

/// Format a weight value: whole numbers without decimal, others with one decimal.
fn fmt_weight(w: f32) -> String {
    if w.fract() == 0.0 { format!("{:.0}", w) } else { format!("{:.1}", w) }
}
use crate::models::{get_input_with_fallback, weight_history_for_exercise, CompletedSet, Exercise, TimerState, WeightPoint};
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

// ── Weight progression chart (SVG) ────────────────────────────────────────────
fn render_weight_chart(points: &[WeightPoint]) -> Html {
    if points.is_empty() {
        return html! {
            <p class="chart-empty">
                {"Nessun dato ancora. Completa almeno un allenamento per vedere il grafico."}
            </p>
        };
    }

    let view_width:  f64 = 300.0;
    let view_height: f64 = 110.0;
    let pad_left:    f64 = 32.0;  // space for y-axis labels
    let pad_right:   f64 = 10.0;
    let pad_top:     f64 = 18.0;  // space for weight labels above dots
    let pad_bottom:  f64 = 24.0;  // space for date labels below
    let inner_w = view_width  - pad_left  - pad_right;
    let inner_h = view_height - pad_top   - pad_bottom;

    let n = points.len();
    let min_w = points.iter().map(|p| p.max_weight).fold(f32::INFINITY,     f32::min);
    let max_w = points.iter().map(|p| p.max_weight).fold(f32::NEG_INFINITY, f32::max);
    let weight_range = ((max_w - min_w) as f64).max(1.0);

    let to_x = |i: usize| -> f64 {
        pad_left + if n <= 1 { inner_w / 2.0 } else { i as f64 / (n - 1) as f64 * inner_w }
    };
    let to_y = |w: f32| -> f64 {
        pad_top + inner_h - (w as f64 - min_w as f64) / weight_range * inner_h
    };

    let path_d: String = points.iter().enumerate()
        .map(|(i, p)| {
            let x = to_x(i); let y = to_y(p.max_weight);
            if i == 0 { format!("M {x:.1} {y:.1}") } else { format!("L {x:.1} {y:.1}") }
        })
        .collect::<Vec<_>>().join(" ");

    let label_max_w   = format!("{:.0}kg", max_w);
    let label_min_w   = format!("{:.0}kg", min_w);
    let y_top_str     = format!("{:.1}", pad_top);
    let y_bot_str     = format!("{:.1}", pad_top + inner_h);
    let x_ylabel_str  = format!("{:.0}", pad_left - 4.0);

    html! {
        <svg viewBox={format!("0 0 {view_width} {view_height}")} width="100%" height="130"
             style="display:block;overflow:visible">
            // Horizontal grid lines
            <line x1={format!("{pad_left}")} y1={y_top_str.clone()}
                  x2={format!("{:.0}", view_width - pad_right)} y2={y_top_str.clone()}
                  stroke="#f3f4f6" stroke-width="1"/>
            <line x1={format!("{pad_left}")} y1={y_bot_str.clone()}
                  x2={format!("{:.0}", view_width - pad_right)} y2={y_bot_str.clone()}
                  stroke="#e5e7eb" stroke-width="1"/>
            // Y-axis labels
            <text x={x_ylabel_str.clone()} y={y_top_str.clone()}
                  text-anchor="end" font-size="9" fill="#9ca3af">
                { label_max_w }
            </text>
            <text x={x_ylabel_str} y={format!("{:.1}", pad_top + inner_h + 3.0)}
                  text-anchor="end" font-size="9" fill="#9ca3af">
                { label_min_w }
            </text>
            // Line
            <path d={path_d} fill="none" stroke="#2563eb"
                  stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
            // Dots + labels
            { for points.iter().enumerate().map(|(i, p)| {
                let x   = to_x(i);
                let y   = to_y(p.max_weight);
                let cx  = format!("{x:.1}");
                let cy  = format!("{y:.1}");
                let lx  = cx.clone();
                let ly  = format!("{:.1}", y - 7.0);
                let lbl = format!("{:.1}", p.max_weight);
                // Date: show only first and last
                let show_date = i == 0 || i == n - 1;
                let date_str  = p.date.get(5..).unwrap_or(&p.date).to_string();
                let anchor    = if i == 0 { "start" } else { "end" };
                let dy        = format!("{:.1}", view_height - 4.0);
                html! {
                    <g>
                        <circle cx={cx} cy={cy} r="3.5"
                                fill="#2563eb" stroke="white" stroke-width="1.5"/>
                        <text x={lx} y={ly} text-anchor="middle"
                              font-size="9" fill="#111" font-weight="600">
                            { lbl }
                        </text>
                        { if show_date { html! {
                            <text x={format!("{x:.1}")} y={dy}
                                  text-anchor={anchor} font-size="9" fill="#9ca3af">
                                { date_str }
                            </text>
                        }} else { html! {} } }
                    </g>
                }
            }) }
        </svg>
    }
}
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
    pub on_cancel_timer: Callback<()>,
    pub timer: TimerState,
    pub history_mode: bool,
    pub workout_id: String,
}

#[function_component(ExerciseCard)]
pub fn exercise_card(props: &ExerciseCardProps) -> Html {
    let active_set  = use_state(|| 0usize);
    let chart_open  = use_state(|| false);
    let step_idx    = use_state(|| 0usize); // index into STEP_VALUES, default 1.0 kg

    let exercise    = &props.exercise;
    let exercise_id = exercise.id.clone();
    let n           = exercise.serie as usize;
    let clamped     = (*active_set).min(n.saturating_sub(1));
    let set_number  = (clamped + 1) as u32;
    let completed   = props.saved_sets.iter()
        .any(|s| s.exercise_id == exercise.id && s.set_number == set_number);

    // ── Card click ─────────────────────────────────────────────────────────
    let onclick_card = {
        let on_select   = props.on_select.clone();
        let is_selected = props.is_selected;
        Callback::from(move |_: MouseEvent| {
            if !is_selected { on_select.emit(()); }
        })
    };

    // ── Input values (with fallback to most recent non-empty entry) ───────────
    let weight_value = get_input_with_fallback(&props.weight_inputs, &exercise_id, clamped, "");
    let reps_value   = get_input_with_fallback(&props.reps_inputs,   &exercise_id, clamped, &exercise.reps);

    // ── Weight step controls ───────────────────────────────────────────────
    let step = STEP_VALUES[*step_idx];

    let on_cycle_step = {
        let si = step_idx.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            si.set((*si + 1) % STEP_VALUES.len());
        })
    };

    let weight_f: f32 = weight_value.parse().unwrap_or(0.0);

    let on_weight_minus = {
        let cb  = props.on_weight_change.clone();
        let eid = exercise_id.clone();
        let val = fmt_weight((weight_f - step).max(0.0));
        Callback::from(move |_: MouseEvent| cb.emit((eid.clone(), clamped, val.clone())))
    };
    let on_weight_plus = {
        let cb  = props.on_weight_change.clone();
        let eid = exercise_id.clone();
        let val = fmt_weight(weight_f + step);
        Callback::from(move |_: MouseEvent| cb.emit((eid.clone(), clamped, val.clone())))
    };

    // ── Reps step controls ─────────────────────────────────────────────────
    let reps_n: i32 = reps_value.parse().unwrap_or(1);

    let on_reps_minus = {
        let cb  = props.on_reps_change.clone();
        let eid = exercise_id.clone();
        let val = (reps_n - 1).max(1).to_string();
        Callback::from(move |_: MouseEvent| cb.emit((eid.clone(), clamped, val.clone())))
    };
    let on_reps_plus = {
        let cb  = props.on_reps_change.clone();
        let eid = exercise_id.clone();
        let val = (reps_n + 1).to_string();
        Callback::from(move |_: MouseEvent| cb.emit((eid.clone(), clamped, val.clone())))
    };

    // ── Dot completion state ───────────────────────────────────────────────
    let dot_done: Vec<bool> = (0..n).map(|i| {
        let s = (i + 1) as u32;
        props.saved_sets.iter().any(|e| e.exercise_id == exercise.id && e.set_number == s)
    }).collect();

    // ── Auto-advance active_set after a timer save ──────────────────────────
    // The timer saves sets from lib.rs and can't touch active_set directly.
    // If the currently active dot just became completed (timer saved it),
    // advance to the next incomplete set. Manual register handles its own
    // advancement via onclick, so we only need to act here.
    {
        let asc  = active_set.clone();
        let snap = dot_done.clone();
        let cv   = clamped;
        // n_saves = how many sets of THIS exercise are saved; changes on timer save.
        let n_saves = props.saved_sets.iter()
            .filter(|s| s.exercise_id == exercise.id)
            .count();
        use_effect_with_deps(
            move |_: &usize| {
                // Act only if the active dot is now completed (timer just saved it).
                if snap.get(cv).copied().unwrap_or(false) {
                    let next = (1..snap.len())
                        .map(|off| (cv + off) % snap.len())
                        .find(|&i| !snap.get(i).copied().unwrap_or(false));
                    if let Some(idx) = next { asc.set(idx); }
                }
                || ()
            },
            n_saves,
        );
    }

    // Pre-compute chart data only when chart is open
    let chart_points: Vec<WeightPoint> = if *chart_open {
        weight_history_for_exercise(&props.workout_id, &exercise.id)
    } else { vec![] };

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
                <button class="chart-icon-btn" title="Grafico avanzamento peso"
                    onclick={{
                        let co = chart_open.clone();
                        Callback::from(move |e: MouseEvent| {
                            e.stop_propagation();
                            co.set(!*co);
                        })
                    }}>
                    {"📈"}
                </button>
            </div>
            if *chart_open {
                <div class="weight-chart-section">
                    { render_weight_chart(&chart_points) }
                </div>
            }
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
                    <ProgressBar
                        n={exercise.serie}
                        dot_done={dot_done.clone()}
                        active={clamped}
                        on_select={{
                            let asc = active_set.clone();
                            Callback::from(move |idx: usize| asc.set(idx))
                        }}
                    />

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
                            // ── Peso ──────────────────────────────────────
                            <div class="input-field">
                                <span class="input-label">{"Peso (kg)"}</span>
                                <input
                                    class="weight-val-input"
                                    value={weight_value}
                                    inputmode="decimal"
                                    placeholder="0"
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
                                <div class="step-row">
                                    <button class="step-btn" onclick={on_weight_minus}>{"−"}</button>
                                    <button class="step-selector" onclick={on_cycle_step}
                                            title="Tocca per cambiare incremento">
                                        { fmt_weight(step) }
                                    </button>
                                    <button class="step-btn" onclick={on_weight_plus}>{"+"}</button>
                                </div>
                            </div>
                            // ── Reps ──────────────────────────────────────
                            <div class="input-field">
                                <span class="input-label">{"Reps"}</span>
                                <div class="reps-row">
                                    <button class="reps-btn" onclick={on_reps_minus}>{"−"}</button>
                                    <input
                                        class="reps-val-input"
                                        value={reps_value}
                                        inputmode="numeric"
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
                                    <button class="reps-btn" onclick={on_reps_plus}>{"+"}</button>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="action-row">
                        // "Registra / Aggiorna" — if timer is running and this is a NEW
                        // registration (not an update), cancel the timer immediately.
                        <button class="primary-button" onclick={{
                            let save          = props.on_save_set.clone();
                            let cancel_timer  = props.on_cancel_timer.clone();
                            let asc           = active_set.clone();
                            let dot_done_snap = dot_done.clone();
                            let was_completed = completed;
                            let timer_active  = props.timer.running;
                            Callback::from(move |_: MouseEvent| {
                                save.emit(clamped);
                                if !was_completed {
                                    if timer_active { cancel_timer.emit(()); }
                                    let next = (1..n)
                                        .map(|off| (clamped + off) % n)
                                        .find(|&i| !dot_done_snap.get(i).copied().unwrap_or(false));
                                    if let Some(idx) = next { asc.set(idx); }
                                }
                            })
                        }}>
                            { if completed { "Aggiorna serie" } else { "Registra serie" } }
                        </button>
                        // Timer toggle: hidden in history mode and for completed sets.
                        if !props.history_mode
                            && exercise.recupero.is_some()
                            && (!completed || props.timer.running || props.timer.left > 0) {
                            <button class="secondary-button" onclick={{
                                let cb = props.on_start_timer.clone();
                                Callback::from(move |_: MouseEvent| cb.emit(()))
                            }}>
                                { if props.timer.running {
                                    "Pausa"
                                } else if props.timer.left > 0 {
                                    "Riprendi recupero"
                                } else {
                                    "Avvia recupero"
                                } }
                            </button>
                        }
                    </div>

                </div>
            }
        </article>
    }
}
