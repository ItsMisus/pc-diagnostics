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

  // Page groups.
  componentsTitle: "Componenti",

  // Device identity (form factor and its evidence).
  deviceGroup: "Dispositivo",
  softwareGroup: "Software",
  formFactor_desktop: "PC fisso",
  formFactor_laptop: "Portatile",
  formFactor_mini: "Mini PC",
  formFactor_allInOne: "All-in-one",
  formFactor_other: "Altro tipo di dispositivo",
  basis_productName: "Rilevato dal nome del prodotto",
  basis_modelIdentifier: "Rilevato dall'identificativo del modello",
  basis_chassisType: "Rilevato dal tipo di chassis",
  deviceUnknown: "Tipo di dispositivo non determinato",
  modelIdentifier: "Identificativo modello",
  chassisTypes: "Tipo di chassis (SMBIOS)",
  internalBattery: "Batteria interna",

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
  unifiedMemory: "Nessuna: memoria unificata",
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
  moduleDetails: "Dettaglio moduli",
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
  "drive@macos": "Punto di montaggio",
  label: "Etichetta",
  fileSystem: "File system",
  removable: "Rimovibile",
  yes: "Sì",
  no: "No",
} satisfies Record<string, string>;
