# Piattaforma Allenamento Estremo

Web app statica hostata su **GitHub Pages** per gestire e tracciare le schede di allenamento in palestra, giorno per giorno.

> URL GitHub Pages: `https://polpatch.github.io/GitHubPageTest/`

---

## Struttura del repository

```
GitHubPageTest/
├── index.html          ← Web app (entry point GitHub Pages)
├── catalog.json        ← Indice delle schede disponibili
├── README.md
└── schede/
    ├── push_a.json     ← Esempio scheda Push
    ├── pull_a.json     ← Esempio scheda Pull
    └── legs_a.json     ← Esempio scheda Legs
```

Ogni scheda è un **file JSON separato** nella cartella `schede/`. Il file `catalog.json` funge da indice: contiene i riferimenti ai file e le info di anteprima mostrate nella griglia iniziale.

---

## Come funziona l'app

### 1. Selezione scheda
All'apertura l'app legge `catalog.json` e mostra una griglia di card, una per ogni scheda registrata. Cliccando una card viene caricato il file JSON corrispondente dalla cartella `schede/`.

### 2. Selezione giorno
Dopo aver caricato una scheda, vengono mostrati i **tab cliccabili** con i giorni di allenamento definiti nel file JSON. Ogni giorno può avere un proprio set di esercizi.

### 3. Esercizi e serie
Per ogni giorno viene visualizzata la lista degli esercizi, ognuno con:
- Nome, numero di serie, range reps e tempo di recupero
- **Note tecniche** in giallo
- **Pallini serie** cliccabili (uno per ogni serie dell'esercizio)
- Campi input: **peso usato**, **reps effettive**, **note libere**

### 4. Timer di recupero
Cliccando un pallino (tranne l'ultimo della serie) parte automaticamente il **timer di recupero** con il countdown in secondi specificato nel JSON (`recupero`, default 90s).

Funzionalità timer:
- Countdown grande con barra di progresso
- Display giallo negli ultimi 10 secondi
- 3 bip audio alla scadenza (Web Audio API)
- Pulsanti: **+30s**, **Pausa/Riprendi**, **Salta**

### 5. Salvataggio sessione
Il pulsante **"Salva sessione"** salva i dati di peso, reps e note nel **localStorage** del browser, associati alla data odierna e al giorno di allenamento selezionato.

### 6. Storico per esercizio
Il pulsante **"Storico"** su ogni esercizio apre un drawer laterale con tutte le sessioni precedenti salvate per quell'esercizio, in ordine cronologico inverso.

### 7. Navigazione
- Freccia **"Tutte le schede"** → torna alla selezione scheda
- Freccia **"Torna ai giorni"** → torna alla selezione giorno
- Pulsante **"Cambia scheda"** nell'header → stesso effetto

---

## Attivare GitHub Pages

1. **Settings → Pages**
2. Source: **Deploy from a branch**
3. Branch: **main** / root
4. Salva — l'app sarà disponibile su `https://polpatch.github.io/GitHubPageTest/`

---

## Aggiungere una nuova scheda

### 1. Crea il file JSON in `schede/`

Nomina il file con un nome descrittivo, es. `schede/fullbody_b.json`.

```json
{
  "id": "fullbody_b",
  "nome": "Full Body B",
  "categoria": "Full Body",
  "descrizione": "Allenamento completo — Lunedì, Mercoledì, Venerdì",
  "giorni": [
    {
      "giorno": "Lunedì",
      "etichetta": "Full Body B — W1",
      "esercizi": [
        {
          "id": "squat",
          "nome": "Squat",
          "serie": 4,
          "reps": "6-8",
          "recupero": 120,
          "note": "Bilanciere, scendi a 90° o più in basso"
        }
      ]
    }
  ]
}
```

### 2. Aggiungi la voce in `catalog.json`

```json
[
  ...
  {
    "file": "schede/fullbody_b.json",
    "nome": "Full Body B",
    "categoria": "Full Body",
    "descrizione": "Allenamento completo — Lunedì, Mercoledì, Venerdì"
  }
]
```

---

## Schema campi esercizio

| Campo | Tipo | Obbligatorio | Descrizione |
|---|---|---|---|
| `id` | stringa | ✅ | Chiave localStorage — **non cambiare** dopo il primo uso |
| `nome` | stringa | ✅ | Nome visualizzato nella card |
| `serie` | intero | ✅ | Numero di pallini |
| `reps` | stringa | ✅ | Testo libero: `"6-8"`, `"12 per gamba"`, `"30s"` |
| `recupero` | intero | ❌ | Secondi timer di recupero; default 90s se omesso |
| `note` | stringa | ❌ | Indicazione tecnica visualizzata in giallo |

---

## Note tecniche

- App **100% statica** — nessun server, nessun backend
- Schede caricate via `fetch()` da GitHub Pages (funziona correttamente)
- Dati salvati in **localStorage** per dispositivo
- Audio timer via **Web Audio API** (nessuna dipendenza esterna)
- Font: **Bebas Neue** (display) + **Inter** (body) via Google Fonts
- Compatibile mobile (touch target ≥44px, layout responsive)
