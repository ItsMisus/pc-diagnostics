import { m } from "motion/react";
import { SECTIONS } from "@/app/sections";
import { useAppStore } from "@/app/store";
import { Icon } from "@/components/common/Icon";
import { Tooltip } from "@/components/common/Tooltip";
import { useT } from "@/i18n";
import { spring } from "@/lib/motion";

/** Row height + gap; the selection pill slides by this step. */
const ROW = 32;
const GAP = 2;

/**
 * Navigation rail. Icons sit at a fixed x in both states, so collapsing never moves them:
 * only the width animates (CSS, compositor-friendly enough for one column) and the labels
 * fade. The selection pill springs between rows. Collapsed, each item gets a tooltip.
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
        collapsed ? "w-14" : "w-56"
      }`}
    >
      <div className="flex h-14 shrink-0 items-center gap-2.5 px-[18px]">
        <BrandMark />
        <span className={`text-[13px] font-semibold tracking-tight whitespace-nowrap ${labelClass}`}>PC Diagnostics</span>
      </div>

      <ul className="relative flex flex-1 flex-col px-2" style={{ gap: GAP }}>
        <m.li
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-2 top-0 rounded-md border border-line bg-surface-2"
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
                  className={`group flex w-full items-center gap-3 rounded-md px-[11px] text-[13px] transition-colors duration-150 ${
                    active ? "font-medium text-fg" : "text-fg-muted hover:bg-surface-2 hover:text-fg"
                  }`}
                  style={{ height: ROW }}
                >
                  <Icon
                    name={s.icon}
                    className={`shrink-0 transition-[color,scale] duration-150 group-active:scale-90 ${active ? "text-accent-text" : ""}`}
                  />
                  <span className={labelClass}>{label}</span>
                </button>
              </Tooltip>
            </li>
          );
        })}
      </ul>

      <div className="shrink-0 p-2">
        <Tooltip content={<TooltipLabel label={collapsed ? t("expand") : t("collapse")} shortcut="Ctrl+B" />} side={collapsed ? "right" : "top"} className="w-full">
          <button
            type="button"
            onClick={toggle}
            aria-label={collapsed ? t("expandSidebar") : t("collapseSidebar")}
            aria-expanded={!collapsed}
            className="flex w-full items-center gap-3 rounded-md px-[11px] text-[13px] text-fg-subtle transition-colors duration-150 hover:bg-surface-2 hover:text-fg"
            style={{ height: ROW }}
          >
            <Icon name={collapsed ? "expand" : "collapse"} className="shrink-0" />
            <span className={labelClass}>{t("collapse")}</span>
          </button>
        </Tooltip>
      </div>
    </nav>
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

/** App mark: a pulse line in an accent tile. Decorative; the name is in the label/title. */
function BrandMark() {
  return (
    <span aria-hidden="true" className="grid size-5 shrink-0 place-items-center rounded-md bg-accent-solid text-accent-fg shadow-xs">
      <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
        <path d="M1.5 8.5h3l1.75-4.5 3.5 8.5 1.75-4h3" />
      </svg>
    </span>
  );
}
