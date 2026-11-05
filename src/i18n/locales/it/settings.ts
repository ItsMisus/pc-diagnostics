/** Italian (reference) texts for the "settings" namespace. */
export default {
  language: "Lingua",
  languageHint: "«Sistema» segue la lingua di Windows. Report e diagnosi già creati restano nella lingua in cui sono stati generati.",
  languageSystem: "Sistema",

  // Page
  pageDescription: "Preferenze salvate localmente in SQLite",
  seconds: "{n} s",
  saveFailed: "Impostazione non salvata",
  saving: "Salvataggio…",
  saved: "Modifiche salvate automaticamente.",

  // Appearance
  appearance: "Aspetto",
  theme: "Tema",
  theme_system: "Sistema",
  theme_light: "Chiaro",
  theme_dark: "Scuro",
  viewMode: "Visualizzazione di problemi ed errori",
  viewModeSimpleHint:
    "{name}: spiegazioni in parole semplici e cosa puoi fare; i dettagli tecnici restano consultabili su richiesta.",
  viewModeAdvancedHint: "{name}: tutti i dettagli tecnici (evidenze, fonti, codici, regole, messaggi di errore completi).",

  // Monitoring
  monitoring: "Monitoraggio",
  samplingInterval: "Intervallo di campionamento",
  samplingIntervalHint: "Metriche leggere (CPU, RAM, GPU, disco). Le sorgenti lente usano una cadenza propria.",
  topN: "Processi in classifica",
  topNHint: "Primi N per Impact Score. Usato dalla pagina Processes.",

  // Privacy
  privacy: "Privacy e rete",
  onlineChecks: "Consenti verifiche online",
  onlineChecksHint:
    "Disattivato di default. Necessario per cercare aggiornamenti driver tramite Windows Update (pagina Drivers). Nessun driver viene installato. Nessuna telemetria viene mai inviata.",

  // App overhead card
  overhead: "Consumo dell'applicazione",
  overheadCaption: "Risorse usate dall'applicazione",
  colComponent: "Componente",
  colCpu: "CPU",
  colMemory: "Memoria (working set)",
  colProcesses: "Processi",
  rowCore: "Core (Rust)",
  rowWebview: "WebView2 (interfaccia)",
  overheadNote: "CPU come quota dell'intera macchina, media sugli ultimi {window}. Aggiornato ogni {interval} s.",
  historyCapacity: "Capacità dello storico",
  historyCapacityValue: "{count} campioni (circa {duration})",
  samplesCollected: "Campioni raccolti",

  // About card
  about: "Informazioni",
  version: "Versione",
  build: "Build",
  dbSchema: "Schema database",
  data: "Dati",
  dataValue: "Solo locali. Nessuna telemetria.",

  // "Risolvi" assistant card
  assistantTitle: "Assistente Risolvi (AI)",
  assistantIntro:
    "Quando premi «Risolvi» su un problema, l'app può chiedere a Claude, l'intelligenza artificiale di Anthropic, come sistemarlo. Serve la tua chiave personale di Anthropic: {cost}.",
  assistantIntroCost: "il costo di ogni richiesta viene addebitato sul tuo account Anthropic",
  assistantPointConsent: "I dati partono solo quando premi «Risolvi» e, dopo averli visti, accetti l'invio.",
  assistantPointData:
    "Viene inviato solo il problema scelto con i dati tecnici che lo riguardano (per esempio uso di memoria o disco, nomi dei programmi coinvolti). Nome del PC, nome utente e percorsi dei file vengono rimossi.",
  assistantPointConfirm:
    "Niente viene eseguito senza la tua conferma, un passaggio alla volta, e solo tra poche operazioni sicure previste dall'app.",
  assistantPointKey: "La chiave è conservata in Gestione credenziali di Windows e non viene più mostrata.",
  assistantEnable: "Attiva l'assistente Risolvi",
  offByDefault: "Disattivato di default.",
  apiKey: "Chiave API di Anthropic",
  keyChecking: "Verifica…",
  keyStatusUnavailable: "Stato non disponibile",
  keySaved: "Chiave salvata",
  keySavedDetail: "Conservata in Gestione credenziali di Windows.",
  keyRemoved: "Chiave rimossa",
  noKey: "Nessuna chiave",
  keyReplacePlaceholder: "Incolla una nuova chiave per sostituirla",
  saveKey: "Salva chiave",
  removeKey: "Rimuovi chiave",
  keyHint: "La crei su console.anthropic.com. Dopo il salvataggio il campo si svuota.",
  keyNotSaved: "Chiave non salvata",
  assistantStatusUnavailable: "Stato dell'assistente non disponibile",
  model: "Modello",
  modelHint: "Costi indicativi a richiesta, a listino Anthropic.",
} satisfies Record<string, string>;
