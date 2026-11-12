import { useId, type ReactNode } from "react";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { Icon } from "@/components/common/Icon";
import { UnavailableReason } from "@/components/common/MetricValue";
import { Panel } from "@/components/common/Panel";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { useAsync } from "@/hooks/useAsync";
import { useIsSimple } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { formatDuration } from "@/lib/format";
import { commands } from "@/services/commands";
import { DefinitionSkeleton } from "@/features/monitoring/StatusViews";
import type { FormFactor } from "@/types/generated/FormFactor";
import type { FormFactorInfo } from "@/types/generated/FormFactorInfo";
import { formFactorIcon } from "./layout";
import { metricItem } from "./metricItem";

const OS_ROWS = 6;

/**
 * The focal panel of the page: what the machine is (form factor with the evidence it was
 * detected from) and which system it runs (one request on mount). Static facts only.
 */
export function SystemSection({ formFactor }: { formFactor: FormFactorInfo }) {
  const t = useT("hardware");
  const titleId = useId();
  return (
    <Panel variant="primary" title={t("systemTitle")} titleId={titleId}>
      <Identity info={formFactor} />
      {/* Its own size container: the two groups go side by side only when the panel has the room. */}
      <div className="@container mt-5">
        <div className="grid grid-cols-1 gap-x-10 gap-y-6 @2xl:grid-cols-2">
          <div className="min-w-0">
            <h3 className="eyebrow mb-2">{t("deviceGroup")}</h3>
            <DeviceRows info={formFactor} />
          </div>
          <div className="min-w-0">
            <h3 className="eyebrow mb-2">{t("softwareGroup")}</h3>
            <SystemRows />
          </div>
        </div>
      </div>
    </Panel>
  );
}

function useFormFactorLabel(): (f: FormFactor) => string {
  const t = useT("hardware");
  return (f) => t(`formFactor_${f}`);
}

/**
 * Headline: the product name when the OS reports one (macOS), otherwise the detected form
 * factor. The line under it says how the form factor was found, or why it is not known.
 */
function Identity({ info }: { info: FormFactorInfo }) {
  const t = useT("hardware");
  const label = useFormFactorLabel();
  const ff = info.formFactor;
  const ffLabel = ff.status === "available" ? label(ff.value) : null;
  const headline = info.productName ?? ffLabel ?? t("deviceUnknown");
  const basis = info.basis ? t(`basis_${info.basis}`) : null;

  let line: ReactNode;
  if (ff.status === "unavailable") {
    line = <UnavailableReason reason={ff.reason} />;
  } else {
    // The form factor is already the headline when there is no product name: say only how.
    const parts = [info.productName ? ffLabel : null, basis].filter((p): p is string => Boolean(p));
    line = parts.length > 0 ? parts.join(" · ") : null;
  }

  return (
    <div className="flex min-w-0 items-center gap-4">
      <span className="satin flex size-12 shrink-0 items-center justify-center rounded-full text-fg-muted" aria-hidden="true">
        <Icon name={formFactorIcon(ff)} size={22} />
      </span>
      <div className="min-w-0">
        <p className="font-display text-2xl font-semibold break-words text-fg">{headline}</p>
        {line && (
          <p className="mt-0.5 text-sm text-fg-muted">
            {line}
            <span className="ml-2 empty:hidden">
              <SourceCaption metric={ff} />
            </span>
          </p>
        )}
      </div>
    </div>
  );
}

/** Spec rows of the device: evidence (model identifier, chassis codes), battery. */
function DeviceRows({ info }: { info: FormFactorInfo }) {
  const t = useT("hardware");
  const simple = useIsSimple();
  const yesNo = (v: boolean) => (v ? t("yes") : t("no"));

  // The type itself is the headline (or its line): one home per value.
  const items: DefinitionItem[] = [];
  if (info.modelIdentifier) {
    items.push({ label: t("modelIdentifier"), value: <span className="font-mono text-xs">{info.modelIdentifier}</span> });
  }
  // SMBIOS codes are technical evidence: Advanced view only (the headline already says "detected from the chassis type").
  if (!simple && info.chassisTypes.length > 0) {
    items.push({ label: t("chassisTypes"), value: <span className="font-mono text-xs">{info.chassisTypes.join(", ")}</span> });
  }
  items.push(metricItem(t("internalBattery"), info.internalBattery, yesNo));

  return <DefinitionList divided items={items} />;
}

function SystemRows() {
  const [state, reload] = useAsync(commands.getSystemInfo);
  const t = useT("hardware");
  const tc = useT("common");

  if (state.status === "loading") {
    return (
      <div role="status" aria-busy="true">
        <span className="sr-only">{tc("stateLoading")}</span>
        <DefinitionSkeleton rows={OS_ROWS} />
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
    [t("appBuild"), s.buildArch],
    [t("uptime"), s.uptimeS !== null ? formatDuration(s.uptimeS) : na],
  ];

  return <DefinitionList divided items={rows.map(([label, value]) => ({ label, value }))} />;
}
