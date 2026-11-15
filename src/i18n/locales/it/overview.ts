/** Italian (reference) texts for the "overview" namespace (the dashboard). */
export default {
  componentsTitle: "Componenti",
  trendAria: "{label}, ultimi due minuti",
  notOnThisDevice: "Non rilevabili su questo computer: {list}.",
  volumeFree: "{free} liberi su {total}",
  monitoringCard: "Monitoraggio",

  // Panels.
  load: "Carico",
  inUse: "In uso",
  used: "Usata",
  meterAria: "{label} {component}",
  ofTotal: "{used} di {total}",
  clock: "Frequenza",
  frequencyLimited: "Limitata a {limit}",
  frequencyLimitBody:
    "Windows riporta un limite di {limit} rispetto a un massimo nominale di {max}. Il sistema non ne indica la causa (ad esempio piano energetico, temperatura o alimentazione).",
  "frequencyLimitBody@macos":
    "macOS riporta un limite di {limit} rispetto a un massimo nominale di {max}. Il sistema non ne indica la causa (ad esempio temperatura, alimentazione o modalità di risparmio energetico).",
  onlySoftwareAdapters: "Sono presenti solo adattatori software.",
  committedMemory: "Memoria impegnata",
  pagesFromDisk: "Pagine lette da disco",
  memoryPressure: "Pressione memoria",
  pressureNormal: "Normale",
  swapUsed: "Swap in uso",
  diskTitle: "Disco",
  activeTime: "Tempo attivo",
  storageTitle: "Archiviazione",
  detectedAt: "rilevato alle {time}",
  healthNote: "Riepilogo di Windows Storage, non una valutazione completa dei dati SMART.",
  "healthNote@macos": "Dal registro SMART dei dischi NVMe (avvisi critici, riserva, usura), non una valutazione completa.",
  noPhysicalDisks: "Nessun disco fisico riportato",

  // Status of a meter above its threshold (features/overview/levels.ts).
  levelWarn: "Avviso",
  levelBad: "Critico",
  levelAria: "{value}, {level}",

  // Top processes.
  processesTitle: "Processi",
  openProcesses: "Apri Processi",
  topProcessesCaption: "Processi con l'impatto maggiore",
  colName: "Nome",
  ioNote: "I/O logico (file, rete, dispositivi), non byte fisici su disco",
  "ioNote@macos": "I/O su disco del processo (macOS non conta per processo rete e dispositivi)",
  processesWaiting: "In attesa dei dati dei processi…",
  processesUnavailable: "Dati dei processi non disponibili",
  noProcesses: "Nessun processo nella classifica",

  // Per logical processor.
  perCoreTitle: "Processori logici",
  perCoreUsage: "Utilizzo per processore logico",
  logicalProcessor: "Processore logico {n}",

  // Hero: device identity.
  heroLabel: "Il tuo computer",
  thisComputer: "Questo computer",
  identityCpu: "Processore",
  identityGpu: "Grafica",
  identityMemory: "Memoria",
  identitySystem: "Sistema",
  cores: "{count} core",
  coresThreads: "{physical} core, {logical} thread",

  // Hero: overall state, from the latest diagnosis (no score).
  stateEyebrow: "Stato",
  stateClear: "Nessun problema rilevato",
  stateFrom: "Dall'ultima diagnosi, {date}",
  stateNotAScore: "È il risultato dell'ultima diagnosi, non un punteggio di salute.",
  stateNeverRunHint: "Avviala per sapere se c'è qualcosa da sistemare.",
  stateLoading: "Lettura dello stato…",
  stateUnavailable: "Stato non disponibile",
  runBenchmark: "Esegui un benchmark",
  benchmarkRunning: "Benchmark in corso",
  quickActions: "Azioni rapide",

  // Hero: the air around the 3D model.
  airflowTitle_fans: "Flusso d'aria",
  airflowTitle_passive: "Flusso d'aria: raffreddamento passivo",
  airflowTitle_unknown: "Flusso d'aria non mostrato",
  airflowIntake: "Aria fresca in entrata",
  airflowExhaust: "Aria calda in uscita",
  airflowHeat: "Calore che lascia la scocca",
  airflowNote_fans: "Le ventole aspirano aria fresca e spingono fuori quella calda. Il percorso è quello tipico di questo tipo di computer, non una misura.",
  airflowNote_passive: "Non ci sono ventole: nessuna aria entra o esce. Il calore passa alla scocca e sale con l'aria intorno.",
  airflowNote_unknown: "Il sistema non dice come è raffreddato questo computer, quindi il flusso d'aria non viene disegnato.",

  // Hero: device model shown by the 3D stage.
  shapeTitle: "Modello del dispositivo",
  shape_tower: "PC fisso",
  shape_laptop: "Portatile",
  shape_mini: "Mini PC",
  shape_allInOne: "All-in-one",
  shapeAuto: "Automatico",
  shapeDetected_productName: "Rilevato dal nome del prodotto",
  shapeDetected_modelIdentifier: "Rilevato dall'identificativo del modello",
  shapeDetected_chassisType: "Rilevato dal tipo di chassis",
  shapeDetected_unknownBasis: "Rilevato automaticamente",
  shapeOverride: "Scelto da te",
  shapePending: "Rilevamento del modello…",
  shapeUnknown: "Modello non rilevato",
  shapeChange: "Cambia",
  shapeChangeAria: "Cambia il modello del dispositivo",
  shapeReset: "Usa il rilevamento",
  shapeChooseTitle: "Che computer è?",
  shapeChooseHint: "Non è stato possibile capirlo in modo affidabile. Scegli il modello da mostrare: puoi cambiarlo quando vuoi.",
  shapeNoModel_allInOne: "È un computer tutto-in-uno, per cui non c'è un modello dedicato: scegli quello più simile.",
  shapeNoModel_other: "Per questo tipo di computer non c'è un modello dedicato: scegli quello più simile.",
  shapeChooseOption: "Mostra come {shape}",
  shapeSaveFailed: "Scelta non salvata",
  shapeEvidenceProduct: "Prodotto: {value}",
  shapeEvidenceModel: "Identificativo: {value}",
  shapeEvidenceChassis: "Tipo di chassis SMBIOS: {value}",

  // Chips around the device (live values).
  chipCpu: "CPU",
  chipRam: "RAM",
  chipGpu: "GPU",
  chipTemp: "Temp. CPU",

  // Essential metrics.
  sparkAria: "{label}, ultimo minuto",
  cpuLoad: "Carico CPU",
  gpuLoad: "Carico GPU",
  memoryUsed: "Memoria in uso",
  storageFree: "Spazio libero",
  freeOf: "liberi su {total}",
  temperatureValue: "Temperatura {value}",
  noGpu: "Nessuna GPU",

  // Groups.
  findingsTitle: "Segnalazioni",
  findingsNeverRun: "Le segnalazioni compaiono dopo la prima diagnosi.",
} satisfies Record<string, string>;
