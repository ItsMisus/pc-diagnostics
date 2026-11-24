# PC Diagnostics

App desktop per **Windows 11** e **macOS (Apple Silicon)** che monitora l'hardware, esegue benchmark e produce diagnostica del PC.

Mostra solo dati reali: se un valore non si può leggere, lo segna come non disponibile e spiega il motivo. Funziona offline, tiene tutti i dati in locale e non richiede diritti di amministratore per l'uso normale.

## Cosa fa

- **Monitoraggio hardware** in tempo reale: CPU, GPU, memoria, dischi, rete, temperature e ventole.
- **Benchmark** per misurare le prestazioni del sistema.
- **Diagnostica** con regole che individuano problemi e li spiegano in modo chiaro.
- **Assistente "Risolvi"** (opzionale, spento di default): con la tua chiave API di Claude propone un piano di correzione per un singolo problema. Non viene inviato nulla prima che tu veda i dati e accetti.
- Interfaccia in **italiano e inglese**.

## ⬇️ Scarica (V1 beta 1)

| Sistema | Download |
|---|---|
| **Windows 11** (x64) | [PC.Diagnostics_1.0.0-beta.1_x64-setup.exe](https://github.com/ItsMisus/pc-diagnostics-download/releases/download/v1.0.0-beta.1/PC.Diagnostics_1.0.0-beta.1_x64-setup.exe) (77 MB) |
| **Mac** (Apple Silicon, macOS 13.3+) | [PC.Diagnostics_1.0.0-beta.1_aarch64.dmg](https://github.com/ItsMisus/pc-diagnostics-download/releases/download/v1.0.0-beta.1/PC.Diagnostics_1.0.0-beta.1_aarch64.dmg) (111 MB) |

Ogni file ha il suo SHA-256 nella pagina [Releases](https://github.com/ItsMisus/pc-diagnostics-download/releases).

### Windows

1. Avvia l'installer: è per utente e non chiede diritti di amministratore.
2. Se compare "Windows ha protetto il PC", clicca **Ulteriori informazioni → Esegui comunque**. Succede perché l'app non è firmata con un certificato pubblico. Fallo solo se lo SHA-256 corrisponde a quello della release.

### Mac

1. Apri il `.dmg` e trascina **PC Diagnostics** in **Applicazioni**.
2. L'app non è notarizzata da Apple, quindi al primo avvio macOS la blocca:
   - **macOS 15 Sequoia e successivi**: fai doppio clic, scegli **Fine**, poi **Impostazioni di Sistema → Privacy e sicurezza → Apri comunque**.
   - **macOS 13 Ventura e 14 Sonoma**: clic destro sull'app → **Apri**, poi di nuovo **Apri**.

macOS ricorda la scelta: dagli avvii successivi si apre normalmente.

## Note

- È una **beta**: alcuni valori che esistono su Windows non esistono su macOS e restano segnati come non disponibili.
- Windows 10 x64 dovrebbe funzionare ma non è stato testato.
- Per segnalare un problema apri una [Issue](https://github.com/ItsMisus/pc-diagnostics-download/issues).
