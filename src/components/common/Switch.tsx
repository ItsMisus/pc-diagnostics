import { m } from "motion/react";
import { spring } from "@/lib/motion";

interface SwitchProps {
  /** Accessible name (the visible label usually sits next to the switch). */
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  /** id of a visible element describing the setting (its hint). */
  describedBy?: string;
}

/**
 * On/off switch (role="switch"), macOS proportions: capsule track that fills with the accent
 * when on, a white thumb that springs across and shrinks slightly while pressed.
 */
export function Switch({ label, checked, onChange, disabled = false, describedBy }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      aria-describedby={describedBy}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`group relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full p-[2px] transition-[background-color,box-shadow] duration-200 disabled:opacity-45 ${
        checked
          ? "bg-accent-solid shadow-[inset_0_0_0_1px_rgb(0_0_0/0.06)]"
          : "bg-control-track shadow-[inset_0_0_0_1px_var(--line-strong)] hover:shadow-[inset_0_0_0_1px_var(--fg-subtle)]"
      }`}
    >
      <m.span
        aria-hidden="true"
        initial={false}
        animate={{ x: checked ? 16 : 0 }}
        transition={spring.snappy}
        className="block size-[18px] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.25),0_0_0_0.5px_rgb(0_0_0/0.06)] transition-[scale] duration-150 group-active:scale-90"
      />
    </button>
  );
}
