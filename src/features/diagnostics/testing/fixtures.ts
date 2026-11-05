import type { CoverageItem } from "@/types/generated/CoverageItem";
import type { DiagnosticReport } from "@/types/generated/DiagnosticReport";
import type { Finding } from "@/types/generated/Finding";

export function finding(ruleId: string, patch: Partial<Finding> = {}): Finding {
  return {
    ruleId,
    title: ruleId,
    component: "memory",
    severity: "warning",
    kind: "observation",
    confidence: "medium",
    description: "d",
    evidence: [],
    possibleCauses: [],
    recommendation: "r",
    limitations: [],
    simple: { headline: "", meaning: "", action: "" },
    ...patch,
  };
}

export function coverage(component: CoverageItem["component"], status: CoverageItem["status"], note = ""): CoverageItem {
  return { component, status, note };
}

export function report(patch: Partial<DiagnosticReport> = {}): DiagnosticReport {
  return {
    id: "rep",
    generatedAt: 1000,
    appVersion: "0.1.0",
    rulesVersion: 1,
    observedWindowS: 300,
    findings: [],
    coverage: [],
    limitations: [],
    ...patch,
  };
}
