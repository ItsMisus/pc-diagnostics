import { m } from "motion/react";
import { SECTIONS } from "@/app/sections";
import { useAppStore } from "@/app/store";
import { Icon } from "@/components/common/Icon";
import { Segmented } from "@/components/common/Segmented";
import { Tooltip } from "@/components/common/Tooltip";
import { useSetViewMode, useViewMode, useViewModeOptions } from "@/hooks/useViewMode";
import { useT } from "@/i18n";
import { spring } from "@/lib/motion";

/** Row height + gap; the selection indicator slides by this step. */
const ROW = 40;
const GAP = 4;

/**
 * Navigation rail, dashboard style: tall rows, muted grey items, the active one in full
 * white and bold on a soft fill. Icons sit at a fixed x in both states, so
 * collapsing never moves them: only the width animates and the labels fade. The selection
 * springs between rows. Collapsed, each item gets a tooltip.
 *
 * The footer holds the one global "Semplice | Avanzata" control (rule R4 of
 * docs/ui-dashboard-brief.md): pages don't repeat it in their headers.
 */
export function Sidebar() {
  const section = useAppStore((s) => s.section);
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const setSection = useAppStore((s) => s.setSection);
  const toggle = useAppStore((s) => s.toggleSidebar);
  const t = useT("shell");
  const activeIndex = Math.max(
    0,
    SECTIONS.findIndex((s) => s.id === section),
  );

  const labelClass = `truncate transition-opacity duration-200 ${collapsed ? "pointer-events-none opacity-0" : "opacity-100 delay-75"}`;

  return (
    <nav
      aria-label={t("navLabel")}
      className={`relative z-20 flex shrink-0 flex-col border-r border-line bg-sidebar transition-[width] duration-[var(--dur-nav)] ease-out ${
        collapsed ? "w-16" : "w-60"
      }`}
    >
      <div className="flex h-16 shrink-0 items-center gap-3 px-[22px]">
        <BrandMark />
        <span className={`font-display text-[15px] font-semibold tracking-tight whitespace-nowrap ${labelClass}`}>PC Diagnostics</span>
      </div>

      <ul className="relative mt-2 flex flex-1 flex-col px-2.5" style={{ gap: GAP }}>
        <m.li
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-2.5 top-0 rounded-md bg-surface-2"
          style={{ height: ROW }}
          initial={false}
          animate={{ y: activeIndex * (ROW + GAP) }}
          transition={spring.indicator}
        />
        {SECTIONS.map((s, i) => {
          const active = s.id === section;
          const label = t(`section_${s.id}`);
          return (
            <li key={s.id} className="relative">
              <Tooltip content={<TooltipLabel label={label} index={i} />} enabled={collapsed} className="w-full">
                <button
                  type="button"
                  onClick={() => setSection(s.id)}
                  aria-current={active ? "page" : undefined}
                  aria-label={collapsed ? label : undefined}
                  className={`group flex w-full items-center gap-3.5 rounded-md px-[13px] text-sm transition-colors duration-150 ${
                    active ? "font-semibold text-fg" : "text-fg-muted hover:bg-surface-2 hover:text-fg"
                  }`}
                  style={{ height: ROW }}
                >
                  <Icon
                    name={s.icon}
                    className={`shrink-0 transition-[color,scale] duration-150 group-active:scale-90 ${active ? "text-fg" : ""}`}
                  />
                  <span className={labelClass}>{label}</span>
                </button>
              </Tooltip>
            </li>
          );
        })}
      </ul>

      <div className="shrink-0 space-y-1 border-t border-line p-2.5">
        <ViewModeControl collapsed={collapsed} />
        <Tooltip content={<TooltipLabel label={collapsed ? t("expand") : t("collapse")} shortcut="Ctrl+B" />} side={collapsed ? "right" : "top"} className="w-full">
          <button
            type="button"
            onClick={toggle}
            aria-label={collapsed ? t("expandSidebar") : t("collapseSidebar")}
            aria-expanded={!collapsed}
            className="flex w-full items-center gap-3.5 rounded-lg px-[13px] text-[13px] text-fg-subtle transition-colors duration-150 hover:bg-surface-2 hover:text-fg"
            style={{ height: 36 }}
          >
            <Icon name={collapsed ? "expand" : "collapse"} className="shrink-0" />
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
    return (
      <Tooltip content={`${t("viewModeLabel")}: ${current}`} side="right" className="w-full">
        <button
          type="button"
          onClick={() => setMode(mode === "simple" ? "advanced" : "simple")}
          aria-label={`${t("viewModeLabel")}: ${current}`}
          className="flex w-full items-center rounded-lg px-[13px] text-fg-subtle transition-colors duration-150 hover:bg-surface-2 hover:text-fg"
          style={{ height: 36 }}
        >
          <Icon name="info" className="shrink-0" />
        </button>
      </Tooltip>
    );
  }
  return (
    <div className="px-1 pb-1" title={t("viewModeHint")}>
      <p className="eyebrow mb-1.5 px-0.5">{t("viewModeLabel")}</p>
      <div className="[&>div]:flex [&>div]:w-full [&_button]:flex-1">
        <Segmented size="sm" label={t("viewModeLabel")} options={options} value={mode} onChange={setMode} />
      </div>
    </div>
  );
}

function TooltipLabel({ label, index, shortcut }: { label: string; index?: number; shortcut?: string }) {
  const keys = shortcut ?? (index !== undefined ? `Ctrl+${index + 1}` : undefined);
  return (
    <span className="flex items-center gap-2">
      {label}
      {keys && <kbd className="font-sans text-2xs text-fg-subtle">{keys}</kbd>}
    </span>
  );
}

/** App mark: a pulse line in a light tile. Decorative; the name is in the label/title. */
function BrandMark() {
  return (
    <span
      aria-hidden="true"
      className="grid size-7 shrink-0 place-items-center rounded-md bg-fg text-canvas"
    >
      <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
        <path d="M1.5 8.5h3l1.75-4.5 3.5 8.5 1.75-4h3" />
      </svg>
    </span>
  );
}
