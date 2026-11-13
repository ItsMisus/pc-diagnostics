import { PageHeader } from "@/components/layout/PageHeader";
import { sectionLabel } from "@/app/sections";

/** Performance center. Placeholder until the feature lands. */
export function PerformancePage() {
  return (
    <div className="page-gutter">
      <PageHeader title={sectionLabel("performance")} />
    </div>
  );
}
