use crate::models::Day;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DayTabsProps {
    pub giorni: Vec<Day>,
    pub day_index: usize,
    pub on_change_day: Callback<usize>,
}

#[function_component(DayTabs)]
pub fn day_tabs(props: &DayTabsProps) -> Html {
    html! {
        <section class="day-tabs">
            { for props.giorni.iter().enumerate().map(|(idx, day)| {
                let selected = props.day_index == idx;
                let on_change_day = props.on_change_day.clone();
                let onclick = Callback::from(move |_| on_change_day.emit(idx));
                html! {
                    <button class={classes!("day-tab", if selected { Some("active") } else { None })} {onclick}>
                        {&day.giorno}
                    </button>
                }
            }) }
        </section>
    }
}
