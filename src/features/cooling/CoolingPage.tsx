import { PageHeader } from "@/components/layout/PageHeader";
import { sectionLabel } from "@/app/sections";

/** Cooling center. Placeholder until the feature lands. */
export function CoolingPage() {
  return (
    <div className="page-gutter">
      <PageHeader title={sectionLabel("cooling")} />
    </div>
  );
}
