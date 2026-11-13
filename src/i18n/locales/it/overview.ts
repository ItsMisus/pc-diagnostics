/** Italian (reference) texts for the "overview" namespace (the dashboard). */
export default {
  monitoringCard: "Monitoraggio",
  recentTrend: "Andamento recente",

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
} satisfies Record<string, string>;
