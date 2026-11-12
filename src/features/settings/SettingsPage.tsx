import { useId, useMemo } from "react";
import { useAppStore } from "@/app/store";
import { Icon } from "@/components/common/Icon";
import { Panel } from "@/components/common/Panel";
import { Segmented } from "@/components/common/Segmented";
import { Skeleton } from "@/components/common/Skeleton";
import { Spinner, StateView } from "@/components/common/StateView";
import { PageHeader } from "@/components/layout/PageHeader";
import { useSetViewMode, useViewMode, useViewModeOptions } from "@/hooks/useViewMode";
import type { ThemePreference } from "@/types/generated/ThemePreference";
import { AboutCard } from "./AboutCard";
import { AppOverheadCard } from "./AppOverheadCard";
import { AssistantCard } from "./AssistantCard";
import { DeviceShapeRow } from "./DeviceShapeRow";
import { Row, RowsPanel, SettingsGroup, Toggle } from "./controls";
import { localeTag, useLang, useT, type Translator } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { hasWindowsFeatures, usePlatform } from "@/lib/platform";
import type { LanguagePreference } from "@/types/generated/LanguagePreference";

const THEMES: readonly ThemePreference[] = ["system", "light", "dark"];
const THEME_ICONS = { system: "display", light: "sun", dark: "moon" } as const;
const INTERVALS_MS = [500, 1000, 2000, 5000];
const TOP_N = [5, 10, 20, 50].map((n) => ({ value: n, label: String(n) }));

/** Translated option tables, rebuilt when the language changes. */
function useOptions(t: Translator<"settings">) {
  const lang = useLang();
  return useMemo(() => {
    const seconds = new Intl.NumberFormat(localeTag(lang), { maximumFractionDigits: 1 });
    return {
      themes: THEMES.map((value) => ({ value, label: t(`theme_${value}`), icon: THEME_ICONS[value] })),
      intervals: INTERVALS_MS.map((ms) => ({ value: ms, label: t("seconds", { n: seconds.format(ms / 1000) }) })),
    };
  }, [t, lang]);
}

/**
 * Settings as grouped rows (Aspetto, Dispositivo, Monitoraggio, Privacy e rete, Assistente,
 * Informazioni): each row is label + one short hint + the control on the right, on solid
 * panels. Every change is saved at once; the toolbar says when.
 */
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
  const ids = { language: useId(), interval: useId(), topN: useId(), online: useId(), viewMode: useId() };
  // "Consenti verifiche online" gates the Windows Update driver search, the only online
  // check: elsewhere the toggle would do nothing, so the panel says why it is absent.
  const onlineChecks = hasWindowsFeatures(usePlatform());
  const languageOptions: { value: LanguagePreference; label: string }[] = [
    { value: "system", label: t("languageSystem") },
    // Language names are always written in their own language.
    { value: "it", label: "Italiano" },
    { value: "en", label: "English" },
  ];

  return (
    <>
      <PageHeader
        title="Settings"
        {...(settings.status === "ready" ? { actions: <SaveStatus saving={settings.saving} failed={settings.saveError !== null} /> } : {})}
      />
      {settings.status === "loading" && <SettingsSkeleton />}
      {settings.status === "error" && (
        <Panel>
          <StateView
            layout="hero"
            kind={settings.error.code === "notSupported" ? "unsupported" : stateKindForError(settings.error)}
            error={settings.error}
            onRetry={() => void load()}
          />
        </Panel>
      )}
      {settings.status === "ready" && (
        <div className="flex flex-col gap-8">
          {settings.saveError && <StateView compact kind="error" title={t("saveFailed")} error={settings.saveError} />}

          <SettingsGroup title={t("appearance")}>
            {/* The page's focal panel: the settings people change most. */}
            <RowsPanel variant="primary">
              <Row label={t("theme")}>
                <Segmented
                  label={t("theme")}
                  options={themes}
                  value={settings.value.theme}
                  onChange={(theme) => void update({ theme })}
                />
              </Row>
              <Row label={t("language")} hint={t("languageHint")} hintId={ids.language}>
                <Segmented
                  label={t("language")}
                  options={languageOptions}
                  value={settings.value.language}
                  onChange={(language) => void update({ language })}
                />
              </Row>
              <Row
                label={t("viewMode")}
                hintId={ids.viewMode}
                hint={t.rich("viewModeHint", {
                  simple: <span className="font-medium text-fg">{tc("viewModeSimple")}</span>,
                  advanced: <span className="font-medium text-fg">{tc("viewModeAdvanced")}</span>,
                })}
              >
                <Segmented label={t("viewMode")} options={viewModeOptions} value={viewMode} onChange={setViewMode} />
              </Row>
            </RowsPanel>
          </SettingsGroup>

          <SettingsGroup title={t("device")}>
            <RowsPanel>
              <DeviceShapeRow />
            </RowsPanel>
          </SettingsGroup>

          <SettingsGroup title={t("monitoring")}>
            <RowsPanel>
              <Row label={t("samplingInterval")} hint={t("samplingIntervalHint")} hintId={ids.interval}>
                <Segmented
                  label={t("samplingInterval")}
                  options={intervals}
                  value={settings.value.samplingIntervalMs}
                  onChange={(samplingIntervalMs) => void update({ samplingIntervalMs })}
                />
              </Row>
              <Row label={t("topN")} hint={t("topNHint")} hintId={ids.topN}>
                <Segmented
                  label={t("topN")}
                  options={TOP_N}
                  value={settings.value.processTopN}
                  onChange={(processTopN) => void update({ processTopN })}
                />
              </Row>
            </RowsPanel>
            {/* The app's own footprint and the history size follow from the sampling interval. */}
            <AppOverheadCard />
          </SettingsGroup>

          <SettingsGroup title={t("privacy")}>
            <RowsPanel>
              {onlineChecks ? (
                <Row label={t("onlineChecks")} hint={t("onlineChecksHint")} hintId={ids.online}>
                  <Toggle
                    label={t("onlineChecks")}
                    checked={settings.value.allowOnlineChecks}
                    onChange={(allowOnlineChecks) => void update({ allowOnlineChecks })}
                    describedBy={ids.online}
                  />
                </Row>
              ) : (
                <p className="flex max-w-prose items-start gap-2 text-xs leading-[18px] text-fg-muted">
                  <Icon name="info" size={14} className="mt-0.5 shrink-0 text-fg-subtle" />
                  {t("onlineChecksWindowsOnly")}
                </p>
              )}
            </RowsPanel>
          </SettingsGroup>

          <AssistantCard
            enabled={settings.value.assistantEnabled}
            model={settings.value.assistantModel}
            onEnabledChange={(assistantEnabled) => void update({ assistantEnabled })}
            onModelChange={(assistantModel) => void update({ assistantModel })}
          />

          <SettingsGroup title={t("about")}>
            <AboutCard />
          </SettingsGroup>
        </div>
      )}
    </>
  );
}

/**
 * Quiet toolbar status: a spinner while a change is being written, "not saved" (glyph +
 * words; the details are inline at the top of the page) after a failed save, otherwise a
 * reminder that saving is automatic. The toolbar is sticky, so a failure is seen from any row.
 */
function SaveStatus({ saving, failed }: { saving: boolean; failed: boolean }) {
  const t = useT("settings");
  return (
    <p
      className={`flex min-h-5 items-center gap-1.5 text-xs ${failed && !saving ? "text-bad" : "text-fg-subtle"}`}
      aria-live="polite"
    >
      {saving ? (
        <Spinner size={12} />
      ) : failed ? (
        <Icon name="error" size={12} className="shrink-0" />
      ) : (
        <Icon name="check" size={12} className="shrink-0" />
      )}
      {saving ? t("saving") : failed ? t("saveFailed") : t("saved")}
    </p>
  );
}

/** First load: the shape of the grouped rows, nothing more. */
function SettingsSkeleton() {
  const tc = useT("common");
  return (
    <div role="status" aria-busy="true" className="flex flex-col gap-8">
      <span className="sr-only">{tc("stateLoading")}</span>
      {[3, 1, 2].map((rows, g) => (
        <div key={g} className="flex flex-col gap-2.5">
          <Skeleton className="ml-1 h-4 w-28" />
          <div className={g === 0 ? "surface-primary p-5" : "surface-secondary p-4"}>
            {Array.from({ length: rows }, (_, r) => (
              <div key={r} className="flex items-center justify-between gap-6 border-t border-line py-3.5 first:border-t-0 first:pt-0 last:pb-0">
                <span className="flex flex-1 flex-col gap-1.5">
                  <Skeleton className="h-3 w-40" />
                  <Skeleton className="h-2.5 w-64 max-w-full" />
                </span>
                <Skeleton round className="h-7 w-48 shrink-0" />
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
