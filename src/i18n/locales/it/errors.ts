/** Italian (reference) texts for the "errors" namespace: src/lib (error titles, reasons, formats). */
export default {
  // lib/format.ts: technical reason a value is missing ("Avanzata").
  reason_notSupported: "Non esposto da hardware o sistema operativo",
  reason_providerMissing: "Provider non disponibile: {detail}",
  reason_permissionDenied: "Richiede privilegi elevati",
  reason_notImplemented: "Provider non ancora implementato",
  reason_failed: "Lettura non riuscita: {message}",

  // lib/plain.ts: the same reasons in everyday words ("Semplice").
  simpleReason_notSupported: "Questo PC non fornisce questo dato",
  "simpleReason_notSupported@macos": "Questo Mac non fornisce questo dato",
  simpleReason_providerMissing: "Manca un componente necessario per leggerlo",
  simpleReason_permissionDenied: "Serve avviare l'app come amministratore",
  "simpleReason_permissionDenied@macos": "macOS non concede all'app l'accesso a questo dato",
  simpleReason_notImplemented: "Non ancora supportato dall'app",
  simpleReason_failed: "Lettura non riuscita, riprova più tardi",
  reasonTooltip: "{plain}. Dettaglio tecnico: {technical}",

  // lib/format.ts: errorTitle ("Avanzata").
  title_invalidInput: "Valore non valido",
  title_notFound: "Elemento non trovato",
  title_permissionDenied: "Permessi insufficienti",
  title_notSupported: "Funzione non supportata",
  title_platform: "Errore del sistema operativo",
  title_database: "Errore del database locale",
  title_busy: "Operazione già in corso",
  title_cancelled: "Operazione annullata",
  title_consentRequired: "Consenso richiesto",
  title_network: "Errore di rete",
  title_internal: "Errore interno",

  // lib/plain.ts: simpleError ("Semplice"): title, explanation, "Cosa puoi fare".
  simpleTitle_invalidInput: "Valore non accettato",
  simpleExplanation_invalidInput: "L'app ha ricevuto un valore che non può usare.",
  simpleAction_invalidInput: "Controlla la scelta fatta e riprova.",
  simpleTitle_notFound: "Dato non trovato",
  simpleExplanation_notFound: "Quello che cercavi non è disponibile, oppure non esiste ancora.",
  simpleAction_notFound: "Ripeti l'operazione; se serve, aggiorna i dati della pagina.",
  simpleTitle_permissionDenied: "Servono permessi da amministratore",
  "simpleTitle_permissionDenied@macos": "Accesso non consentito da macOS",
  simpleExplanation_permissionDenied: "Windows non permette all'app di leggere questo dato con i permessi attuali.",
  "simpleExplanation_permissionDenied@macos":
    "macOS non permette all'app di leggere questo dato con i permessi attuali.",
  simpleAction_permissionDenied: "Chiudi l'app e riaprila con il tasto destro → «Esegui come amministratore».",
  "simpleAction_permissionDenied@macos":
    "Nessuna azione necessaria: su macOS l'app funziona senza permessi da amministratore e non li chiede, quindi questo dato resta non disponibile.",
  simpleTitle_notSupported: "Non disponibile su questo PC",
  "simpleTitle_notSupported@macos": "Non disponibile su questo Mac",
  simpleExplanation_notSupported: "Questo PC o questa versione di Windows non offre questa funzione. Non è un guasto.",
  "simpleExplanation_notSupported@macos":
    "Questo Mac o questa versione di macOS non offre questa funzione. Non è un guasto.",
  simpleAction_notSupported: "Nessuna azione necessaria.",
  simpleTitle_platform: "Windows non ha risposto come previsto",
  "simpleTitle_platform@macos": "macOS non ha risposto come previsto",
  simpleExplanation_platform: "La richiesta a Windows non è andata a buon fine.",
  "simpleExplanation_platform@macos": "La richiesta a macOS non è andata a buon fine.",
  simpleAction_platform: "Riprova tra qualche istante; se succede ancora, riavvia il PC.",
  "simpleAction_platform@macos": "Riprova tra qualche istante; se succede ancora, riavvia il Mac.",
  simpleTitle_database: "Problema con i dati salvati dall'app",
  simpleExplanation_database: "L'app non è riuscita a leggere o salvare i propri dati.",
  simpleAction_database: "Chiudi e riapri l'app; se succede ancora, controlla che il disco non sia pieno.",
  simpleTitle_busy: "Operazione già in corso",
  simpleExplanation_busy: "L'app sta già eseguendo questa operazione.",
  simpleAction_busy: "Attendi che finisca, poi riprova.",
  simpleTitle_cancelled: "Operazione interrotta",
  simpleExplanation_cancelled: "L'operazione è stata fermata prima della fine.",
  simpleAction_cancelled: "Puoi avviarla di nuovo quando vuoi.",
  simpleTitle_consentRequired: "Serve il tuo consenso",
  simpleExplanation_consentRequired: "Questa funzione usa Internet e per scelta è disattivata.",
  simpleAction_consentRequired: "Se vuoi usarla, attiva «Consenti verifiche online» nelle impostazioni (Settings).",
  "simpleAction_consentRequired@macos": "Se vuoi usarla, attivala nelle impostazioni (Settings).",
  simpleTitle_network: "Connessione non riuscita",
  simpleExplanation_network: "L'app non è riuscita a comunicare con il servizio su Internet.",
  simpleAction_network: "Controlla la connessione a Internet e riprova tra qualche istante.",
  simpleTitle_internal: "Qualcosa non ha funzionato nell'app",
  simpleExplanation_internal: "Si è verificato un errore imprevisto nell'app.",
  simpleAction_internal: "Riprova; se succede ancora, chiudi e riapri l'app.",

  // lib/format.ts: source labels that are words, not product names.
  source_d3dkmt: "driver grafico (WDDM)",

  // lib/format.ts: units and date layout.
  durationDaysHours: "{days} g {hours} h",
  isoDate: "{day}/{month}/{year}",
} satisfies Record<string, string>;
