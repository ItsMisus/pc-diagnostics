/** Italian (reference) texts for the "processes" namespace. */
export default {
  // ProcessesPage
  connectingTitle: "Connessione all'analisi dei processi…",
  waitingTitle: "In attesa della prima misurazione…",
  waitingDescription:
    "La prima classifica è disponibile pochi secondi dopo l'avvio: l'uso della CPU di un processo si calcola confrontando due campioni successivi.",
  unsupportedTitle: "Analisi dei processi non supportata",
  unsupportedDescription: "L'analisi dei processi richiede l'applicazione desktop. Nessun dato dimostrativo viene mostrato.",
  notAllowedTitle: "Analisi dei processi non consentita",
  unavailableTitle: "Analisi dei processi non disponibile",
  modeInstant: "Istantanea",
  modeWindow: "Intervallo (ultimi {seconds} s)",
  rankingLabel: "Classifica",
  searchLabel: "Cerca processo per nome",
  searchPlaceholder: "Cerca per nome…",
  summaryTotal_one: "{count} processo",
  summaryTotal_other: "{count} processi",
  summaryProtected_one: "{count} protetto",
  summaryProtected_other: "{count} protetti",
  summaryShown: "primi {shown} per Impact Score",

  // ProcessTable
  protectedNote:
    "Processo protetto: il percorso dell'eseguibile non è leggibile senza privilegi elevati. Le metriche provengono dallo snapshot del kernel e sono reali.",
  ioNote: "I/O logico (file, rete, dispositivi): non corrisponde ai byte fisici scritti sul disco",
  colName: "Nome processo",
  colPeak: "Picco",
  selfTag: "questa app",
  emptyRanking: "Nessun processo in classifica in questo campione.",
  noMatch: "Nessun processo corrisponde a «{query}».",
  tableCaption: "Processi ordinati per {column}",
  rowShowDetails: "{name}, PID {pid}: mostra dettagli",
  rowHideDetails: "{name}, PID {pid}: chiudi dettagli",
  protectedShort: "protetto",
  impactOf: "Impact Score di {name}",
  outOf100: "{value} su 100",

  // rows.ts: process categories
  category_system: "Sistema",
  category_service: "Servizio",
  category_application: "Applicazione",
  category_background: "In background",
  category_unknown: "Non classificato",

  // ProcessDetail
  detailTitle: "Dettaglio",
  detailEmpty: "Seleziona un processo nella tabella per vederne l'andamento recente.",
  protectedDetail:
    "Processo protetto: percorso dell'eseguibile non leggibile senza privilegi elevati (metriche reali dal kernel)",
  historyLoading: "Caricamento dello storico…",
  endedTitle: "Processo terminato o non più monitorato",
  endedDescription: "Lo storico di un processo viene conservato solo per poco tempo dopo la sua chiusura.",
  memorySection: "Memoria",
  privateMemory: "Memoria privata",
  exePathSection: "Percorso eseguibile",
  localInfoAria: "Informazioni locali",
  exePathUnavailable: "Non disponibile per questo processo",
  localInfoNote: "Informazione locale, mostrata solo qui: non viene inclusa nei report condivisibili.",
  pressureSection: "Pressione sulle risorse (0–100)",
  pressureAria: "Pressione sulle risorse",
  pressureOf: "Pressione {label}",
  staleNote: "Il processo non è più in classifica: sono mostrati gli ultimi valori noti.",
  historyEnded: "Terminato",
  noSamples: "Nessun campione ancora registrato per questo processo.",
  cpuChartTitle: "CPU (% dell'intero sistema)",
  cpuChartAria: "CPU del processo nel tempo, scala fissa 0–100%",
  ramChartTitle: "RAM (working set)",
  ramChartAria: "Memoria del processo nel tempo, scala da zero",
  ioChartTitle: "I/O logico: scrittura (linea piena) e lettura",
  ioChartAria: "I/O logico del processo nel tempo, scala da zero",
  seriesWrite: "Scrittura",
  seriesRead: "Lettura",

  // ScoreExplanation
  scoreDisclaimer: "L'indice stima la pressione sulle risorse; non dimostra che un processo stia rallentando il computer.",
  scoreTitle: "Come viene calcolato l'Impact Score",
  scoreIntro: "Ogni risorsa è normalizzata su una scala 0–100 e l'indice è la media pesata:",
  scoreCpu:
    "quota della capacità dell'intero sistema ({processors} processori logici); 100 significa tutti i processori occupati.",
  scoreRam: "memoria del processo rapportata alla memoria fisica totale ({total}; disponibile ora {available}).",
  scoreIo:
    "I/O logico (file, rete, dispositivi) rapportato a un riferimento di {reference}, che corrisponde a 100.",
  scoreModes:
    "«Istantanea» usa l'ultimo campione (ogni {interval} s). «Intervallo» usa la media degli ultimi {window} s; il picco è il valore massimo osservato nello stesso intervallo.",
  scoreMethodology:
    "Metodologia v{version}: i punteggi sono confrontabili solo tra misure con la stessa versione. Questa pagina è in sola lettura: non termina, sospende né modifica alcun processo.",
} satisfies Record<string, string>;
