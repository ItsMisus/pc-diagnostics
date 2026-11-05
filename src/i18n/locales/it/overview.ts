/** Italian (reference) texts for the "overview" namespace. */
export default {
  pageDescription: "Stato sintetico del sistema",
  monitoringCard: "Monitoraggio",
  recentTrend: "Andamento recente",
  systemCard: "Sistema",

  // Live cards.
  cpuCaption: "Utilizzo totale",
  gpuCaption: "Utilizzo (motore più attivo)",
  onlySoftwareAdapters: "Sono presenti solo adattatori software.",
  ramCaption: "in uso",
  ramCaptionOf: "in uso di {total}",
  detectedAt: "rilevato alle {time}",
  ssdCaption: "liberi su {mount} di {total}",
  noPhysicalDisks: "Nessun disco fisico riportato",

  // System summary.
  notReported: "Non riportato dal sistema",
  os: "Sistema operativo",
  version: "Versione",
  kernel: "Kernel",
  hostname: "Nome computer",
  cpuArch: "Architettura CPU",
  appBuild: "Build applicazione",
  uptime: "Tempo di attività",
} satisfies Record<string, string>;
