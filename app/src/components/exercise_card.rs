use crate::models::{weight_history_for_exercise, CompletedSet, Exercise, WeightPoint};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, TouchEvent};
use yew::prelude::*;

const MAX_VISIBLE: usize = 5;
const STEP_PX:  f64 = 54.0; // DOT (34px) + GAP (20px)

// ── Weight progression chart (SVG) ────────────────────────────────────────────
fn render_weight_chart(points: &[WeightPoint]) -> Html {
    if points.is_empty() {
        return html! {
            <p class="chart-empty">
                {"Nessun dato ancora. Completa almeno un allenamento per vedere il grafico."}
            </p>
        };
    }

    // Layout constants
    let vw: f64    = 300.0;
    let vh: f64    = 110.0;
    let pad_l: f64 = 32.0;   // left: y-axis labels
    let pad_r: f64 = 10.0;
    let pad_t: f64 = 18.0;   // top: weight labels
    let pad_b: f64 = 24.0;   // bottom: date labels
    let cw = vw - pad_l - pad_r;   // inner chart width
    let ch = vh - pad_t - pad_b;   // inner chart height

    let n = points.len();
    let min_w = points.iter().map(|p| p.max_weight).fold(f32::INFINITY,     f32::min);
    let max_w = points.iter().map(|p| p.max_weight).fold(f32::NEG_INFINITY, f32::max);
    let w_range = ((max_w - min_w) as f64).max(1.0);

    let to_x = |i: usize| -> f64 {
        pad_l + if n <= 1 { cw / 2.0 } else { i as f64 / (n - 1) as f64 * cw }
    };
    let to_y = |w: f32| -> f64 {
        pad_t + ch - (w as f64 - min_w as f64) / w_range * ch
    };

    // SVG line path
    let path_d: String = points.iter().enumerate()
        .map(|(i, p)| {
            let x = to_x(i); let y = to_y(p.max_weight);
            if i == 0 { format!("M {x:.1} {y:.1}") } else { format!("L {x:.1} {y:.1}") }
        })
        .collect::<Vec<_>>().join(" ");

    // Y-axis labels
    let y_top_label  = format!("{:.0}kg", max_w);
    let y_bot_label  = format!("{:.0}kg", min_w);
    let y_top        = format!("{:.1}", pad_t);
    let y_bot        = format!("{:.1}", pad_t + ch);
    let x_axis_label = format!("{:.0}", pad_l - 4.0);

    html! {
        <svg viewBox={format!("0 0 {vw} {vh}")} width="100%" height="130"
             style="display:block;overflow:visible">
            // Horizontal grid lines
            <line x1={format!("{pad_l}")} y1={y_top.clone()}
                  x2={format!("{:.0}", vw - pad_r)} y2={y_top.clone()}
                  stroke="#f3f4f6" stroke-width="1"/>
            <line x1={format!("{pad_l}")} y1={y_bot.clone()}
                  x2={format!("{:.0}", vw - pad_r)} y2={y_bot.clone()}
                  stroke="#e5e7eb" stroke-width="1"/>
            // Y-axis labels
            <text x={x_axis_label.clone()} y={y_top.clone()}
                  text-anchor="end" font-size="9" fill="#9ca3af">
                { y_top_label }
            </text>
            <text x={x_axis_label} y={format!("{:.1}", pad_t + ch + 3.0)}
                  text-anchor="end" font-size="9" fill="#9ca3af">
                { y_bot_label }
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
                let dy        = format!("{:.1}", vh - 4.0);
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
    pub on_cancel_timer: Callback<()>,
    pub timer_running: bool,
    pub timer_left: u32,
    pub timer_total: u32,
    pub history_mode: bool,
    pub workout_id: String,
}

#[function_component(ExerciseCard)]
pub fn exercise_card(props: &ExerciseCardProps) -> Html {
    let active_set  = use_state(|| 0usize);
    let chart_open  = use_state(|| false);
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
                        // "Registra / Aggiorna" — if timer is running and this is a NEW
                        // registration (not an update), cancel the timer immediately.
                        <button class="primary-button" onclick={{
                            let save          = props.on_save_set.clone();
                            let cancel_timer  = props.on_cancel_timer.clone();
                            let asc           = active_set.clone();
                            let dot_done_snap = dot_done.clone();
                            let was_completed = completed;
                            let timer_active  = props.timer_running;
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
                            && (!completed || props.timer_running || props.timer_left > 0) {
                            <button class="secondary-button" onclick={{
                                let cb = props.on_start_timer.clone();
                                Callback::from(move |_: MouseEvent| cb.emit(()))
                            }}>
                                { if props.timer_running {
                                    "Pausa"
                                } else if props.timer_left > 0 {
                                    "Riprendi recupero"
                                } else {
                                    "Avvia recupero"
                                } }
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
