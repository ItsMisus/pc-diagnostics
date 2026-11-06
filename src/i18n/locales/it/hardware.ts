/** Italian (reference) texts for the "hardware" namespace (inventory and specs, no live values). */
export default {
  inventoryAt: "Inventario rilevato alle {time}",
  refreshing: "Aggiornamento…",
  refresh: "Aggiorna inventario",
  refreshFailed: "Aggiornamento dell'inventario non riuscito: sono mostrati i dati precedenti",

  // Shared labels.
  model: "Modello",
  manufacturer: "Produttore",
  capacity: "Capacità",

  // System.
  systemTitle: "Sistema",
  notReported: "Non riportato dal sistema",
  os: "Sistema operativo",
  version: "Versione",
  kernel: "Kernel",
  hostname: "Nome computer",
  appBuild: "Build applicazione",
  uptime: "Tempo di attività",

  // CPU.
  architecture: "Architettura",
  physicalCores: "Core fisici",
  logicalThreads: "Thread logici",
  maxNominalFrequency: "Frequenza massima nominale",

  // GPU.
  noGpuAdapters: "Nessun adattatore grafico rilevato",
  softwareAdapter: "Adattatore software",
  virtualDisplay: "Display virtuale (nessuna telemetria)",
  dedicatedVram: "VRAM dedicata",
  sharedMemory: "Memoria condivisa",
  driverVersion: "Versione driver",
  telemetry: "Telemetria",
  noProvider: "Nessun provider",

  // Memory.
  memoryTitle: "Memoria",
  totalInstalled: "Totale installato",
  modules: "Moduli",
  noModules: "Nessun modulo riportato dal sistema",
  modulesCaption: "Moduli di memoria installati",
  slot: "Slot",
  ratedSpeed: "Velocità nominale",
  configuredSpeed: "Velocità configurata",
  partNumber: "Part number",

  // Storage.
  storageTitle: "Archiviazione",
  physicalDisks: "Dischi fisici",
  volumes: "Volumi",
  noDisks: "Nessun disco fisico riportato dal sistema",
  bus: "Bus",
  media: "Supporto",
  firmware: "Firmware",
  drive: "Unità",
  label: "Etichetta",
  fileSystem: "File system",
  removable: "Rimovibile",
  yes: "Sì",
  no: "No",
} satisfies Record<string, string>;
