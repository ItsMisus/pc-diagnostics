import { Icon } from "@/components/common/Icon";

interface SearchFieldProps {
  /** Accessible name, visually hidden. */
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** Width utility for the field (e.g. `w-56`). */
  className?: string;
}

/**
 * Capsule search input with a leading magnifier. Border, focus ring and the clear button come
 * from the global native input styles; the icon brightens while the field has focus. Esc
 * clears a non-empty field (and only then, so Esc still closes an enclosing dialog).
 */
export function SearchField({ label, value, onChange, placeholder, className = "" }: SearchFieldProps) {
  return (
    <label className={`group relative flex items-center ${className}`}>
      <span className="sr-only">{label}</span>
      <Icon
        name="search"
        size={14}
        className="pointer-events-none absolute left-3 text-fg-subtle transition-colors duration-150 group-focus-within:text-fg-muted"
      />
      <input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && value !== "") {
            e.preventDefault();
            e.stopPropagation();
            onChange("");
          }
        }}
        placeholder={placeholder}
        className="h-8 w-full min-w-0 rounded-full pr-3 pl-8"
      />
    </label>
  );
}
