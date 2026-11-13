import { Panel } from "@/components/common/Panel";
import { SkeletonLines } from "@/components/common/Skeleton";
import { DefinitionList } from "@/components/common/DefinitionList";
import { StateView } from "@/components/common/StateView";
import { useAsync } from "@/hooks/useAsync";
import { useT } from "@/i18n";
import { commands } from "@/services/commands";

export function AboutCard() {
  const [state, reload] = useAsync(commands.getAppInfo);
  const t = useT("settings");
  const tc = useT("common");
  return (
    // The group heading ("Informazioni") names this panel.
    <Panel as="div">
      {state.status === "loading" && (
        <div role="status" aria-busy="true">
          <span className="sr-only">{tc("stateLoading")}</span>
          <SkeletonLines lines={4} />
        </div>
      )}
      {state.status === "error" && <StateView compact kind="error" error={state.error} onRetry={reload} />}
      {state.status === "ready" && (
        // Same label/value list as the app overhead card, so the two cards line up.
        <DefinitionList
          items={[
            { label: t("version"), value: state.data.version },
            { label: t("build"), value: state.data.buildProfile },
            { label: t("dbSchema"), value: `v${state.data.schemaVersion}` },
            { label: t("data"), value: t("dataValue") },
          ]}
        />
      )}
    </Panel>
  );
}
