import { useShallow } from "zustand/react/shallow";
import { Card } from "@/components/common/Card";
import { Icon } from "@/components/common/Icon";
import { useT } from "@/i18n";
import { formatBytes, formatBytesPerSec, formatPercent } from "@/lib/format";
import { useProcesses } from "./processStore";
import { weightShare } from "./rows";

/**
 * Collapsible methodology. Selects only primitives (shallow-compared), so it re-renders
 * when the context actually changes, not on every snapshot object. The whole card header
 * is the native `<summary>`; height animates via the global `::details-content` rule.
 */
export function ScoreExplanation() {
  const t = useT("processes");
  const m = useProcesses(
    useShallow((s) => {
      const snap = s.snapshot;
      return {
        ready: snap !== null,
        version: snap?.methodologyVersion ?? 0,
        cpu: snap?.weights.cpu ?? 0,
        ram: snap?.weights.ram ?? 0,
        io: snap?.weights.io ?? 0,
        processors: snap?.context.logicalProcessors ?? 0,
        totalMemory: snap?.context.totalMemoryBytes ?? 0,
        availableMemory: snap?.context.availableMemoryBytes ?? 0,
        ioReference: snap?.context.ioReferenceBytesPerSec ?? 0,
        windowS: snap?.windowS ?? 0,
        intervalMs: snap?.intervalMs ?? 0,
      };
    }),
  );
  if (!m.ready) return null;
  const weights = [m.cpu, m.ram, m.io];
  const share = (w: number) => formatPercent(weightShare(w, weights));

  return (
    <Card flush>
      <details className="group">
        <summary className="flex items-center gap-2 rounded-md px-4 py-3 text-[13px] font-semibold text-fg-muted transition-colors duration-150 select-none hover:text-fg focus-visible:outline-offset-[-2px]">
          <Icon
            name="chevronRight"
            size={14}
            strokeWidth={1.75}
            className="shrink-0 text-fg-subtle transition-transform duration-200 ease-out group-open:rotate-90"
          />
          {t("scoreTitle")}
        </summary>
        <div className="flex max-w-prose flex-col gap-3 pt-1 pr-4 pb-4 pl-9.5 text-sm text-fg-muted">
          <p className="text-fg">{t("scoreDisclaimer")}</p>
          <p>
            {t("scoreIntro")}
            <span className="mt-2 block w-fit max-w-full rounded-md border border-line bg-surface-2/60 px-2.5 py-1.5 font-mono text-xs text-fg tabular">
              Impact = {share(m.cpu)} × CPU + {share(m.ram)} × RAM + {share(m.io)} × I/O
            </span>
          </p>
          <ul className="list-disc space-y-1 pl-5 marker:text-fg-subtle">
            <li>
              <span className="text-fg">CPU</span>: {t("scoreCpu", { processors: m.processors })}
            </li>
            <li>
              <span className="text-fg">RAM</span>:{" "}
              {t("scoreRam", { total: formatBytes(m.totalMemory), available: formatBytes(m.availableMemory) })}
            </li>
            <li>
              <span className="text-fg">I/O</span>: {t("scoreIo", { reference: formatBytesPerSec(m.ioReference) })}
            </li>
          </ul>
          <p>{t("scoreModes", { interval: (m.intervalMs / 1000).toFixed(0), window: Math.round(m.windowS) })}</p>
          <p className="text-xs text-fg-subtle">{t("scoreMethodology", { version: m.version })}</p>
        </div>
      </details>
    </Card>
  );
}
