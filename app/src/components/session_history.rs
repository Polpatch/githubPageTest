use crate::models::CompletedSet;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SessionHistoryProps {
    pub saved_sets: Vec<CompletedSet>,
}

#[function_component(SessionHistory)]
pub fn session_history(props: &SessionHistoryProps) -> Html {
    html! {
        <section class="session-history">
            <h3>{"Storico serie"}</h3>
            { if props.saved_sets.is_empty() {
                html! { <p>{"Nessuna serie registrata per questo giorno."}</p> }
            } else {
                html! {
                    <ul>
                        { for props.saved_sets.iter().map(|entry| html! {
                            <li>
                                <strong>{ format!("{} - set {}", entry.nome, entry.set_number) }</strong>
                                <div>{ format!("Peso: {} reps: {}", entry.peso.map(|v| v.to_string()).unwrap_or_else(|| "-".into()), entry.reps.clone().unwrap_or_else(|| "-".into())) }</div>
                                <div class="history-time">{ &entry.timestamp }</div>
                            </li>
                        }) }
                    </ul>
                }
            } }
        </section>
    }
}
