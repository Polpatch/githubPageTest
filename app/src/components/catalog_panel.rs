use crate::models::CatalogEntry;
use web_sys::Event;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CatalogPanelProps {
    pub catalog: Vec<CatalogEntry>,
    pub catalog_loading: bool,
    pub on_load_catalog_entry: Callback<CatalogEntry>,
    pub on_file_change: Callback<Event>,
}

#[function_component(CatalogPanel)]
pub fn catalog_panel(props: &CatalogPanelProps) -> Html {
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
                            { for props.catalog.iter().map(|entry| {
                                let item = entry.clone();
                                let on_load = props.on_load_catalog_entry.clone();
                                html! {
                                    <article class="catalog-card" onclick={Callback::from(move |_| on_load.emit(item.clone()))}>
                                        <div class="catalog-info">
                                            <div class="catalog-title">{ format!("{} {}", entry.nome, entry.numero.clone().unwrap_or_default()) }</div>
                                            <div class="catalog-meta">{ format!("{} / {}", entry.mese.clone().unwrap_or_default(), entry.anno.clone().unwrap_or_default()) }</div>
                                        </div>
                                        <button class="select-button">{"Apri"}</button>
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
