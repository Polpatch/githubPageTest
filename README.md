# 🏋️ Piattaforma Allenamento Estremo

Visualizzatore di schede di allenamento con tracciamento dei pesi, timer di recupero e storico sessioni. Progetto statico deployato via **GitHub Pages**.

## 🔗 Link

> Attivare GitHub Pages da **Settings → Pages → Branch: main / root**

---

## ⚙️ Come funziona l'app

### 1. Caricamento scheda
All'apertura, la pagina fa un `fetch` del file `scheda.json` nella root del repository. Se il file è assente o malformato, viene mostrato un messaggio di errore.

### 2. Selezione scheda e giorno
- Il menu a tendina in cima mostra tutte le schede definite nel JSON (un array).
- I tab colorati sotto permettono di passare da un giorno all'altro (es. Giorno A, Giorno B…).
- Cambiare scheda o giorno **azzera la sessione corrente** senza toccare i dati salvati.

### 3. Tabella esercizi
Ogni esercizio mostra:
- **Nome** + note tecniche in grigio.
- **Pallini serie** (dot): cliccando un pallino si segna quella serie come completata (arancio). Ri-cliccando l'ultimo dot completato si de-segna.
- **Campo peso** (kg): valore numerico modificabile, precompilato con l'ultimo peso salvato per quell'esercizio.
- **Badge recupero**: tempo di recupero previsto dalla scheda.
- **Icona orologio**: apre il drawer storico dell'esercizio.

### 4. Timer di recupero
Il timer si avvia **automaticamente** ogni volta che si clicca un pallino serie (tranne l'ultimo che completa l'esercizio).

- Si apre come overlay scuro centrale con countdown grande.
- **Barra di progresso** che si svuota con il tempo.
- A 10 secondi dalla fine il display diventa giallo (warning).
- Allo scadere suona tre brevi bip (Web Audio API) e si chiude da solo dopo 1.8 s.
- Pulsanti: **Salta** (chiude subito), **Pausa/Riprendi**, **+30s** (aggiunge 30 secondi).
- Cliccando fuori dall'overlay si chiude senza aspettare.

### 5. Completamento esercizio
Quando si segnano **tutte le serie** di un esercizio, appare un toast `✅ Esercizio completato!` invece di avviare il timer.

### 6. Salvataggio sessione
Ogni modifica (peso o serie) fa comparire una **barra fissa in basso** con due azioni:
- **Salva sessione**: scrive su `localStorage` l'ultimo peso inserito per ogni esercizio e aggiunge una voce allo storico (data, peso, serie completate).
- **Scarta**: annulla le modifiche e ricarica i valori salvati in precedenza.

### 7. Storico per esercizio
Il drawer laterale destro mostra tutte le sessioni passate per quell'esercizio, dalla più recente alla più vecchia, con data, peso e numero di serie.

---

## 📁 Struttura repository

```
GitHubPageTest/
├── index.html      ← App web (unico file HTML)
├── scheda.json     ← Schede di allenamento
└── README.md
```

---

## 📋 Come costruire la scheda (`scheda.json`)

Il file è un **array JSON di schede**. Ogni scheda ha questa struttura:

```json
[
  {
    "id": "nome-scheda-unico",
    "nome": "Nome visualizzato nel menu",
    "descrizione": "Descrizione opzionale",
    "giorni": [
      {
        "nome": "Giorno A — Push",
        "esercizi": [
          {
            "id": "id-unico-esercizio",
            "nome": "Nome esercizio",
            "serie": 4,
            "reps": "6-8",
            "recupero": 180,
            "note": "Indicazioni tecniche"
          }
        ]
      }
    ]
  }
]
```

### Campi esercizio

| Campo | Tipo | Obbligatorio | Descrizione |
|---|---|---|---|
| `id` | stringa | ✅ | Identificatore univoco — **non cambiare mai** dopo aver iniziato ad usarlo, è la chiave del localStorage |
| `nome` | stringa | ✅ | Nome visualizzato nella tabella |
| `serie` | intero | ✅ | Numero di serie — determina quanti pallini vengono mostrati |
| `reps` | stringa | ✅ | Libero: `"6-8"`, `"10-12"`, `"max"`, `"5 min"` |
| `recupero` | intero | ❌ | Secondi di recupero tra serie. Default: **90 secondi** se omesso |
| `note` | stringa | ❌ | Indicazioni tecniche mostrate in grigio sotto al nome |

### Regole importanti

- L'`id` della scheda e l'`id` di ogni esercizio sono usati come **chiavi nel localStorage**: non modificarli una volta che hai iniziato ad usare la scheda, altrimenti perdi lo storico.
- Puoi avere **più schede** nello stesso file, tutte accessibili dal menu a tendina.
- Il campo `reps` è una stringa libera: puoi scrivere `"6-8"`, `"max"`, `"AMRAP"`, `"20 min steady state"` — viene mostrato così com'è.
- Il `recupero` è in **secondi** (180 = 3 minuti).

### Esempio scheda minima funzionante

```json
[
  {
    "id": "full-body-a",
    "nome": "Full Body A",
    "giorni": [
      {
        "nome": "Giorno 1",
        "esercizi": [
          { "id": "squat",      "nome": "Squat",      "serie": 3, "reps": "8-10", "recupero": 120 },
          { "id": "panca",      "nome": "Panca Piana", "serie": 3, "reps": "8-10", "recupero": 120 },
          { "id": "trazioni",   "nome": "Trazioni",   "serie": 3, "reps": "max",  "recupero": 90  }
        ]
      }
    ]
  }
]
```
