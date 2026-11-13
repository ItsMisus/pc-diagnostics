import { m } from "motion/react";
import { type Section, visibleSections } from "@/app/sections";
import { sectionShortcutKey } from "@/app/shortcuts";
import { useAppStore } from "@/app/store";
import { Icon } from "@/components/common/Icon";
import { Kbd } from "@/components/common/Kbd";
import { Segmented } from "@/components/common/Segmented";
import { Tooltip } from "@/components/common/Tooltip";
import { useSetViewMode, useViewMode, useViewModeOptions } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { spring } from "@/lib/motion";
import { useIsMac, usePlatform } from "@/lib/platform";
import { shortcutLabel } from "@/lib/shortcuts";

/** Row height + gap; the selection indicator slides by this step. */
const ROW = 36;
const GAP = 2;
/** Hairline between sidebar groups (control centers / monitoring), margins included. */
const DIVIDER = 9;

/** Whether a divider sits above the i-th visible section: wherever the group changes. */
function dividerBefore(sections: readonly Section[], i: number): boolean {
  return i > 0 && sections[i]?.group !== sections[i - 1]?.group;
}

/** Widths of the floating panel (the window keeps an 8 px margin around it). */
export const SIDEBAR_WIDTH = { expanded: 236, collapsed: 72 } as const;

/**
 * Navigation: a floating glass panel inset from the window edges (8 px), full height.
 * Icons sit at a fixed x in both states, so collapsing never moves them: only the width
 * animates (one CSS transition, user-triggered) and the labels fade. The selection pill (a
 * satin capsule) springs between rows; the active icon takes the accent. Collapsed, each item
 * gets a tooltip with its name and shortcut.
 *
 * macOS: the window has an overlay title bar, so the top row of the panel is empty, draggable
 * space for the traffic lights (tauri.macos.conf.json `trafficLightPosition`). Windows keeps
 * its native title bar and shows the app name there instead.
 *
 * The footer holds the one global "Semplice | Avanzata" control (docs/ui-dashboard-brief.md,
 * R4): pages don't repeat it in their headers.
 */
export function Sidebar() {
  const section = useAppStore((s) => s.section);
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const setSection = useAppStore((s) => s.setSection);
  const toggle = useAppStore((s) => s.toggleSidebar);
  const t = useT("shell");
  const platform = usePlatform();
  const mac = useIsMac();
  // Only the sections this platform has (no Drivers on macOS); shortcuts follow this order.
  const sections = visibleSections(platform);
  const activeIndex = Math.max(
    0,
    sections.findIndex((s) => s.id === section),
  );
  let dividersAbove = 0;
  for (let i = 1; i <= activeIndex; i++) if (dividerBefore(sections, i)) dividersAbove++;

  const labelClass = `min-w-0 truncate transition-opacity duration-200 ${collapsed ? "pointer-events-none opacity-0" : "opacity-100 delay-75"}`;

  return (
    <nav
      aria-label={t("navLabel")}
      className="glass relative z-20 m-2 mr-0 flex shrink-0 flex-col rounded-xl transition-[width] duration-[var(--dur-nav)] ease-out"
      style={{ width: collapsed ? SIDEBAR_WIDTH.collapsed : SIDEBAR_WIDTH.expanded }}
    >
      {mac ? (
        // Traffic lights live here; the whole row drags the window.
        <div data-tauri-drag-region="deep" className="h-12 shrink-0" />
      ) : (
        <div className="flex h-14 shrink-0 items-center gap-3 overflow-hidden px-[22px]">
          <BrandMark />
          <span className={`font-display text-md font-semibold whitespace-nowrap ${labelClass}`}>PC Diagnostics</span>
        </div>
      )}

      <ul className="relative mt-1 flex min-h-0 flex-1 flex-col overflow-y-auto px-2.5 pb-2" style={{ gap: GAP }}>
        <m.li
          aria-hidden="true"
          className="satin pointer-events-none absolute inset-x-2.5 top-0 rounded-md"
          style={{ height: ROW }}
          initial={false}
          animate={{ y: activeIndex * (ROW + GAP) + dividersAbove * (DIVIDER + GAP) }}
          transition={spring.indicator}
        />
        {sections.map((s, i) => {
          const active = s.id === section;
          const label = t(`section_${s.id}`);
          const divider = dividerBefore(sections, i);
          const key = sectionShortcutKey(s.id, i);
          return (
            <li key={s.id} className="relative">
              {divider && (
                <div
                  role="separator"
                  aria-label={s.group === "control" ? t("groupControl") : undefined}
                  className="mx-[13px] border-t border-line"
                  style={{ marginTop: (DIVIDER - 1) / 2, marginBottom: (DIVIDER - 1) / 2 + GAP }}
                />
              )}
              <Tooltip content={<TooltipLabel label={label} shortcut={key === null ? null : shortcutLabel(key, platform)} />} enabled={collapsed} className="w-full">
                <button
                  type="button"
                  onClick={() => setSection(s.id)}
                  aria-current={active ? "page" : undefined}
                  aria-label={collapsed ? label : undefined}
                  className={`group flex w-full items-center gap-3 rounded-md px-[13px] text-sm font-medium transition-colors duration-150 focus-visible:outline-offset-0 ${
                    active ? "text-fg" : "text-fg-muted hover:bg-control hover:text-fg"
                  }`}
                  style={{ height: ROW }}
                >
                  <Icon
                    name={s.icon}
                    size={18}
                    className={`shrink-0 transition-[color,scale] duration-150 group-active:scale-90 ${active ? "text-accent-text" : ""}`}
                  />
                  <span className={labelClass}>{label}</span>
                </button>
              </Tooltip>
            </li>
          );
        })}
      </ul>

      <div className="shrink-0 space-y-1.5 border-t border-line p-2.5">
        <ViewModeControl collapsed={collapsed} />
        <Tooltip content={<TooltipLabel label={collapsed ? t("expand") : t("collapse")} shortcut={shortcutLabel("B", platform)} />} side={collapsed ? "right" : "top"} className="w-full">
          <button
            type="button"
            onClick={toggle}
            aria-label={collapsed ? t("expandSidebar") : t("collapseSidebar")}
            aria-expanded={!collapsed}
            className="flex w-full items-center gap-3 rounded-md px-[13px] text-sm text-fg-subtle transition-colors duration-150 hover:bg-control hover:text-fg focus-visible:outline-offset-0"
            style={{ height: 34 }}
          >
            <Icon name={collapsed ? "expand" : "collapse"} size={18} className="shrink-0" />
            <span className={labelClass}>{t("collapse")}</span>
          </button>
        </Tooltip>
      </div>
    </nav>
  );
}

/**
 * Global "Semplice | Avanzata" switch. Expanded: a segmented control. Collapsed: one button
 * that flips the mode, labelled with the current mode in its tooltip.
 */
function ViewModeControl({ collapsed }: { collapsed: boolean }) {
  const mode = useViewMode();
  const setMode = useSetViewMode();
  const options = useViewModeOptions();
  const t = useT("common");
  const current = options.find((o) => o.value === mode)?.label ?? "";

  if (collapsed) {
    // The icon shows the current mode; the tooltip names it and what a click switches to.
    const next = options.find((o) => o.value !== mode)?.label ?? "";
    return (
      <Tooltip content={`${t("viewModeLabel")}: ${current} · ${t("viewModeSwitchTo", { mode: next })}`} side="right" className="w-full">
        <button
          type="button"
          onClick={() => setMode(mode === "simple" ? "advanced" : "simple")}
          aria-label={`${t("viewModeLabel")}: ${current}`}
          className="flex w-full items-center rounded-md px-[13px] text-fg-subtle transition-colors duration-150 hover:bg-control hover:text-fg focus-visible:outline-offset-0"
          style={{ height: 34 }}
        >
          <Icon name={mode === "advanced" ? "viewAdvanced" : "viewSimple"} size={18} className="shrink-0" />
        </button>
      </Tooltip>
    );
  }
  return (
    <Tooltip content={t("viewModeHint")} side="top" multiline className="w-full">
      <div className="w-full px-1 pb-0.5">
        <p className="eyebrow mb-1.5 px-0.5">{t("viewModeLabel")}</p>
        <Segmented size="sm" fullWidth label={t("viewModeLabel")} options={options} value={mode} onChange={setMode} />
      </div>
    </Tooltip>
  );
}

function TooltipLabel({ label, shortcut }: { label: string; shortcut: string | null }) {
  return (
    <span className="flex items-center gap-2">
      {label}
      {shortcut !== null && <Kbd>{shortcut}</Kbd>}
    </span>
  );
}

/** App mark: a pulse line in an ink tile. Decorative; the name is in the label/title. */
function BrandMark() {
  return (
    <span aria-hidden="true" className="grid size-7 shrink-0 place-items-center rounded-sm bg-fg text-canvas shadow-sm">
      <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
        <path d="M1.5 8.5h3l1.75-4.5 3.5 8.5 1.75-4h3" />
      </svg>
    </span>
  );
}
