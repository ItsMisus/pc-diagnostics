import type { ElementType, HTMLAttributes, ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

/**
 * Surface system. Two questions decide the variant:
 * 1. Does it carry data? Then it is solid (`primary`, `secondary`, `inset`): charts, numbers
 *    and tables stay legible on an opaque background.
 * 2. Is it the focal element of the screen? `primary` (widget look, soft shadow): one or two
 *    per screen. Everything supporting it is `secondary` (same look, no shadow). Both have a
 *    24 px radius and 20 px padding, like the Overview widgets. `inset` groups content inside
 *    a panel.
 * `glass` is for panels that float over other content (e.g. over the 3D device stage).
 */
export type SurfaceVariant = "primary" | "secondary" | "inset" | "glass";

const SURFACE: Record<SurfaceVariant, string> = {
  primary: "surface-primary",
  secondary: "surface-secondary",
  inset: "surface-inset",
  glass: "glass rounded-xl",
};

const PADDING: Record<SurfaceVariant, string> = {
  primary: "p-5",
  secondary: "p-5",
  inset: "p-3",
  glass: "p-4",
};

export interface SurfaceProps extends HTMLAttributes<HTMLElement> {
  variant?: SurfaceVariant;
  /** Element to render (default `div`). */
  as?: ElementType;
  /** Removes the default padding. */
  flush?: boolean;
}

/** A bare surface with the variant's material, radius and padding. No header. */
export function Surface({ variant = "secondary", as: Tag = "div", flush = false, className = "", ...rest }: SurfaceProps) {
  return <Tag className={`${SURFACE[variant]} ${flush ? "" : PADDING[variant]} ${className}`} {...rest} />;
}

export interface PanelProps {
  title?: ReactNode;
  /** Optional short line under the title. Keep it rare (docs/ui-dashboard-brief.md, R2/R6). */
  description?: ReactNode;
  /** Small label above the title ("Stato", "Ultimo test"). */
  eyebrow?: ReactNode;
  /** Leading icon of the title (component glyphs: `cpu`, `gpu`, `memory`, `storage`…). */
  icon?: IconName;
  /** Right side of the header: a status badge, a "Show all" ghost button, a source caption. */
  aside?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Removes the inner padding (e.g. for edge-to-edge tables). The header keeps its padding. */
  flush?: boolean;
  variant?: SurfaceVariant;
  /** Heading level of the title (default 2). */
  level?: 2 | 3;
  /** id for the heading, e.g. to label a region elsewhere. */
  titleId?: string;
  /** Element of the panel (default `section`). */
  as?: "section" | "div" | "article" | "aside";
}

/**
 * Titled panel. The title names the content in one or two words ("CPU", "Avvisi"); details go
 * into the content, not the header.
 */
export function Panel({
  title,
  description,
  eyebrow,
  icon,
  aside,
  children,
  className = "",
  flush = false,
  variant = "secondary",
  level = 2,
  titleId,
  as: Tag = "section",
}: PanelProps) {
  const pad = PADDING[variant];
  const hasHeader = Boolean(title || aside || eyebrow);
  return (
    <Tag className={`${SURFACE[variant]} ${flush ? "" : pad} ${className}`} {...(titleId && title ? { "aria-labelledby": titleId } : {})}>
      {hasHeader && (
        <PanelHeader
          title={title}
          description={description}
          eyebrow={eyebrow}
          icon={icon}
          aside={aside}
          level={level}
          titleId={titleId}
          className={flush ? "px-5 pt-5" : ""}
          large={variant === "primary"}
        />
      )}
      {children}
    </Tag>
  );
}

/** The header of a `Panel`, for compositions that need it outside one. */
export function PanelHeader({
  title,
  description,
  eyebrow,
  icon,
  aside,
  level = 2,
  titleId,
  className = "",
  large = false,
}: {
  title?: ReactNode;
  description?: ReactNode | undefined;
  eyebrow?: ReactNode | undefined;
  icon?: IconName | undefined;
  aside?: ReactNode | undefined;
  level?: 2 | 3;
  titleId?: string | undefined;
  className?: string;
  /** Primary panels: 17 px title; secondary: 15 px. */
  large?: boolean;
}) {
  const H = level === 2 ? "h2" : "h3";
  return (
    <header className={`mb-3 flex min-h-7 items-start justify-between gap-3 ${className}`}>
      <div className="min-w-0">
        {eyebrow && <p className="eyebrow mb-1">{eyebrow}</p>}
        {title && (
          <H
            id={titleId}
            className={`flex items-center ${icon ? "gap-2.5" : "gap-2"} font-display font-semibold text-fg ${large ? "text-lg" : "text-md"}`}
          >
            {icon && (
              <span aria-hidden="true" className="satin grid size-8 shrink-0 place-items-center rounded-full text-fg-muted">
                <Icon name={icon} size={16} />
              </span>
            )}
            <span className="min-w-0">{title}</span>
          </H>
        )}
        {description && <p className="mt-0.5 text-xs text-fg-subtle">{description}</p>}
      </div>
      {aside && <div className="flex shrink-0 items-center gap-2">{aside}</div>}
    </header>
  );
}
