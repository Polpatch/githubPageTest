use crate::components::icons::{icon_star_empty, icon_star_filled, icon_star_radiant};
use crate::models::CatalogEntry;
use web_sys::Event;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CatalogPanelProps {
    pub catalog: Vec<CatalogEntry>,
    pub catalog_loading: bool,
    pub on_load_catalog_entry: Callback<CatalogEntry>,
    pub on_file_change: Callback<Event>,
    pub user_preferred: Option<String>,
    pub on_set_preferred: Callback<Option<String>>,
}

/// Three visual states for the star on each catalog card.
#[derive(PartialEq, Clone, Copy)]
enum StarState {
    UserPreferred,    // user explicitly starred → radiant star
    CatalogPreferred, // catalog.json flagged, no user override → filled star
    None,             // not preferred → empty star
}

fn star_state(entry: &CatalogEntry, user_preferred: &Option<String>) -> StarState {
    if user_preferred.as_deref() == Some(entry.file.as_str()) {
        StarState::UserPreferred
    } else if user_preferred.is_none() && entry.preferita.unwrap_or(false) {
        StarState::CatalogPreferred
    } else {
        StarState::None
    }
}

#[function_component(CatalogPanel)]
pub fn catalog_panel(props: &CatalogPanelProps) -> Html {
    let mut entries: Vec<&CatalogEntry> = props.catalog.iter().collect();
    entries.sort_by_key(|e| match star_state(e, &props.user_preferred) {
        StarState::UserPreferred | StarState::CatalogPreferred => 0,
        StarState::None => 1,
    });

    html! {
        <section class="upload-panel">
            <div class="upload-card">
                <p>{"Scegli una scheda predefinita o carica un file JSON personale."}</p>
                { if props.catalog_loading {
                    html! { <p class="hint">{"Caricamento catalogo schede in corso..."}</p> }
                } else if props.catalog.is_empty() {
                    html! { <p class="hint">{"Nessuna scheda disponibile nel catalogo."}</p> }
                } else {
                    html! {
                        <div class="catalog-list">
                            { for entries.iter().map(|entry| {
                                let item      = (*entry).clone();
                                let on_load   = props.on_load_catalog_entry.clone();
                                let state     = star_state(entry, &props.user_preferred);
                                let is_feat   = state != StarState::None;
                                let file_key  = entry.file.clone();
                                let on_set    = props.on_set_preferred.clone();
                                let user_pref = props.user_preferred.clone();
                                html! {
                                    <article
                                        class={classes!(
                                            "catalog-card",
                                            if is_feat { Some("catalog-card--featured") } else { None }
                                        )}
                                        onclick={Callback::from(move |_| on_load.emit(item.clone()))}
                                    >
                                        <div class="catalog-info">
                                            <div class="catalog-title-row">
                                                // Star lives here — left of title, same row, both layouts
                                                <button
                                                    class={classes!(
                                                        "star-btn",
                                                        if state != StarState::None { Some("star-btn--active") } else { None }
                                                    )}
                                                    title={match state {
                                                        StarState::UserPreferred    => "Rimuovi dai preferiti",
                                                        StarState::CatalogPreferred => "Segna come preferita",
                                                        StarState::None             => "Segna come preferita",
                                                    }}
                                                    onclick={Callback::from(move |e: MouseEvent| {
                                                        e.stop_propagation();
                                                        // Toggle: if already user-preferred, unset; otherwise set
                                                        let new_pref = if user_pref.as_deref() == Some(file_key.as_str()) {
                                                            None
                                                        } else {
                                                            Some(file_key.clone())
                                                        };
                                                        on_set.emit(new_pref);
                                                    })}
                                                >
                                                    { match state {
                                                        StarState::UserPreferred    => icon_star_radiant(),
                                                        StarState::CatalogPreferred => icon_star_filled(),
                                                        StarState::None             => icon_star_empty(),
                                                    }}
                                                </button>
                                                <span class="catalog-title">{ entry.display_name() }</span>
                                            </div>
                                            <div class="catalog-meta">{ entry.date_label() }</div>
                                        </div>
                                        <button class={if is_feat { "select-button select-button--featured" } else { "select-button" }}>
                                            {"Apri"}
                                        </button>
                                    </article>
                                }
                            }) }
                        </div>
                    }
                }}
                <div style="margin-top: 24px;">
                    <label class="file-label">
                        <span>{"Carica file JSON"}</span>
                        <input type="file" accept=".json" onchange={props.on_file_change.clone()} />
                    </label>
                </div>
                <p class="hint">{"Il file deve contenere un oggetto JSON con campi: id, nome, giorni -> esercizi."}</p>
            </div>
        </section>
    }
}
