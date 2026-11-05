# MISSIONE: UI/UX REDESIGN & ANIMATION OVERHAUL

(Brief fornito dall'utente il 2026-11-04, salvato testualmente. Da eseguire nella prossima sessione.)

Agisci come un team di esperti composto da Senior UI/UX Designer, Motion Designer, Senior React Engineer, Tauri Desktop Engineer e QA Engineer.

Il tuo compito è analizzare, correggere e perfezionare l'intera interfaccia della mia applicazione, portandola a uno standard visivo e qualitativo paragonabile alle migliori applicazioni desktop moderne, come Linear, Raycast e le applicazioni native Apple.

Non limitarti a proporre modifiche: **analizza il progetto e implementa concretamente tutti i miglioramenti necessari.**

## 1. STRUMENTI E SKILL

Verifica la disponibilità dei seguenti strumenti:

- **Frontend Design:** per progettare un'interfaccia premium, minimalista e coerente.
- **Motion AI Kit:** per realizzare animazioni e transizioni professionali con Motion for React.
- **Emil Kowalski Skills:** utilizza, se disponibili, `animate`, `review-animations`, `improve-animations` e `apple-design`.
- **Motion for React:** come libreria principale per le animazioni.
- **Playwright:** per verificare visivamente le interfacce web, quando compatibile con l'ambiente.

Se uno strumento non è disponibile, verifica la procedura ufficiale di installazione. Non inventare comandi, non installare dipendenze inutili e non bloccare il lavoro per un plugin opzionale.

Usa la documentazione aggiornata e le API compatibili con la versione del progetto.

## 2. AUDIT COMPLETO DELL'APPLICAZIONE

Prima di modificare qualsiasi codice:

1. Analizza la struttura completa del frontend.
2. Identifica framework, librerie UI, componenti, sistema di routing e gestione dello stato.
3. Individua problemi estetici, funzionali e di navigazione.
4. Controlla eventuali animazioni già presenti.
5. Verifica la compatibilità con Tauri 2.
6. Individua componenti duplicati, CSS ridondante e stili incoerenti.
7. Analizza eventuali problemi di performance e rendering.

Definisci un piano d'intervento ordinato per priorità e poi procedi all'implementazione.

## 3. REDESIGN VISIVO PREMIUM

Voglio un'interfaccia estremamente curata, minimalista, moderna e professionale.

Direzione estetica:

- Ispirazione a Linear, Raycast e macOS.
- Dark mode elegante e raffinata.
- Palette neutra con accenti cromatici discreti.
- Tipografia moderna con gerarchia visiva chiara.
- Spaziature coerenti e proporzioni equilibrate.
- Bordi sottili e ombre leggere dove necessarie.
- Componenti visivamente uniformi.
- Iconografia consistente.
- Dashboard ordinate e leggibili.
- Nessun effetto decorativo eccessivo.
- Nessun aspetto generico tipico delle interfacce generate automaticamente dall'IA.

Crea o migliora un design system centralizzato che definisca colori, tipografia, spaziature, raggi, ombre, stati interattivi e principi di animazione.

Non riscrivere componenti già validi senza una motivazione tecnica.

## 4. ANIMAZIONI E TRANSIZIONI

Implementa un sistema di animazioni coerente utilizzando Motion for React, dove appropriato.

### Navigazione

- Transizioni fluide tra le pagine.
- Entrate e uscite naturali.
- Animazioni coordinate con il sistema di routing.
- Nessun flickering o salto visivo.
- Evita di animare inutilmente ogni elemento.

### Sidebar

- Apertura e chiusura fluide.
- Animazione delle dimensioni e del contenuto.
- Indicatori di selezione animati.
- Transizioni eleganti per icone ed etichette.
- Nessuno spostamento improvviso del layout.

### Componenti interattivi

- Feedback hover discreto.
- Feedback alla pressione dei pulsanti.
- Animazioni degli switch.
- Checkbox e selettori con transizioni.
- Dropdown e menu contestuali animati.
- Tooltip con comparsa naturale.
- Stati di focus chiaramente visibili.

### Modali e notifiche

- Animazioni di apertura e chiusura.
- Transizioni spring controllate.
- Backdrop elegante.
- Toast animati.
- Stati di conferma ed errore coerenti.
- Corretta gestione del focus e della tastiera.

### Dashboard e grafici

- Apparizione progressiva dei dati, solo quando appropriata.
- Aggiornamenti animati senza perdere leggibilità.
- Transizioni fluide nei grafici.
- Skeleton loading discreti.
- Indicatori di caricamento coerenti.
- Nessuna animazione continua superflua.

## 5. MOTION DESIGN SYSTEM

Definisci un sistema centralizzato di animazioni.

Valori iniziali suggeriti:

- Microinterazioni: 100–180 ms.
- Transizioni di componenti: 180–280 ms.
- Transizioni di navigazione: 220–350 ms.
- Spring naturali con damping appropriato.
- Easing coerenti in tutta l'applicazione.

Adatta questi valori al contesto e alla percezione dell'utente.

Privilegia animazioni basate su `transform` e `opacity`.

Evita layout thrashing, rendering inutili e animazioni pesanti.

Supporta `prefers-reduced-motion`.

## 6. CORREZIONE DEI PROBLEMI UI/UX

Non occuparti soltanto dell'estetica.

Individua e correggi:

- Elementi disallineati.
- Spaziature incoerenti.
- Componenti che si sovrappongono.
- Scrollbar indesiderate.
- Layout che si rompono quando la finestra viene ridimensionata.
- Pulsanti con stati visivi mancanti.
- Menu che si aprono nella posizione sbagliata.
- Problemi di contrasto.
- Transizioni interrotte.
- Flickering.
- Problemi di navigazione da tastiera.
- Componenti con dimensioni sproporzionate.
- Problemi di leggibilità dei grafici.
- Eventuali errori frontend già presenti.

Mantieni una corretta esperienza d'uso anche con finestre desktop di dimensioni ridotte.

## 7. PERFORMANCE E OTTIMIZZAZIONE

L'applicazione deve rimanere estremamente reattiva.

- Riduci re-render non necessari.
- Evita animazioni costose.
- Utilizza correttamente React e Motion.
- Evita memory leak e listener non rimossi.
- Controlla le prestazioni delle pagine più complesse.
- Mantieni reattivi i controlli durante gli aggiornamenti dei dati.
- Ottimizza le animazioni quando sono presenti numerosi componenti.

Nel caso di dashboard con monitoraggio hardware in tempo reale, evita che l'aggiornamento frequente delle metriche provochi continui riavvii delle animazioni.

Non sacrificare prestazioni e usabilità per ottenere effetti visivi.

## 8. REGOLE DI SICUREZZA DEL PROGETTO

Queste regole sono obbligatorie:

1. Non alterare la logica di business.
2. Non modificare il backend Rust se non strettamente necessario e motivato.
3. Non eliminare funzionalità esistenti.
4. Non modificare arbitrariamente le API di comunicazione tra React e Tauri.
5. Non introdurre dipendenze senza verificarne la necessità.
6. Non modificare la struttura del progetto senza una ragione precisa.
7. Mantieni compatibilità con le versioni installate.
8. Preserva le preferenze e le impostazioni dell'utente.
9. Evita modifiche distruttive.
10. Non dichiarare funzionante una modifica senza verificarla.

## 9. TEST E VALIDAZIONE

Dopo ogni gruppo significativo di modifiche:

- Esegui type checking.
- Esegui linting, se configurato.
- Esegui i test esistenti.
- Verifica la compilazione del frontend.
- Verifica il funzionamento con Tauri.
- Controlla che la navigazione sia corretta.
- Controlla modali, dropdown e sidebar.
- Verifica il comportamento con finestra ridimensionata.
- Verifica che non siano presenti errori nella console.
- Controlla il supporto a reduced motion.

Quando possibile, utilizza screenshot e strumenti di test visuale per confrontare il risultato prima e dopo.

Non considerare concluso il lavoro finché non sono stati verificati i componenti modificati.

## 10. ORGANIZZAZIONE DEL LAVORO CON SUBAGENTI

Se l'ambiente supporta subagenti, suddividi il lavoro in:

**Agent UI/UX Designer**
Analisi dell'interfaccia, design system, tipografia, colori e layout.

**Agent Motion Designer**
Progettazione e implementazione di animazioni, transizioni e microinterazioni.

**Agent Frontend Engineer**
Implementazione React e TypeScript, integrazione Motion e ottimizzazione dei componenti.

**Agent QA & Performance**
Test, individuazione di regressioni, compatibilità Tauri e valutazione delle prestazioni.

Coordina le attività evitando modifiche simultanee e incompatibili sugli stessi file.

Se non sono disponibili subagenti, esegui le stesse fasi in sequenza.

## 11. CRITERI DI COMPLETAMENTO

Il lavoro sarà completo quando:

- Tutte le sezioni principali hanno uno stile coerente.
- Le animazioni sono naturali e non invasive.
- La navigazione è fluida.
- Sidebar, modali, menu e notifiche funzionano correttamente.
- Il layout è stabile durante il ridimensionamento.
- Le prestazioni non sono peggiorate.
- I test disponibili risultano superati.
- Nessuna funzionalità preesistente è stata compromessa.

Al termine fornisci un report con:

1. Problemi individuati.
2. Modifiche effettivamente implementate.
3. Dipendenze aggiunte o aggiornate.
4. File modificati.
5. Test effettuati e relativi risultati.
6. Eventuali limitazioni o problemi ancora aperti.

## ISTRUZIONE FINALE

Voglio che l'applicazione trasmetta immediatamente una sensazione di qualità, precisione e fluidità.

Ogni transizione deve avere uno scopo.

Ogni animazione deve migliorare l'esperienza utente.

Ogni componente deve sembrare parte dello stesso prodotto.

L'obiettivo non è avere più animazioni possibili, ma ottenere un'interfaccia desktop raffinata, moderna, stabile e piacevole da utilizzare.

**Analizza il progetto, definisci il piano e procedi con l'implementazione completa. Non fermarti alla sola analisi o alla generazione di suggerimenti.**
