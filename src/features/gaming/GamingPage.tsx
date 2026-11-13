import { PageHeader } from "@/components/layout/PageHeader";
import { sectionLabel } from "@/app/sections";

/** Gaming center. Placeholder until the feature lands. */
export function GamingPage() {
  return (
    <div className="page-gutter">
      <PageHeader title={sectionLabel("gaming")} />
    </div>
  );
}
