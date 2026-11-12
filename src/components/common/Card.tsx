import { Panel, type PanelProps } from "./Panel";

/**
 * Legacy name of `Panel` (same props). Existing pages import `Card`; new code should import
 * `Panel` from `./Panel` and choose a `variant` deliberately (primary for the focal panel,
 * secondary for supporting ones). Defaults to `secondary`.
 */
export function Card(props: PanelProps) {
  return <Panel {...props} />;
}

export type CardProps = PanelProps;
