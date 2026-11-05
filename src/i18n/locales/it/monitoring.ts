/** Italian (reference) texts for the "monitoring" namespace: live data, inventory, shared metric labels. */
export default {
  // Shared metric labels (Overview, Hardware).
  temperature: "Temperatura",
  coreClock: "Frequenza core",
  power: "Potenza",
  fan: "Ventola",
  available: "Disponibile",
  read: "Lettura",
  write: "Scrittura",
  cpuUsage: "Utilizzo CPU",
  gpuUsage: "Utilizzo GPU",
  memoryInUse: "Memoria in uso",
  notAvailableText: "non disponibile",
  usedSpaceOn: "Spazio occupato su {mount}",
  percentUsed: "{percent} occupato",
  windowsHealth: "Stato Windows",
  diskFallback: "Disco {id}",
  noVolumes: "Nessun volume rilevato",
  noHardwareGpu: "Nessuna GPU hardware rilevata",
  noSampleForAdapter: "Nessun campione per questo adattatore",

  // GPU power.
  percentOfMax: "{value} del massimo",

  // Labels for inventory enums.
  vendorOther: "Altro (0x{id})",
  busVirtual: "Virtuale",
  busOther: "Altro",
  mediaUnknown: "Non determinato",
  health_healthy: "Integro",
  health_warning: "Avviso",
  health_unhealthy: "Non integro",
  health_unknown: "Non determinato",

  // Live charts.
  percentChartAria: "{label} nel tempo, scala fissa 0–100%",
  diskThroughputTitle: "Disco: lettura e scrittura",
  throughputAria: "Velocità di lettura e scrittura su disco nel tempo, scala da zero",
  legend: "Legenda",

  // Live connection and inventory states.
  liveUnsupportedDesc:
    "Il monitoraggio in tempo reale richiede l'applicazione desktop. Nessun dato dimostrativo viene mostrato.",
  liveConnecting: "Connessione al monitoraggio…",
  liveUnsupported: "Monitoraggio non supportato",
  livePermission: "Monitoraggio non consentito",
  liveUnavailable: "Monitoraggio non disponibile",
  waitingFirstSample: "In attesa del primo campione…",
  badgeLive: "In tempo reale",
  badgeConnecting: "Connessione…",
  badgeStopped: "Monitoraggio interrotto",
  detectingHardware: "Rilevamento dell'hardware…",
  inventoryUnsupported: "Inventario non supportato",
  inventoryUnsupportedDesc:
    "L'inventario hardware richiede l'applicazione desktop. Nessun dato dimostrativo viene mostrato.",
} satisfies Record<string, string>;
