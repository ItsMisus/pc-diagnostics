import { type ReactNode, useEffect, useId, useState } from "react";
import { Icon, type IconName } from "@/components/common/Icon";
import { StateView } from "@/components/common/StateView";
import { Td, Table } from "@/components/common/Table";
import { useT } from "@/i18n";
import { stateKindForError } from "@/lib/errors";
import { formatUnitValue } from "@/lib/format";
import { commands } from "@/services/commands";
import { toAppError } from "@/services/ipc";
import type { AppError } from "@/types/generated/AppError";
import type { BenchmarkComparison } from "@/types/generated/BenchmarkComparison";
import { comparisonSummary, type DeltaDirection, deltaView, MEANINGFUL_CHANGE_PCT } from "./compare";
import { INSET, Th } from "./ui";

type State =
  | { status: "loading" }
  | { status: "ready"; data: BenchmarkComparison }
  | { status: "none" }
  | { status: "error"; error: AppError };

const TONE_TEXT = { ok: "text-ok", bad: "text-bad", neutral: "text-fg-muted" } as const;

/** Shape cue next to the words, so the direction never rests on color alone. */
const DIRECTION_GLYPH: Partial<Record<DeltaDirection, IconName>> = { better: "success", worse: "error" };

/**
 * "Confronto con l'esecuzione precedente". `previousRunId` null = latest earlier comparable.
 * `aside` sits in the header (e.g. History's run picker).
 */
export function ComparisonPanel({
  currentRunId,
  previousRunId = null,
  aside,
}: {
  currentRunId: string;
  previousRunId?: string | null;
  aside?: ReactNode;
}) {
  const [state, setState] = useState<State>({ status: "loading" });
  const [nonce, setNonce] = useState(0);
  const titleId = useId();
  const t = useT("benchmarks");

  useEffect(() => {
    let active = true;
    setState({ status: "loading" });
    commands.compareBenchmarks(currentRunId, previousRunId).then(
      (data) => active && setState({ status: "ready", data }),
      (e: unknown) => {
        if (!active) return;
        const error = toAppError(e);
        setState(error.code === "notFound" ? { status: "none" } : { status: "error", error });
      },
    );
    return () => {
      active = false;
    };
  }, [currentRunId, previousRunId, nonce]);

  return (
    <section aria-labelledby={titleId} className={`${INSET} min-w-0 p-4`}>
      <header className="mb-3 flex min-h-5 flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <h3 id={titleId} className="min-w-0 text-sm font-semibold text-fg">
          {t("compareTitle")}
        </h3>
        {aside}
      </header>
      <ComparisonBody state={state} onRetry={() => setNonce((n) => n + 1)} />
    </section>
  );
}

function ComparisonBody({ state, onRetry }: { state: State; onRetry: () => void }) {
  const t = useT("benchmarks");
  switch (state.status) {
    case "loading":
      return <StateView compact kind="loading" title={t("comparing")} />;
    case "none":
      return <StateView compact kind="empty" title={t("noComparable")} description={t("noComparableDesc")} />;
    case "error":
      return (
        <StateView
          compact
          kind={stateKindForError(state.error)}
          title={t("compareUnavailable")}
          error={state.error}
          onRetry={onRetry}
        />
      );
    case "ready":
      return <ComparisonTable data={state.data} />;
  }
}

function ComparisonTable({ data }: { data: BenchmarkComparison }) {
  const t = useT("benchmarks");
  const summary = comparisonSummary(data);
  const noteId = useId();
  return (
    <div className="flex flex-col gap-3">
      {summary.kind === "notComparable" && <p className="text-sm text-fg">{t("notComparable")}</p>}
      {summary.caveats.length > 0 && (
        <div role="note" className="rounded-md border border-warn/25 bg-warn-soft px-3 py-2.5 text-sm text-fg">
          <p className="flex items-start gap-2 font-medium">
            <Icon name="alert" className="mt-0.5 shrink-0 text-warn" />
            <span>{summary.kind === "comparable" ? t("caveatsComparable") : t("caveatsReasons")}</span>
          </p>
          <ul className="mt-1 list-disc pl-10 text-fg-muted marker:text-warn">
            {summary.caveats.map((c) => (
              <li key={c} className="break-words">
                {c}
              </li>
            ))}
          </ul>
        </div>
      )}
      {summary.kind === "comparable" && (
        <>
          <Table
            caption={t("deltasCaption")}
            head={
              <>
                <Th>{t("colMetric")}</Th>
                <Th align="right">{t("colCurrent")}</Th>
                <Th align="right">{t("colPrevious")}</Th>
                <Th align="right">
                  <span aria-describedby={noteId}>{t("colChange")}</span>
                </Th>
              </>
            }
          >
            {data.deltas.map((d) => {
              const view = deltaView(d, data.comparable, t);
              const glyph = DIRECTION_GLYPH[view.direction];
              return (
                <tr key={d.id}>
                  <Td className="text-fg-muted">{d.label}</Td>
                  <Td className="text-right font-medium whitespace-nowrap text-fg">{formatUnitValue(d.current, d.unit)}</Td>
                  <Td className="text-right whitespace-nowrap text-fg-muted">{formatUnitValue(d.previous, d.unit)}</Td>
                  <Td className={`text-right whitespace-nowrap ${TONE_TEXT[view.tone]}`}>
                    <span className="inline-flex items-center justify-end gap-1.5">
                      {glyph && <Icon name={glyph} size={12} strokeWidth={1.75} />}
                      <span className="font-medium">{view.text}</span>
                      {view.direction === "better" && <span>· {t("better")}</span>}
                      {view.direction === "worse" && <span>· {t("worse")}</span>}
                    </span>
                    <span className="sr-only"> — {view.description}</span>
                  </Td>
                </tr>
              );
            })}
          </Table>
          <p id={noteId} className="text-2xs text-fg-subtle">
            {t("noiseNote", { pct: MEANINGFUL_CHANGE_PCT })}
          </p>
        </>
      )}
    </div>
  );
}
