import { useMemo } from "react";
import { useAppStore } from "@/app/store";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { Segmented } from "@/components/common/Segmented";
import { Spinner, StateView } from "@/components/common/StateView";
import { PageHeader } from "@/components/layout/PageHeader";
import { useSetViewMode, useViewMode, useViewModeOptions } from "@/hooks/useViewMode";
import type { AppError } from "@/types/generated/AppError";
import type { ThemePreference } from "@/types/generated/ThemePreference";
import { AboutCard } from "./AboutCard";
import { AppOverheadCard } from "./AppOverheadCard";
import { AssistantCard } from "./AssistantCard";
import { Row, Toggle } from "./controls";
import { localeTag, useLang, useT, type Translator } from "@/i18n";
import { hasWindowsFeatures, usePlatform } from "@/lib/platform";
import type { LanguagePreference } from "@/types/generated/LanguagePreference";

const THEMES: readonly ThemePreference[] = ["system", "light", "dark"];
const INTERVALS_MS = [500, 1000, 2000, 5000];
const TOP_N = [5, 10, 20, 50].map((n) => ({ value: n, label: String(n) }));

/** Translated option tables, rebuilt when the language changes. */
function useOptions(t: Translator<"settings">) {
  const lang = useLang();
  return useMemo(() => {
    const seconds = new Intl.NumberFormat(localeTag(lang), { maximumFractionDigits: 1 });
    return {
      themes: THEMES.map((value) => ({ value, label: t(`theme_${value}`) })),
      intervals: INTERVALS_MS.map((ms) => ({ value: ms, label: t("seconds", { n: seconds.format(ms / 1000) }) })),
    };
  }, [t, lang]);
}

export function SettingsPage() {
  const settings = useAppStore((s) => s.settings);
  const load = useAppStore((s) => s.loadSettings);
  const update = useAppStore((s) => s.updateSettings);
  const viewMode = useViewMode();
  const viewModeOptions = useViewModeOptions();
  const setViewMode = useSetViewMode();
  const t = useT("settings");
  const tc = useT("common");
  const { themes, intervals } = useOptions(t);
  // "Consenti verifiche online" gates the Windows Update driver search, the only online
  // check: elsewhere the toggle would do nothing, so the card says why it is absent.
  const onlineChecks = hasWindowsFeatures(usePlatform());
  const languageOptions: { value: LanguagePreference; label: string }[] = [
    { value: "system", label: t("languageSystem") },
    // Language names are always written in their own language.
    { value: "it", label: "Italiano" },
    { value: "en", label: "English" },
  ];

  return (
    <>
      <PageHeader title="Settings" />
      {settings.status === "loading" && (
        <Card>
          <StateView kind="loading" />
        </Card>
      )}
      {settings.status === "error" && (
        <Card>
          <StateView
            kind={settings.error.code === "notSupported" ? "unsupported" : "error"}
            error={settings.error}
            onRetry={() => void load()}
          />
        </Card>
      )}
      {settings.status === "ready" && (
        <div className="flex flex-col gap-4">
          <Card title={t("appearance")}>
            <Row label={t("language")} hint={t("languageHint")}>
              <Segmented
                label={t("language")}
                options={languageOptions}
                value={settings.value.language}
                onChange={(language) => void update({ language })}
              />
            </Row>
            <Row label={t("theme")}>
              <Segmented
                label={t("theme")}
                options={themes}
                value={settings.value.theme}
                onChange={(theme) => void update({ theme })}
              />
            </Row>
            <Row
              label={t("viewMode")}
              hint={t.rich("viewModeHint", {
                simple: <span className="font-medium text-fg">{tc("viewModeSimple")}</span>,
                advanced: <span className="font-medium text-fg">{tc("viewModeAdvanced")}</span>,
              })}
            >
              <Segmented
                label={t("viewMode")}
                options={viewModeOptions}
                value={viewMode}
                onChange={setViewMode}
              />
            </Row>
          </Card>

          <Card title={t("monitoring")}>
            <Row label={t("samplingInterval")} hint={t("samplingIntervalHint")}>
              <Segmented
                label={t("samplingInterval")}
                options={intervals}
                value={settings.value.samplingIntervalMs}
                onChange={(samplingIntervalMs) => void update({ samplingIntervalMs })}
              />
            </Row>
            <Row label={t("topN")} hint={t("topNHint")}>
              <Segmented
                label={t("topN")}
                options={TOP_N}
                value={settings.value.processTopN}
                onChange={(processTopN) => void update({ processTopN })}
              />
            </Row>
          </Card>

          <AppOverheadCard />

          <Card title={t("privacy")}>
            {onlineChecks ? (
              <Row label={t("onlineChecks")} hint={t("onlineChecksHint")}>
                <Toggle
                  label={t("onlineChecks")}
                  checked={settings.value.allowOnlineChecks}
                  onChange={(allowOnlineChecks) => void update({ allowOnlineChecks })}
                />
              </Row>
            ) : (
              <p className="max-w-lg text-xs leading-[18px] text-fg-muted">{t("onlineChecksWindowsOnly")}</p>
            )}
          </Card>

          <AssistantCard
            enabled={settings.value.assistantEnabled}
            model={settings.value.assistantModel}
            onEnabledChange={(assistantEnabled) => void update({ assistantEnabled })}
            onModelChange={(assistantModel) => void update({ assistantModel })}
          />

          <SaveStatus saving={settings.saving} error={settings.saveError} />
          <AboutCard />
        </div>
      )}
    </>
  );
}

/** Quiet status line: a spinner while a change is being written, otherwise a reminder that saving is automatic. */
function SaveStatus({ saving, error }: { saving: boolean; error: AppError | null }) {
  const t = useT("settings");
  if (error) return <StateView compact kind="error" title={t("saveFailed")} error={error} />;
  return (
    <p className="flex min-h-5 items-center gap-1.5 px-1 text-xs text-fg-subtle" aria-live="polite">
      {saving ? <Spinner size={12} /> : <Icon name="check" size={12} />}
      {saving ? t("saving") : t("saved")}
    </p>
  );
}
