import { useEffect, useState } from "react";
import { useAppStore } from "@/app/store";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Spinner, StateView } from "@/components/common/StateView";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatDateTime, formatElapsed } from "@/lib/format";
import type { AppError } from "@/types/generated/AppError";
import type { UpdateCheckInfo } from "@/types/generated/UpdateCheckInfo";
import { useDrivers } from "./driverStore";
import { updateSourceLabel } from "./model";

/** Online update search. Gated by the explicit "Consenti verifiche online" setting. */
export function UpdateCheckPanel() {
  const t = useT("drivers");
  const check = useDrivers((s) => s.check);
  const run = useDrivers((s) => s.checkUpdates);
  const refreshing = useDrivers((s) => s.refreshing);
  const info = useDrivers((s) => s.data?.updateCheck ?? null);
  const allowed = useAppStore((s) => s.settings.status === "ready" && s.settings.value.allowOnlineChecks);
  const running = check.status === "running";

  return (
    <Card title={t("updatesTitle")}>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <p id="update-check-desc" className="max-w-prose text-sm text-fg-muted">
          {t("updatesDescription")}
        </p>
        <Button
          variant="primary"
          onClick={run}
          disabled={!allowed || running || refreshing}
          aria-busy={running}
          aria-describedby={allowed ? "update-check-desc" : "update-check-consent"}
        >
          <Icon name="refresh" />
          {running ? t("checking") : t("checkButton")}
        </Button>
      </div>

      {!allowed && <ConsentNotice />}
      {check.status === "running" && <Running startedAt={check.startedAt} />}
      {check.status === "error" && (allowed || check.error.code !== "consentRequired") && (
        <CheckError error={check.error} onRetry={run} />
      )}
      {info && <LastCheck info={info} />}
    </Card>
  );
}

function ConsentNotice() {
  const t = useT("drivers");
  const settings = useAppStore((s) => s.settings);
  const updateSettings = useAppStore((s) => s.updateSettings);

  if (settings.status === "loading") {
    return <StateView compact kind="loading" title={t("settingsLoading")} />;
  }
  if (settings.status === "error") {
    return (
      <StateView
        compact
        kind="error"
        title={t("settingsError")}
        error={settings.error}
      />
    );
  }
  return (
    <div
      id="update-check-consent"
      className="mt-4 flex flex-col gap-2 rounded-lg border border-line bg-surface-2/60 p-3 text-sm"
    >
      <div className="flex items-center gap-2">
        <Icon name="lock" className="shrink-0 text-fg-subtle" />
        <span className="font-medium text-fg">{t("onlineOff")}</span>
      </div>
      <p className="pl-6 text-fg-muted">
        {t("consentCopy")} {t("consentRevoke")}
      </p>
      <div className="pt-1 pl-6">
        <Button onClick={() => void updateSettings({ allowOnlineChecks: true })} disabled={settings.saving}>
          {settings.saving ? t("saving") : t("allowOnline")}
        </Button>
      </div>
      {settings.saveError && (
        <StateView compact kind="error" title={t("settingNotSaved")} error={settings.saveError} />
      )}
    </div>
  );
}

/** Elapsed time only: the search exposes no progress, so no percentage is invented. */
function Running({ startedAt }: { startedAt: number }) {
  const t = useT("drivers");
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  return (
    <div role="status" className="mt-4 flex items-start gap-2 text-sm text-fg-muted">
      <Spinner className="mt-0.5" />
      <p>
        {t.rich("runningText", {
          elapsed: (
            <span aria-live="off" className="tabular">
              {" "}
              {t("elapsed", { time: formatElapsed(now - startedAt) })}
            </span>
          ),
        })}
      </p>
    </div>
  );
}

function CheckError({ error, onRetry }: { error: AppError; onRetry: () => void }) {
  const t = useT("drivers");
  if (error.code === "consentRequired") {
    return (
      <StateView
        compact
        kind="consent"
        title={t("consentDeniedTitle")}
        description={t("consentCopy")}
        error={error}
        onRetry={onRetry}
      />
    );
  }
  if (error.code === "busy") {
    return (
      <StateView
        compact
        kind="error"
        title={t("busyTitle")}
        description={t("busyDescription")}
        error={{ ...error, retriable: true }}
        onRetry={onRetry}
      />
    );
  }
  return (
    <StateView
      compact
      kind={stateKindForError(error)}
      title={t("checkFailedTitle")}
      error={error}
      onRetry={onRetry}
    />
  );
}

function LastCheck({ info }: { info: UpdateCheckInfo }) {
  const t = useT("drivers");
  return (
    <div className="mt-4 border-t border-line pt-3 text-xs text-fg-muted tabular">
      {t("lastCheck", {
        date: formatDateTime(info.checkedAt),
        duration: formatElapsed(info.durationMs),
        source: updateSourceLabel(info.source),
        offered: t.plural("offered", info.offered),
      })}
      {info.error && (
        <div className="mt-2">
          <StateView compact kind="error" title={t("lastCheckIncomplete")} error={info.error} />
        </div>
      )}
    </div>
  );
}
