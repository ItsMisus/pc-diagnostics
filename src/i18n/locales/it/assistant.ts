/** Italian (reference) texts for the "assistant" namespace: the "Risolvi" assistant. */
export default {
  // Button
  button: "Risolvi",
  buttonAria: "Risolvi: {label}",
  buttonTitle: "Chiedi all'assistente AI come risolverlo (ti mostra prima cosa verrebbe inviato)",

  // Not ready (model.ts)
  setupStepOpenSettings: "Apri Settings e cerca la sezione «Assistente Risolvi (AI)».",
  setupStepEnable: "Attiva l'assistente.",
  setupStepKey: "Incolla la tua chiave personale di Anthropic e premi «Salva chiave».",
  disabledTitle: "L'assistente Risolvi è spento",
  disabledExplanation:
    "Risolvi chiede a un'intelligenza artificiale (Claude di Anthropic) come sistemare il problema. Per scelta è spento finché non decidi di usarlo. Nessun dato è stato inviato.",
  noKeyTitle: "Manca la chiave di Anthropic",
  noKeyExplanation:
    "L'assistente è attivo, ma serve la tua chiave personale di Anthropic: l'eventuale costo delle richieste viene addebitato sul tuo account. Nessun dato è stato inviato.",

  // Models (model.ts)
  modelRecommended: "{model} (consigliato)",
  modelHintSonnet55: "Buon equilibrio tra qualità e costo (~0,03 $ a richiesta)",
  modelHintOpus55: "Per i problemi più difficili (~0,06–0,10 $)",
  modelHintOpus45: "Generazione precedente (~0,10–0,18 $)",
  modelHintHaiku55: "Il più economico, per problemi semplici (~0,002 $)",

  // Plan (model.ts)
  risk_low: "Rischio basso",
  risk_medium: "Rischio medio",
  risk_high: "Rischio alto",
  planConfidence_high: "L'AI ritiene probabile che questi passaggi risolvano il problema",
  planConfidence_medium: "L'AI ritiene possibile che questi passaggi risolvano il problema",
  planConfidence_low: "L'AI non è sicura che questi passaggi risolvano il problema",
  planConfidenceShort_high: "Fiducia alta",
  planConfidenceShort_medium: "Fiducia media",
  planConfidenceShort_low: "Fiducia bassa",

  // Actions (model.ts)
  settingsPage_windowsUpdate: "Windows Update",
  settingsPage_windowsUpdateOptional: "Aggiornamenti facoltativi",
  settingsPage_storageSense: "Sensore memoria",
  settingsPage_display: "Schermo",
  settingsPage_sound: "Audio",
  settingsPage_bluetooth: "Bluetooth e dispositivi",
  settingsPage_network: "Rete e Internet",
  settingsPage_power: "Alimentazione",
  settingsPage_troubleshoot: "Risoluzione dei problemi",
  settingsPage_startupApps: "App di avvio",
  settingsPage_installedApps: "App installate",
  settingsPage_privacy: "Privacy e sicurezza",
  days_one: "{count} giorno",
  days_other: "{count} giorni",
  action_openSettingsPage:
    "Aprirà la pagina «{page}» delle Impostazioni di Windows. Da sola non cambia nulla: sarai tu a decidere cosa modificare.",
  "action_openSettingsPage@macos":
    "Apre la pagina «{page}» delle Impostazioni di Windows: esiste solo su Windows, quindi su questo Mac l'app non la esegue.",
  action_openDeviceManager: "Aprirà Gestione dispositivi di Windows. Da sola non cambia nulla: sarai tu a decidere cosa fare.",
  "action_openDeviceManager@macos":
    "Apre Gestione dispositivi di Windows: esiste solo su Windows, quindi su questo Mac l'app non la esegue.",
  action_openDiskCleanup: "Aprirà Pulizia disco di Windows per l'unità {drive}. Sarai tu a scegliere cosa eliminare.",
  "action_openDiskCleanup@macos":
    "Apre Pulizia disco di Windows: esiste solo su Windows, quindi su questo Mac l'app non la esegue.",
  action_emptyRecycleBin: "Svuoterà il Cestino. Windows ti chiederà conferma. Non si può annullare.",
  "action_emptyRecycleBin@macos": "Svuoterà il Cestino. Non si può annullare.",
  action_cleanTempFiles:
    "Eliminerà i file temporanei più vecchi di {days} dalla tua cartella dei file temporanei. I file ancora in uso vengono saltati. Non si può annullare.",
  action_closeProgram: "Chiederà a «{name}» di chiudersi, come cliccare la X. Salva prima il tuo lavoro.",
  "action_closeProgram@macos": "Chiederà a «{name}» di chiudersi. Salva prima il tuo lavoro.",
  action_runSystemFileCheck:
    "Windows ti chiederà il permesso di amministratore, poi si aprirà una finestra che controlla e ripara i file di sistema di Windows. Può richiedere 10–20 minuti: lascia la finestra aperta fino alla fine.",
  "action_runSystemFileCheck@macos":
    "Avvia il controllo dei file di sistema di Windows: esiste solo su Windows, quindi su questo Mac l'app non lo esegue.",

  // Numbers (model.ts)
  costZero: "0 $",
  costBelowCent: "meno di 0,01 $",
  costApprox: "≈ {amount} $",
  tokenUsage: "{input} token inviati · {output} token ricevuti",

  // Dialog
  dialogTitle: "Risolvi con l'assistente AI",
  close: "Chiudi",
  cancel: "Annulla",
  waitForStep: "Attendi la fine del passaggio in corso",
  checking: "Verifica dell'assistente…",
  preparing: "Preparazione dei dati da inviare…",
  nothingSentYet: "Ancora nessun dato è stato inviato.",
  howToEnable: "Come attivarlo",
  openSettings: "Apri Settings",
  whatWillBeSent: "Cosa verrà inviato",
  previewIntro:
    "Controlla i dati qui sotto. Nulla viene inviato finché non premi «Invia e chiedi una soluzione». Nome del PC, nome utente e percorsi dei file sono già stati rimossi.",
  previewRefreshed:
    "L'anteprima precedente era scaduta: i dati sono stati preparati di nuovo. Controllali e premi ancora «Invia».",
  destination: "Destinazione",
  model: "Modello",
  estimatedCost: "Costo stimato",
  costNote: "(stima, addebitata sul tuo account Anthropic)",
  payloadSize: "Dimensione dei dati",
  exactDataAria: "Dati esatti che verranno inviati",
  showExactData: "Mostra i dati esatti",
  exactData: "Dati esatti",
  sendAndAsk: "Invia e chiedi una soluzione",
  requesting: "{model} sta preparando una proposta…",
  requestingDescription:
    "Di solito servono 30–60 secondi. Se annulli, la risposta verrà ignorata; la richiesta è però già partita e l'eventuale costo resta.",
  errorReportChanged: "Il report diagnostico è cambiato",
  errorStatus: "Impossibile verificare l'assistente",
  errorPreview: "Impossibile preparare i dati da inviare",
  errorRequest: "La richiesta all'assistente non è riuscita",
  errorReportChangedDescription:
    "Nel frattempo è stata eseguita una nuova diagnosi. Chiudi e premi di nuovo «Risolvi» sul problema aggiornato.",
  errorRequestDescription: "Riprovando, i dati verranno preparati di nuovo e ti saranno mostrati prima dell'invio.",
  errorReportChangedDetail: "Il report più recente è cambiato: attesa la regola {expected}, l'anteprima riguarda {got}",

  // Plan view
  proposal: "Proposta",
  warnings: "Attenzione",
  noSteps: "L'assistente non ha proposto passaggi da eseguire.",
  proposedSteps: "Passaggi proposti",
  stepNumber: "Passaggio {n}:",
  requiresAdmin: "Richiede amministratore",
  notReversible: "Non reversibile",
  windowsOnly: "Solo su Windows",
  appNote: "Nota dell'app: {note}",
  confirmStepAria: "Conferma del passaggio {n}",
  whatWillHappen: "Cosa succederà:",
  mayAskAdmin: "Windows potrebbe chiederti il permesso di amministratore.",
  "mayAskAdmin@macos": "Questo passaggio richiede permessi da amministratore, che su macOS l'app non chiede.",
  cannotUndo: "Questo passaggio non si può annullare.",
  confirmAndRun: "Conferma ed esegui",
  runningStep: "In esecuzione…",
  stepDone: "Fatto:",
  stepFailed: "Non riuscito:",
  stepNotRun: "Il passaggio non è stato eseguito",
  runStep: "Esegui questo passaggio",
  manualStep: "Da fare a mano:",
  manualDone: "Fatto",
  aiDisclaimer: "I suggerimenti sono generati da un'AI: verifica prima di procedere.",
  answeredBy: "Risposta di {model}",

  // Progress (progress.ts)
  progressAria: "Avanzamento",
  stage_check: "Verifica",
  stage_preview: "Anteprima",
  stage_request: "Proposta",
  stage_steps: "Passaggi",
  stageState_done: "completata",
  stageState_current: "in corso",
  stageState_blocked: "serve un'azione",
  stageState_failed: "non riuscita",
  stageState_upcoming: "da fare",
  stageOf: "Fase {n} di {total}: {stage}, {state}",
  stepsProgress_one: "{done} di {count} passaggio completato",
  stepsProgress_other: "{done} di {count} passaggi completati",
  stepsFailed_one: "{count} non riuscito",
  stepsFailed_other: "{count} non riusciti",
} satisfies Record<string, string>;
