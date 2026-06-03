# UberTrainingPlane

Webapp mobile-first per il tracciamento degli allenamenti in palestra. Scritta in Rust/Yew compilata in WebAssembly, distribuita come PWA su GitHub Pages. Funziona completamente offline dopo il primo caricamento.

## Demo

[Apri l'app](https://polpatch.github.io/UberTrainingPlane/)

## Funzionalità

- **Catalogo schede** — scegli una scheda predefinita o carica un file JSON personale
- **Bottom sheet esercizio** — sheet minimizzabile con controlli peso/reps, progress bar, timer recupero
- **Timer recupero** — toast flottante con countdown SVG, pausa/riprendi/salta
- **Registrazione serie** — con feedback visivo (pulse animation), vibrazione haptica (Android), auto-chiusura tastiera
- **Video esercizio** — popup YouTube per gli esercizi con campo `video` nel JSON
- **Progresso sessione** — badge "X / Y serie" nell'header, timer elapsed
- **Schermata completamento** — modal con riepilogo serie, peso totale, durata
- **Storico sessioni** — visualizza e riprendi sessioni precedenti
- **Dark mode** — automatica via `prefers-color-scheme`
- **PWA** — installabile su Android e iOS, offline-first via Service Worker
- **Wake Lock** — impedisce lo standby durante l'allenamento
- **Export / Import** — backup JSON di tutte le schede e sessioni

## Tech Stack

| Componente | Tecnologia |
|---|---|
| Linguaggio | Rust (edition 2021), WASM via `wasm32-unknown-unknown` |
| Framework UI | [Yew 0.20](https://yew.rs) (CSR) |
| Build tool | [Trunk](https://trunkrs.dev) |
| Storage | `gloo-storage` (LocalStorage) |
| HTTP | `gloo-net` |
| Timer | `gloo-timers` |
| Deploy | GitHub Actions → GitHub Pages |

## Struttura del progetto

```
app/
├── src/
│   ├── lib.rs                  # Componente App principale + state management
│   ├── models.rs               # Tipi dati + funzioni LocalStorage
│   └── components/
│       ├── bottom_sheet.rs     # Sheet esercizio attivo
│       ├── catalog_panel.rs    # Selezione scheda iniziale
│       ├── day_tabs.rs         # Tab giorni allenamento
│       ├── exercise_card.rs    # Card compatta esercizio
│       └── progress_bar.rs     # Barra pallini serie (con carousel)
├── schede/                     # Schede JSON predefinite
│   └── catalog.json            # Indice schede
├── icons/                      # Icone PWA (SVG + PNG)
├── manifest.json               # Web App Manifest
├── sw.js                       # Service Worker
└── index.html                  # Entrypoint HTML + CSS
examples/
└── schede/
    └── workout_schema.json     # JSON Schema per validazione schede
```

## Come eseguire in locale

```bash
cd app
trunk serve          # dev server con hot reload → http://localhost:8080
```

Build di produzione:

```bash
cd app
trunk build --release
```

## Creare una scheda personalizzata

Scarica il template dal menu burger dell'app ("Scarica template JSON") oppure prendilo da `examples/schede/`. Il formato completo:

```json
{
  "id": "mia_scheda",
  "nome": "La mia scheda",
  "descrizione": "Opzionale",
  "categoria": "Ipertrofia",
  "giorni": [
    {
      "giorno": "A",
      "etichetta": "Giorno A — Spinta",
      "esercizi": [
        {
          "id": "panca_piana",
          "nome": "Panca Piana",
          "serie": 4,
          "reps": "8-10",
          "recupero": 120,
          "video": "https://www.youtube.com/watch?v=...",
          "note": "Opzionale"
        }
      ]
    }
  ]
}
```

Valida il file con lo schema in `examples/schede/workout_schema.json` (supporta autocomplete in VS Code aggiungendo `"$schema": "./workout_schema.json"` al JSON).

Per aggiungere una scheda al catalogo predefinito: metti il file in `app/schede/` e aggiungi una voce in `app/schede/catalog.json`. Aggiungi `"preferita": true` per mostrarla in cima con evidenziazione.

## Deploy

Il deploy avviene automaticamente tramite GitHub Actions al push su `master`. Il branch di sviluppo è `develop` — fare merge su `master` per rilasciare.

```bash
git checkout master
git merge develop
git push origin master
```

## Dati e privacy

Tutti i dati (schede, sessioni, progressi) sono salvati esclusivamente in `LocalStorage` sul dispositivo dell'utente. Nessun dato viene inviato a server esterni.
