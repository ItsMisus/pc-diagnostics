import { Card } from "@/components/common/Card";
import { DefinitionList } from "@/components/common/DefinitionList";
import { StateView } from "@/components/common/StateView";
import { useAsync } from "@/hooks/useAsync";
import { useT } from "@/i18n";
import { commands } from "@/services/commands";

export function AboutCard() {
  const [state, reload] = useAsync(commands.getAppInfo);
  const t = useT("settings");
  return (
    <Card title={t("about")}>
      {state.status === "loading" && <StateView compact kind="loading" />}
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
    </Card>
  );
}
