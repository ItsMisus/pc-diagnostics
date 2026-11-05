"""Local stand-in for api.anthropic.com/v1/messages (UI testing of a debug build only).
Logs each request body (minus the key) to mock_requests.jsonl next to this script."""
import json, os, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))

PLAN = {
    "summary": "Questo è un piano di prova generato da un server finto, non da Claude. Serve solo a controllare che le schermate funzionino.",
    "confidence": "medium",
    "steps": [
        {"id": "s1", "title": "Apri Sensore memoria", "explanation": "Apre la pagina delle Impostazioni dove Windows libera spazio da solo. Non cambia nulla da sola.",
         "risk": "low", "requiresAdmin": False, "reversible": True,
         "action": {"kind": "openSettingsPage", "page": "storageSense"}, "manualInstructions": None},
        {"id": "s2", "title": "Svuota il Cestino", "explanation": "Elimina definitivamente i file nel Cestino. Windows chiede conferma.",
         "risk": "medium", "requiresAdmin": False, "reversible": False,
         "action": {"kind": "emptyRecycleBin"}, "manualInstructions": None},
        {"id": "s3", "title": "Chiudi un programma inesistente", "explanation": "Prova di un'azione che l'app deve rifiutare.",
         "risk": "low", "requiresAdmin": False, "reversible": True,
         "action": {"kind": "closeProgram", "pid": 4, "name": "System"}, "manualInstructions": None},
        {"id": "s4", "title": "Controlla la cartella Download", "explanation": "Spesso contiene file grandi che non servono più.",
         "risk": "low", "requiresAdmin": False, "reversible": True,
         "action": None, "manualInstructions": "Apri Esplora file, vai in Download e ordina per dimensione."},
    ],
    "warnings": ["Piano di prova: non è un consiglio reale."],
}


class H(BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get("content-length", "0"))
        body = json.loads(self.rfile.read(n) or b"{}")
        with open(os.path.join(HERE, "mock_requests.jsonl"), "a", encoding="utf-8") as f:
            f.write(json.dumps({"path": self.path, "headers": {k: v for k, v in self.headers.items() if k.lower() != "x-api-key"},
                                "has_key": bool(self.headers.get("x-api-key")), "body": body}, ensure_ascii=False) + "\n")
        out = json.dumps({
            "id": "msg_mock", "type": "message", "role": "assistant", "model": body.get("model", "?"),
            "stop_reason": "end_turn", "stop_details": None,
            "content": [{"type": "text", "text": json.dumps(PLAN, ensure_ascii=False)}],
            "usage": {"input_tokens": 1834, "output_tokens": 612},
        }).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1]) if len(sys.argv) > 1 else 8787), H).serve_forever()
