/** Italian (reference) texts for the "settings" namespace. */
export default {
  language: "Lingua",
  languageHint: "«Sistema» segue la lingua di Windows.",
  "languageHint@macos": "«Sistema» segue la lingua di macOS.",
  languageSystem: "Sistema",

  // Page
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
  viewModeHint: "{simple}: spiegazioni semplici e cosa fare. {advanced}: tutti i dettagli tecnici (fonti, codici, errori completi).",

  // Monitoring
  monitoring: "Monitoraggio",
  samplingInterval: "Intervallo di campionamento",
  samplingIntervalHint: "CPU, RAM, GPU e disco; le sorgenti lente hanno una cadenza propria.",
  topN: "Processi in classifica",
  topNHint: "Primi N per Impact Score. Usato dalla pagina Processes.",

  // Privacy
  privacy: "Privacy e rete",
  onlineChecks: "Consenti verifiche online",
  onlineChecksHint: "Serve a cercare aggiornamenti dei driver con Windows Update. Nessun driver viene installato.",
  onlineChecksWindowsOnly:
    "Su questo sistema l'app non fa verifiche online: l'unica, la ricerca di aggiornamenti dei driver con Windows Update, esiste solo su Windows.",

  // App overhead card
  overhead: "Consumo dell'applicazione",
  overheadCaption: "Risorse usate dall'applicazione",
  colComponent: "Componente",
  colCpu: "CPU",
  colMemory: "Memoria (working set)",
  "colMemory@macos": "Memoria (residente)",
  colProcesses: "Processi",
  rowCore: "Core (Rust)",
  rowWebview: "WebView2 (interfaccia)",
  "rowWebview@macos": "WebKit (interfaccia)",
  overheadCpuNote: "Quota dell'intera macchina, media sugli ultimi {window}",
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
  "assistantPointKey@macos": "La chiave è conservata nel Portachiavi di macOS e non viene più mostrata.",
  assistantEnable: "Attiva l'assistente Risolvi",
  offByDefault: "Disattivato di default.",
  apiKey: "Chiave API di Anthropic",
  keyChecking: "Verifica…",
  keyStatusUnavailable: "Stato non disponibile",
  keySaved: "Chiave salvata",
  keySavedDetail: "Conservata in Gestione credenziali di Windows.",
  "keySavedDetail@macos": "Conservata nel Portachiavi di macOS.",
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

  // Device (3D model on the Overview)
  device: "Dispositivo",
  deviceModel: "Modello del dispositivo",
  deviceModelHint: "Il modello 3D mostrato nella pagina Overview.",
  shape_auto: "Automatico",
  shape_tower: "Torre",
  shape_laptop: "Portatile",
  shape_mini: "Mini",
  shape_allInOne: "All-in-one",
  detecting: "Rilevamento del dispositivo in corso…",
  detectedAs: "Rilevato: {shape} ({basis})",
  detectedAsNoBasis: "Rilevato: {shape}",
  basis_productName: "dal nome prodotto",
  basis_modelIdentifier: "dall'identificativo del modello",
  basis_chassisType: "dal tipo di chassis",
  evidence_productName: "Nome prodotto: {value}",
  evidence_modelIdentifier: "Identificativo del modello: {value}",
  evidence_chassisType: "Tipo di chassis (SMBIOS): {value}",
  noModel_allInOne: "Rilevato: computer all-in-one, per cui non c'è un modello 3D. Scegli quello che preferisci.",
  noModel_other: "Il formato rilevato non ha un modello 3D. Scegli quello che preferisci.",
  notDetected: "Non rilevato: {reason}. Scegli il modello qui.",
  retryDetection: "Riprova",
} satisfies Record<string, string>;
