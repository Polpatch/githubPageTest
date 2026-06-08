//! Recovery-timer custom hook.
//!
//! Owns the entire timestamp-based countdown that survives screen lock: the
//! `running / left / total` display state, the 1-second tick interval (which
//! recomputes the remaining time from a wall-clock end timestamp so it stays
//! correct after the device sleeps), the completion beep, and the
//! `visibilitychange` listener that snaps the display back on resume.
//!
//! The session-side "what to save when the timer hits zero" logic stays in the
//! caller: it's passed to [`RecoveryTimer::toggle`] as an `on_zero` closure that
//! captures the caller's state at start time (matching the original behavior).

use gloo_timers::callback::Interval;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;

// ── Completion beep (Web Audio API, no external file) ─────────────────────────
#[wasm_bindgen(inline_js = r#"
export function play_beep() {
    try {
        var ctx = new (window.AudioContext || window.webkitAudioContext)();
        function tone(freq, vol, t0, dur, vibrato) {
            var o = ctx.createOscillator(), g = ctx.createGain();
            o.connect(g); g.connect(ctx.destination);
            o.type = 'sine';
            o.frequency.setValueAtTime(freq * 1.04, ctx.currentTime + t0);
            o.frequency.exponentialRampToValueAtTime(freq, ctx.currentTime + t0 + 0.04);
            if (vibrato) {
                var lfo = ctx.createOscillator(), lfoG = ctx.createGain();
                lfo.frequency.value = 5.5;
                lfoG.gain.value = 14;
                lfo.connect(lfoG); lfoG.connect(o.frequency);
                lfo.start(ctx.currentTime + t0 + 0.09);
                lfo.stop(ctx.currentTime + t0 + dur + 0.1);
            }
            g.gain.setValueAtTime(vol, ctx.currentTime + t0);
            g.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + t0 + dur);
            o.start(ctx.currentTime + t0);
            o.stop(ctx.currentTime + t0 + dur + 0.1);
        }
        tone(659,  0.55, 0,    0.22, false);
        tone(784,  0.65, 0.24, 0.13, false);
        tone(1319, 0.9,  0.41, 0.85, true);
    } catch(e) {}
}
"#)]
extern "C" {
    fn play_beep();
}

/// Handle to the recovery timer returned by [`use_recovery_timer`].
///
/// `running` / `left` / `total` are plain snapshots for rendering; `toggle` and
/// `stop` drive the underlying interval. Cheap to `clone` into callbacks.
#[derive(Clone)]
pub struct RecoveryTimer {
    pub running: bool,
    pub left: u32,
    pub total: u32,
    running_h: UseStateHandle<bool>,
    left_h: UseStateHandle<u32>,
    total_h: UseStateHandle<u32>,
    handle: Rc<RefCell<Option<Interval>>>,
    end_ts: Rc<RefCell<f64>>,
}

impl RecoveryTimer {
    /// Toggle the timer:
    /// - running → pause (keep `left`, drop the wall-clock anchor)
    /// - paused with `left > 0` → resume from `left`
    /// - idle → start a fresh countdown of `fresh_secs` (also sets `total`)
    ///
    /// `on_zero` runs once when the countdown reaches zero, after the timer has
    /// stopped itself and beeped. It is captured at call time, so it sees the
    /// caller's state as of the tap that started the timer.
    pub fn toggle(&self, fresh_secs: u32, on_zero: impl Fn() + 'static) {
        if *self.running_h {
            // Pause: stop ticking but keep `left` so the user can resume.
            self.handle.borrow_mut().take();
            self.running_h.set(false);
            *self.end_ts.borrow_mut() = 0.0;
            return;
        }

        let start_from = if *self.left_h > 0 {
            *self.left_h // resume
        } else {
            self.total_h.set(fresh_secs); // only reset total on a fresh start
            fresh_secs
        };
        self.left_h.set(start_from);
        self.handle.borrow_mut().take();

        let end = js_sys::Date::now() + start_from as f64 * 1000.0;
        *self.end_ts.borrow_mut() = end;

        let left_h = self.left_h.clone();
        let running_h = self.running_h.clone();
        let handle = self.handle.clone();
        let end_ref = self.end_ts.clone();
        let h = Interval::new(1000, move || {
            let end = *end_ref.borrow();
            let next = ((end - js_sys::Date::now()) / 1000.0).max(0.0).ceil() as u32;
            left_h.set(next);
            if next == 0 {
                play_beep();
                running_h.set(false);
                handle.borrow_mut().take();
                *end_ref.borrow_mut() = 0.0;
                on_zero();
            }
        });
        *self.handle.borrow_mut() = Some(h);
        self.running_h.set(true);
    }

    /// Fully stop and clear the timer (interval, running flag, left, total, anchor).
    pub fn stop(&self) {
        self.handle.borrow_mut().take();
        self.running_h.set(false);
        self.left_h.set(0);
        self.total_h.set(0);
        *self.end_ts.borrow_mut() = 0.0;
    }
}

/// Provide a [`RecoveryTimer`]. Sets up the `visibilitychange` listener that
/// snaps `left` to the wall-clock-correct value the moment the app resumes
/// (instead of waiting up to a second for the next tick).
#[hook]
pub fn use_recovery_timer() -> RecoveryTimer {
    let running = use_state(|| false);
    let left = use_state(|| 0u32);
    let total = use_state(|| 0u32);
    let handle = use_mut_ref(|| None::<Interval>);
    let end_ts = use_mut_ref(|| 0.0f64);
    let vis_listener = use_mut_ref(|| None::<Closure<dyn Fn()>>);

    // ── Page-visibility correction ───────────────────────────────────────────
    // Gate on the live end timestamp (Rc, always current) rather than a state
    // snapshot, so the correction actually fires on resume.
    {
        let end_ts = end_ts.clone();
        let left = left.clone();
        let vis_listener = vis_listener.clone();
        use_effect_with_deps(
            move |_| {
                let doc_opt = web_sys::window().and_then(|w| w.document());
                if let Some(doc) = &doc_opt {
                    let cb = Closure::<dyn Fn()>::wrap(Box::new(move || {
                        let Some(d) = web_sys::window().and_then(|w| w.document()) else { return };
                        if d.hidden() { return; }
                        let end = *end_ts.borrow();
                        if end <= 0.0 { return; }
                        let remaining = ((end - js_sys::Date::now()) / 1000.0).max(0.0).ceil() as u32;
                        left.set(remaining);
                    }));
                    doc.add_event_listener_with_callback(
                        "visibilitychange",
                        cb.as_ref().unchecked_ref(),
                    )
                    .ok();
                    *vis_listener.borrow_mut() = Some(cb);
                }

                let vl = vis_listener.clone();
                move || {
                    let guard = vl.borrow();
                    if let Some(c) = guard.as_ref() {
                        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                            doc.remove_event_listener_with_callback(
                                "visibilitychange",
                                c.as_ref().unchecked_ref(),
                            )
                            .ok();
                        }
                    }
                    drop(guard);
                    *vl.borrow_mut() = None;
                }
            },
            (),
        );
    }

    RecoveryTimer {
        running: *running,
        left: *left,
        total: *total,
        running_h: running,
        left_h: left,
        total_h: total,
        handle,
        end_ts,
    }
}
