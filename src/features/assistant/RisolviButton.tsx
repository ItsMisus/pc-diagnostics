import { Button } from "@/components/common/Button";
import { Icon } from "@/components/common/Icon";
import { useT } from "@/i18n";
import type { Finding } from "@/types/generated/Finding";
import { fixFlowStore } from "./fixFlow";
import { canShowRisolvi } from "./model";

/**
 * Opens the "Risolvi" dialog for one finding. `findingIndex` is the position in the latest
 * report's `findings` (backend order), not the display order. Renders nothing for info
 * findings. Clicking never sends data: the dialog first checks the setup, then shows a preview.
 * A secondary action on purpose: the page's primary action stays "Esegui diagnosi".
 */
export function RisolviButton({
  finding,
  findingIndex,
  label,
  compact = false,
}: {
  finding: Finding;
  findingIndex: number;
  /** Visible name of the finding, used for the accessible label. */
  label: string;
  compact?: boolean;
}) {
  const t = useT("assistant");
  if (findingIndex < 0 || !canShowRisolvi(finding)) return null;
  return (
    <Button
      variant="secondary"
      size={compact ? "sm" : "md"}
      aria-label={t("buttonAria", { label })}
      title={t("buttonTitle")}
      onClick={() => fixFlowStore.getState().start({ findingIndex, ruleId: finding.ruleId, title: label })}
    >
      <Icon name="sparkle" size={compact ? 14 : 16} className="text-fg-muted" />
      {t("button")}
    </Button>
  );
}
