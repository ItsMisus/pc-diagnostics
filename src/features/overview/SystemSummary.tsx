import type { ReactNode } from "react";
import { DefinitionList } from "@/components/common/DefinitionList";
import { StateView } from "@/components/common/StateView";
import { useAsync } from "@/hooks/useAsync";
import { useT } from "@/i18n";
import { formatDuration } from "@/lib/format";
import { commands } from "@/services/commands";
import { DefinitionSkeleton } from "@/features/monitoring/StatusViews";

const ROWS = 7;

export function SystemSummary() {
  const [state, reload] = useAsync(commands.getSystemInfo);
  const t = useT("overview");
  const tc = useT("common");

  if (state.status === "loading") {
    return (
      <div role="status" aria-busy="true">
        <span className="sr-only">{tc("stateLoading")}</span>
        <DefinitionSkeleton rows={ROWS} />
      </div>
    );
  }
  if (state.status === "error") {
    const kind = state.error.code === "notSupported" ? "unsupported" : "error";
    return <StateView compact kind={kind} error={state.error} onRetry={reload} />;
  }

  const s = state.data;
  const na = <span className="text-fg-subtle">{t("notReported")}</span>;
  const rows: [string, ReactNode][] = [
    [t("os"), s.osLongVersion ?? s.osName ?? na],
    [t("version"), s.osVersion ?? na],
    [t("kernel"), s.kernelVersion ?? na],
    [t("hostname"), s.hostname ?? na],
    [t("cpuArch"), s.cpuArch],
    [t("appBuild"), s.buildArch],
    [t("uptime"), s.uptimeS !== null ? formatDuration(s.uptimeS) : na],
  ];

  return <DefinitionList items={rows.map(([label, value]) => ({ label, value }))} />;
}
