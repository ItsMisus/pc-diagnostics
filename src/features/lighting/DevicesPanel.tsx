import { Icon, type IconName } from "@/components/common/Icon";
import { Panel } from "@/components/common/Panel";
import { StateView } from "@/components/common/StateView";
import { Switch } from "@/components/common/Switch";
import { SupportBadge, SupportReason } from "@/components/control/SupportBadge";
import { useT } from "@/i18n";
import type { LightingDevice } from "@/types/generated/LightingDevice";
import type { LightingDeviceKind } from "@/types/generated/LightingDeviceKind";
import type { LightingSetup } from "@/types/generated/LightingSetup";
import type { LightingTargets } from "@/types/generated/LightingTargets";
import { monochromeCompatible } from "./effect";
import { controllable } from "./store";

export const KIND_ICON: Record<LightingDeviceKind, IconName> = {
  keyboard: "keyboard",
  mouse: "mouse",
  headset: "headset",
  strip: "ledStrip",
  fan: "fan",
  case: "desktop",
  keyboardBacklight: "laptop",
  other: "bulb",
};

/** Targets after switching `id` on or off (all devices again = `all`). */
export function toggleTarget(targets: LightingTargets, all: readonly string[], id: string, on: boolean): LightingTargets {
  const current = targets.kind === "all" ? [...all] : targets.ids.filter((x) => all.includes(x));
  const next = on ? [...new Set([...current, id])] : current.filter((x) => x !== id);
  return all.every((x) => next.includes(x)) ? { kind: "all" } : { kind: "devices", ids: next };
}

/**
 * This computer's lighting devices: the controllable ones with include/exclude and the
 * "Sincronizza" switch, then the recognised ones the app cannot drive (with the reason).
 */
export function DevicesPanel({
  devices,
  setup,
  onChange,
  platformNote,
}: {
  devices: readonly LightingDevice[];
  setup: LightingSetup;
  onChange: (patch: Partial<LightingSetup>) => void;
  platformNote: string | null;
}) {
  const t = useT("lighting");
  const usable = devices.filter(controllable);
  const others = devices.filter((d) => !controllable(d));
  const ids = usable.map((d) => d.id);
  const included = (id: string) => setup.targets.kind === "all" || setup.targets.ids.includes(id);

  return (
    <Panel title={t("devicesTitle")} icon="bulb" className="flex flex-col">
      <div className="flex flex-col gap-4">
        {usable.length === 0 ? (
          <StateView kind="unsupported" title={t("devicesNoneTitle")} description={t("devicesNoneHint")} />
        ) : (
          <ul className="flex flex-col divide-y divide-line">
            {usable.map((d) => {
              const on = included(d.id);
              const cannot = !d.colorCapable && !monochromeCompatible(setup.effect);
              return (
                <li key={d.id} className="flex items-center gap-3 py-2.5 first:pt-0">
                  <span aria-hidden="true" className="satin grid size-9 shrink-0 place-items-center rounded-full text-fg-muted">
                    <Icon name={KIND_ICON[d.kind]} size={18} />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm text-fg">{d.name}</span>
                    <span className="block truncate text-xs text-fg-subtle">
                      {[d.vendor, d.colorCapable ? t.plural("devicesLamps", d.lampCount) : t("devicesWhite"), cannot ? t("devicesCannotShow") : null].filter(Boolean).join(" · ")}
                    </span>
                  </span>
                  <span className="hidden flex-col items-end gap-1 @md:flex">
                    <SupportBadge support={d.color.state === "supported" ? d.color : d.brightness} />
                  </span>
                  <Switch label={t("devicesInclude", { name: d.name })} checked={on} onChange={(v) => onChange({ targets: toggleTarget(setup.targets, ids, d.id, v) })} />
                </li>
              );
            })}
          </ul>
        )}

        {usable.length > 0 && (
          <div className="flex items-center gap-3 rounded-lg bg-surface-2 p-3">
            <Icon name="link" size={16} className="shrink-0 text-fg-subtle" />
            <span className="min-w-0 flex-1">
              <span className="block text-sm text-fg">{t("syncTitle")}</span>
              <span id="lighting-sync-hint" className="block text-xs text-fg-muted">
                {usable.length < 2 ? t("syncOneDevice") : t("syncHint")}
              </span>
            </span>
            <Switch label={t("syncTitle")} checked={setup.sync} disabled={usable.length < 2} describedBy="lighting-sync-hint" onChange={(sync) => onChange({ sync })} />
          </div>
        )}

        {platformNote && (
          <p className="flex items-start gap-1.5 text-xs text-fg-muted">
            <Icon name="info" size={14} className="mt-px shrink-0 text-fg-subtle" />
            {platformNote}
          </p>
        )}

        {others.length > 0 && (
          <div className="flex flex-col gap-2 border-t border-line pt-4">
            <p className="eyebrow">{t("devicesOthers")}</p>
            <ul className="flex flex-col gap-2">
              {others.map((d) => (
                <li key={d.id} className="flex items-start gap-3">
                  <Icon name={KIND_ICON[d.kind]} size={16} className="mt-0.5 shrink-0 text-fg-subtle" />
                  <span className="min-w-0 flex-1">
                    <span className="flex flex-wrap items-center gap-2">
                      <span className="truncate text-sm text-fg-muted">{d.name}</span>
                      <SupportBadge support={d.color} />
                    </span>
                    <SupportReason support={d.color} />
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </Panel>
  );
}
