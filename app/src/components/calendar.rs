use crate::components::icons::{icon_play, icon_x};
use crate::models::{SessionMeta, SuggestionInfo};
use js_sys::Date as JsDate;
use std::collections::HashMap;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CalendarProps {
    pub sessions:           Vec<SessionMeta>,
    pub on_select_session:  Callback<SessionMeta>,
    /// Most recent in-progress session — shown as "Riprendi" CTA.
    pub open_session:       Option<SessionMeta>,
    /// Suggested next workout based on preferred scheda — shown as "Inizia" CTA.
    pub suggestion:         Option<SuggestionInfo>,
    pub on_open_suggestion: Callback<()>,
}

// ── Date helpers ──────────────────────────────────────────────────────────────

fn today_ymd() -> (i32, u32, u32) {
    let d = JsDate::new_0();
    (d.get_full_year() as i32, d.get_month() as u32 + 1, d.get_date() as u32)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 },
        _ => 30,
    }
}

/// Returns 0=Mon … 6=Sun via Sakamoto's algorithm.
fn weekday_mon(year: i32, month: u32, day: u32) -> u32 {
    let t: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let dow = (y + y / 4 - y / 100 + y / 400 + t[month as usize - 1] + day as i32)
        .rem_euclid(7) as u32;
    (dow + 6) % 7
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "Gennaio",   2 => "Febbraio", 3 => "Marzo",     4 => "Aprile",
        5 => "Maggio",    6 => "Giugno",   7 => "Luglio",    8 => "Agosto",
        9 => "Settembre", 10 => "Ottobre", 11 => "Novembre", 12 => "Dicembre",
        _ => "",
    }
}

fn iso_date(iso: &str) -> &str { &iso[..10.min(iso.len())] }
fn iso_time(iso: &str) -> &str { if iso.len() >= 16 { &iso[11..16] } else { "" } }

// ── Component ─────────────────────────────────────────────────────────────────

#[function_component(Calendar)]
pub fn calendar(props: &CalendarProps) -> Html {
    let (ty, tm, td) = today_ymd();
    let view_year  = use_state(|| ty);
    let view_month = use_state(|| tm);
    let popup: UseStateHandle<Option<Vec<SessionMeta>>> = use_state(|| None);

    let year  = *view_year;
    let month = *view_month;

    // Group all sessions by "YYYY-MM-DD"
    let by_date: HashMap<String, Vec<SessionMeta>> = {
        let mut map: HashMap<String, Vec<SessionMeta>> = HashMap::new();
        for s in &props.sessions {
            map.entry(iso_date(&s.started).to_string())
               .or_default()
               .push(s.clone());
        }
        map
    };

    // ── Navigation ────────────────────────────────────────────────────────────
    let prev = {
        let vy = view_year.clone(); let vm = view_month.clone();
        Callback::from(move |_: MouseEvent| {
            if *vm == 1 { vy.set(*vy - 1); vm.set(12); }
            else { vm.set(*vm - 1); }
        })
    };
    let next = {
        let vy = view_year.clone(); let vm = view_month.clone();
        Callback::from(move |_: MouseEvent| {
            if *vm == 12 { vy.set(*vy + 1); vm.set(1); }
            else { vm.set(*vm + 1); }
        })
    };

    // ── Grid cells ────────────────────────────────────────────────────────────
    let n_days    = days_in_month(year, month);
    let first_wd  = weekday_mon(year, month, 1);

    let mut cells: Vec<Html> = (0..first_wd)
        .map(|_| html! { <div class="cal-cell cal-cell--empty"></div> })
        .collect();

    for day in 1..=n_days {
        let date_str   = format!("{:04}-{:02}-{:02}", year, month, day);
        let day_sess   = by_date.get(&date_str).cloned().unwrap_or_default();
        let is_today   = year == ty && month == tm && day == td;
        let has_done   = day_sess.iter().any(|s| s.done);
        let has_open   = day_sess.iter().any(|s| !s.done);
        let has_any    = !day_sess.is_empty();

        let pp = popup.clone();
        let on_sel = props.on_select_session.clone();
        let ds = day_sess.clone();

        let has_sessions = !ds.is_empty();
        let onclick = if has_sessions {
            let ds2 = ds.clone();
            Some(Callback::from(move |_: MouseEvent| {
                match ds2.len() {
                    1 => { on_sel.emit(ds2[0].clone()); }
                    _ => { pp.set(Some(ds2.clone())); }
                }
            }))
        } else {
            None
        };

        cells.push(html! {
            <div
                class={classes!(
                    "cal-cell",
                    if is_today    { Some("cal-cell--today") }  else { None },
                    if has_any     { Some("cal-cell--active") } else { None },
                    if !has_sessions { Some("cal-cell--empty") } else { None },
                )}
                onclick={onclick}
            >
                <span class="cal-day-num">{ day }</span>
                if has_any {
                    <div class="cal-dots">
                        if has_open { <span class="cal-dot cal-dot--open"></span> }
                        if has_done { <span class="cal-dot cal-dot--done"></span> }
                    </div>
                }
            </div>
        });
    }

    // ── CTA button ────────────────────────────────────────────────────────────
    let cta = if let Some(open) = &props.open_session {
        let nome = open.workout_nome.clone();
        let day  = open.day.clone();
        let meta = open.clone();
        let cb   = props.on_select_session.clone();
        html! {
            <button class="cal-cta cal-cta--resume"
                onclick={Callback::from(move |_: MouseEvent| cb.emit(meta.clone()))}>
                { icon_play() }
                <div class="cal-cta-text">
                    <span class="cal-cta-hint">{"Riprendi"}</span>
                    <span class="cal-cta-nome">{ nome }</span>
                    <span class="cal-cta-day">{ day }</span>
                </div>
            </button>
        }
    } else if let Some(sug) = &props.suggestion {
        let nome      = sug.workout_nome.clone();
        let day_label = sug.day_label.clone();
        let cb        = props.on_open_suggestion.clone();
        html! {
            <button class="cal-cta cal-cta--suggest"
                onclick={Callback::from(move |_: MouseEvent| cb.emit(()))}>
                { icon_play() }
                <div class="cal-cta-text">
                    <span class="cal-cta-hint">{"Prossimo allenamento"}</span>
                    <span class="cal-cta-nome">{ nome }</span>
                    <span class="cal-cta-day">{ day_label }</span>
                </div>
            </button>
        }
    } else {
        html! {}
    };

    // ── Disambiguation popup ──────────────────────────────────────────────────
    let popup_html = if let Some(sessions) = (*popup).clone() {
        let pp     = popup.clone();
        let on_sel = props.on_select_session.clone();
        html! {
            <div class="cal-popup-overlay"
                onclick={{ let p = pp.clone(); Callback::from(move |_: MouseEvent| p.set(None)) }}>
                <div class="cal-popup"
                    onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                    <div class="cal-popup-header">
                        <span class="cal-popup-title">{"Sessioni del giorno"}</span>
                        <button class="cal-popup-close"
                            onclick={{ let p = pp.clone(); Callback::from(move |_: MouseEvent| p.set(None)) }}>
                            { icon_x() }
                        </button>
                    </div>
                    { for sessions.iter().map(|s| {
                        let meta = s.clone();
                        let cb   = on_sel.clone();
                        let pp2  = pp.clone();
                        let time = iso_time(&s.started).to_string();
                        html! {
                            <button class="cal-popup-item"
                                onclick={Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    pp2.set(None);
                                    cb.emit(meta.clone());
                                })}>
                                <div class="cal-popup-nome">{ &s.workout_nome }</div>
                                <div class="cal-popup-meta">
                                    <span>{ format!("{} · {}", &s.day, time) }</span>
                                    if s.done {
                                        <span class="cal-popup-status cal-popup-status--done">
                                            {"Completato"}
                                        </span>
                                    } else {
                                        <span class="cal-popup-status cal-popup-status--open">
                                            {"In corso"}
                                        </span>
                                    }
                                </div>
                            </button>
                        }
                    }) }
                </div>
            </div>
        }
    } else {
        html! {}
    };

    html! {
        <>
        <div class="calendar">
            <div class="cal-header">
                <button class="cal-nav" onclick={prev}>{"‹"}</button>
                <span class="cal-month-label">
                    { format!("{} {}", month_name(month), year) }
                </span>
                <button class="cal-nav" onclick={next}>{"›"}</button>
            </div>
            <div class="cal-weekdays">
                { for ["L","M","M","G","V","S","D"].iter().map(|d| html! {
                    <div class="cal-weekday">{ *d }</div>
                }) }
            </div>
            <div class="cal-grid">
                { for cells.into_iter() }
            </div>
            { cta }
        </div>
        { popup_html }
        </>
    }
}
