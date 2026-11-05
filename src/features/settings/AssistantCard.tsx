import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Spinner, StateView } from "@/components/common/StateView";
import { toast } from "@/components/common/Toast";
import { fixFlowStore } from "@/features/assistant/fixFlow";
import { modelHint, modelOptionLabel } from "@/features/assistant/model";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { AssistantStatus } from "@/types/generated/AssistantStatus";
import { ROW_FRAME, ROW_LABEL, Row, Toggle } from "./controls";

type StatusState = { status: "loading" } | { status: "ready"; data: AssistantStatus } | { status: "error"; error: AppError };

interface AssistantCardProps {
  /** From `AppSettings` (saved through `update_settings`). */
  enabled: boolean;
  model: string;
  onEnabledChange: (enabled: boolean) => void;
  onModelChange: (model: string) => void;
}

/**
 * Native radio restyled locally (the global form styles cover checkbox, not radio): accent
 * fill with a dot that scales in, same timing as the checkbox.
 */
const RADIO =
  "mt-0.5 grid size-4 shrink-0 appearance-none place-content-center rounded-full border border-line-strong bg-surface transition-colors duration-150 before:size-1.5 before:scale-0 before:rounded-full before:bg-accent-fg before:transition-transform before:duration-150 checked:border-accent checked:bg-accent checked:before:scale-100 hover:not-checked:border-fg-subtle";

const HINT = "text-xs leading-[18px] text-fg-muted";

/**
 * "Assistente Risolvi (AI)" settings. The API key is write-only: it lives only in the
 * password field until "Salva chiave", is cleared right away and never stored or shown
 * again; the backend keeps it in Windows Credential Manager.
 */
export function AssistantCard({ enabled, model, onEnabledChange, onModelChange }: AssistantCardProps) {
  const [state, setState] = useState<StatusState>({ status: "loading" });
  const [nonce, setNonce] = useState(0);
  const [keyInput, setKeyInput] = useState("");
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyError, setKeyError] = useState<AppError | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const keyId = useId();
  const keyHintId = useId();
  const modelGroupId = useId();
  const t = useT("settings");

  useEffect(() => {
    let active = true;
    setState({ status: "loading" });
    commands.getAssistantStatus().then(
      (data) => active && setState({ status: "ready", data }),
      (e: unknown) => active && setState({ status: "error", error: toAppError(e) }),
    );
    return () => {
      active = false;
    };
  }, [nonce]);

  // Arrived here from "Apri Settings" in the Risolvi dialog: bring this card into view.
  useEffect(() => {
    if (fixFlowStore.getState().consumeSettingsFocus()) {
      rootRef.current?.scrollIntoView({ block: "start" });
      rootRef.current?.focus({ preventScroll: true });
    }
  }, []);

  const saveKey = async (e: FormEvent) => {
    e.preventDefault();
    const key = keyInput.trim();
    if (!key || keyBusy) return;
    // Clear the field before the call: the key is not kept anywhere in the UI, even on error.
    setKeyInput("");
    setKeyBusy(true);
    setKeyError(null);
    try {
      const data = await commands.setAssistantApiKey(key);
      setState({ status: "ready", data });
      if (data.keyConfigured) toast.success(t("keySaved"), t("keySavedDetail"));
    } catch (err) {
      setKeyError(toAppError(err));
    } finally {
      setKeyBusy(false);
    }
  };

  const clearKey = async () => {
    if (keyBusy) return;
    setKeyBusy(true);
    setKeyError(null);
    try {
      const data = await commands.clearAssistantApiKey();
      setState({ status: "ready", data });
      if (!data.keyConfigured) toast.success(t("keyRemoved"));
    } catch (err) {
      setKeyError(toAppError(err));
    } finally {
      setKeyBusy(false);
    }
  };

  const status = state.status === "ready" ? state.data : null;
  const models = status && status.availableModels.length > 0 ? status.availableModels : [model];
  const keyConfigured = status?.keyConfigured ?? false;

  return (
    <div ref={rootRef} tabIndex={-1} aria-label={t("assistantTitle")} className="rounded-xl outline-none">
      <Card title={t("assistantTitle")}>
        <div className="max-w-prose text-sm">
          <p>
            {t.rich("assistantIntro", { cost: <strong className="font-medium">{t("assistantIntroCost")}</strong> })}
          </p>
          <ul className="mt-2.5 flex list-disc flex-col gap-1 pl-4 text-[13px] text-fg-muted marker:text-fg-subtle">
            <li>{t("assistantPointConsent")}</li>
            <li>{t("assistantPointData")}</li>
            <li>{t("assistantPointConfirm")}</li>
            <li>{t("assistantPointKey")}</li>
          </ul>
        </div>

        <div className="mt-4 border-t border-line pt-3.5">
          <Row label={t("assistantEnable")} hint={t("offByDefault")}>
            <Toggle label={t("assistantEnable")} checked={enabled} onChange={onEnabledChange} />
          </Row>

          <div className={ROW_FRAME}>
            <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1">
              <label htmlFor={keyId} className={ROW_LABEL}>
                {t("apiKey")}
              </label>
              <span className="inline-flex min-h-5 items-center gap-1.5 text-xs text-fg-muted" aria-live="polite">
                {state.status === "loading" ? (
                  <>
                    <Spinner size={12} />
                    {t("keyChecking")}
                  </>
                ) : state.status === "error" ? (
                  t("keyStatusUnavailable")
                ) : keyConfigured ? (
                  <>
                    <Icon name="check" size={14} strokeWidth={2} className="text-ok" />
                    {t("keySaved")}
                  </>
                ) : (
                  t("noKey")
                )}
              </span>
            </div>
            <form onSubmit={(e) => void saveKey(e)} className="mt-2.5 flex flex-wrap items-center gap-2">
              <input
                id={keyId}
                type="password"
                value={keyInput}
                onChange={(e) => setKeyInput(e.target.value)}
                autoComplete="off"
                spellCheck={false}
                placeholder={keyConfigured ? t("keyReplacePlaceholder") : "sk-ant-…"}
                aria-describedby={keyHintId}
                className="h-8 min-w-[min(100%,14rem)] flex-1 px-2.5 font-mono placeholder:font-sans"
              />
              <Button type="submit" variant="primary" disabled={keyBusy || keyInput.trim() === ""} aria-busy={keyBusy}>
                <Icon name="key" />
                {t("saveKey")}
              </Button>
              {keyConfigured && (
                <Button onClick={() => void clearKey()} disabled={keyBusy}>
                  {t("removeKey")}
                </Button>
              )}
            </form>
            <p id={keyHintId} className={`mt-1.5 ${HINT}`}>
              {t("keyHint")}
            </p>
            {keyError && (
              <div className="mt-2">
                <StateView compact kind={stateKindForError(keyError)} title={t("keyNotSaved")} error={keyError} />
              </div>
            )}
            {state.status === "error" && (
              <div className="mt-2">
                <StateView
                  compact
                  kind={stateKindForError(state.error)}
                  title={t("assistantStatusUnavailable")}
                  error={state.error}
                  onRetry={() => setNonce((n) => n + 1)}
                />
              </div>
            )}
          </div>

          <fieldset className={`min-w-0 ${ROW_FRAME}`} aria-describedby={modelGroupId}>
            <legend className={`float-left ${ROW_LABEL}`}>{t("model")}</legend>
            <p id={modelGroupId} className={`clear-left mt-0.5 ${HINT}`}>
              {t("modelHint")}
            </p>
            <div className="mt-2.5 -mx-2.5 flex flex-col gap-1">
              {models.map((m) => (
                <label
                  key={m}
                  className="flex cursor-pointer items-start gap-2.5 rounded-lg border border-transparent px-2.5 py-2 transition-colors duration-150 hover:bg-surface-2 has-checked:border-line has-checked:bg-surface-2/60"
                >
                  <input
                    type="radio"
                    name={`${modelGroupId}-model`}
                    value={m}
                    checked={m === model}
                    onChange={() => onModelChange(m)}
                    className={RADIO}
                  />
                  <span className="min-w-0">
                    <span className="block text-[13px] leading-5 text-fg">{modelOptionLabel(m)}</span>
                    {modelHint(m) && <span className={`block ${HINT}`}>{modelHint(m)}</span>}
                  </span>
                </label>
              ))}
            </div>
          </fieldset>
        </div>
      </Card>
    </div>
  );
}
