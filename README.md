# Piattaforma Allenamento Estremo

Web app statica hostata su **GitHub Pages** per gestire e tracciare le schede di allenamento in palestra, giorno per giorno.

> URL: `https://polpatch.github.io/GitHubPageTest/`

---

## Come funziona l'app

### 1. Schermata di selezione scheda

All'apertura l'app legge `scheda.json` dalla stessa cartella del repository e mostra una **griglia di card**, una per ogni scheda presente nel file. Ogni card mostra nome, categoria, descrizione e i giorni della settimana associati.

### 2. Selezione del giorno

Cliccando su una scheda si accede alla schermata dei giorni, con **tab cliccabili** (Lunedì, Martedì, ecc.). Toccando un tab si carica direttamente il programma di quel giorno.

### 3. Visualizzazione esercizi

Per ogni giorno vengono mostrate card esercizio con:
- **Nome**, numero di serie, range di reps e tempo di recupero previsto
- **Note tecniche** in giallo (consigli di esecuzione)
- **Pallini serie** cliccabili (uno per ogni serie)
- **Campi input**: peso usato, reps effettive, note libere

### 4. Pallini serie e timer

Cliccando un pallino si segna la serie come completata (arancione). Automaticamente parte il **timer di recupero** con il countdown in secondi configurato nell'esercizio (default: 90s). Il timer:
- Mostra un display grande con countdown
- Ha una barra di progresso
- Diventa giallo negli ultimi 10 secondi
- Emette 3 bip audio alla scadenza con Web Audio API
- Ha tre pulsanti: **+30s**, **Pausa/Riprendi**, **Salta**

L'ultimo pallino (ultima serie) completa l'esercizio senza avviare il timer.

### 5. Salvataggio sessione

Il pulsante **"Salva sessione"** in cima alla schermata allenamento salva tutti i dati inseriti (peso, reps, note) nel **localStorage** del browser, associandoli alla data odierna. I dati sono quindi disponibili offline e persistono tra le sessioni.

### 6. Storico per esercizio

Il pulsante **"Storico"** su ogni card apre un drawer laterale con tutte le sessioni salvate per quell'esercizio in ordine cronologico inverso.

### 7. Navigazione

- **Freccia indietro** su ogni schermata per tornare al livello precedente
- **"Cambia scheda"** nell'header per tornare alla selezione principale

---

## Struttura del repository

```
GitHubPageTest/
├── index.html       ← Web app principale (GitHub Pages entry point)
├── scheda.json      ← Schede di allenamento (array JSON)
└── README.md        ← Questa documentazione
```

---

## Attivare GitHub Pages

1. Vai su **Settings → Pages**
2. Source: **Deploy from a branch**
3. Branch: **main** / root
4. Salva — l'app sarà disponibile su `https://polpatch.github.io/GitHubPageTest/`

---

## Come costruire la scheda (`scheda.json`)

Il file è un **array JSON** di oggetti scheda. Puoi inserire quante schede vuoi — verranno tutte mostrate nella griglia iniziale.

### Struttura completa

```json
[
  {
    "id": "nome_univoco_mese_anno",
    "nome": "Nome visualizzato nella card",
    "categoria": "Push | Pull | Legs | Full Body | ...",
    "descrizione": "Breve descrizione della scheda",
    "giorni": [
      {
        "giorno": "Lunedì",
        "etichetta": "Push A",
        "esercizi": [
          {
            "id": "id_univoco_esercizio",
            "nome": "Nome esercizio",
            "serie": 4,
            "reps": "6-8",
            "recupero": 120,
            "note": "Indicazione tecnica visualizzata in giallo"
          }
        ]
      }
    ]
  }
]
```

### Campi degli esercizi

| Campo | Tipo | Obbligatorio | Note |
|---|---|---|---|
| `id` | stringa | ✅ | Chiave per localStorage — **non cambiare** dopo il primo uso |
| `nome` | stringa | ✅ | Testo visualizzato nella card esercizio |
| `serie` | intero | ✅ | Determina il numero di pallini |
| `reps` | stringa | ✅ | Testo libero: `"6-8"`, `"12 per gamba"`, `"30s"` |
| `recupero` | intero | ❌ | Secondi del timer; se assente usa 90s come default |
| `note` | stringa | ❌ | Indicazione tecnica in giallo sotto il titolo |

### Convenzione nomi `id`

- **Scheda**: `categoria_progressivo_mese-anno` → `push_a_maggio2026`
- **Esercizio**: `nome_esercizio_in_snake_case` → `panca_piana`, `leg_curl`
- L'`id` dell'esercizio è la chiave con cui i dati vengono salvati nel localStorage: **non modificarlo** dopo aver iniziato a registrare sessioni, altrimenti lo storico viene perso.

---

## Note tecniche

- App **100% statica** — nessun server, nessun backend
- Dati salvati in **localStorage** del browser (per dispositivo)
- Lettura scheda via `fetch('./scheda.json')` — funziona correttamente su GitHub Pages
- Audio timer tramite **Web Audio API** (nessuna dipendenza esterna)
- Font: **Bebas Neue** (display) + **Inter** (body) via Google Fonts
- Compatibile con dispositivi mobili (touch target 44px, layout responsive)
