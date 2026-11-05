import { m } from "motion/react";
import { spring } from "@/lib/motion";

interface SwitchProps {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

/** On/off switch (role="switch"). The thumb springs across; the track color fades. */
export function Switch({ label, checked, onChange, disabled = false }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`group relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border p-0.5 transition-colors duration-200 disabled:opacity-45 ${
        checked ? "border-accent-solid bg-accent-solid" : "border-line-strong bg-surface-3 hover:border-fg-subtle"
      }`}
    >
      <m.span
        aria-hidden="true"
        initial={false}
        animate={{ x: checked ? 16 : 0 }}
        transition={spring.snappy}
        className={`block size-3.5 rounded-full shadow-sm transition-[background-color,scale] duration-150 group-active:scale-90 ${
          checked ? "bg-white" : "bg-fg-subtle"
        }`}
      />
    </button>
  );
}
