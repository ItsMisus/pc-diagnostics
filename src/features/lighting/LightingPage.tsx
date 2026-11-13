import { PageHeader } from "@/components/layout/PageHeader";
import { sectionLabel } from "@/app/sections";

/** Lighting center. Placeholder until the feature lands. */
export function LightingPage() {
  return (
    <div className="page-gutter">
      <PageHeader title={sectionLabel("lighting")} />
    </div>
  );
}
