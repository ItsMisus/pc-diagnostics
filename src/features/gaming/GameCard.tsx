import { m, useMotionValue, useReducedMotion, useSpring, useTransform } from "motion/react";
import type { PointerEvent } from "react";
import { Icon } from "@/components/common/Icon";
import { Tag } from "@/components/common/Tag";
import { useT } from "@/i18n";
import type { GameEntry } from "@/types/generated/GameEntry";
import { GameCover } from "./GameCover";
import type { ProfileIndicator } from "./model";

const TILT_DEG = 7;
const tiltSpring = { stiffness: 260, damping: 26, mass: 0.6 } as const;

/**
 * Library card: the 2:3 cover lifts on hover and tilts gently toward the pointer (3D
 * perspective, spring-smoothed, only while hovered and never with reduced motion), title
 * and source under it, a profile mark when the game has one.
 */
export function GameCard({
  game,
  indicator,
  playing,
  onOpen,
}: {
  game: GameEntry;
  indicator: ProfileIndicator;
  playing: boolean;
  onOpen: () => void;
}) {
  const t = useT("gaming");
  const reduce = useReducedMotion();
  const px = useMotionValue(0);
  const py = useMotionValue(0);
  const rotateY = useSpring(useTransform(px, [-0.5, 0.5], [-TILT_DEG, TILT_DEG]), tiltSpring);
  const rotateX = useSpring(useTransform(py, [-0.5, 0.5], [TILT_DEG, -TILT_DEG]), tiltSpring);
  const glareX = useTransform(px, [-0.5, 0.5], ["10%", "90%"]);
  const glare = useTransform(glareX, (x) => `radial-gradient(circle at ${x} 0%, rgb(255 255 255 / 0.22), transparent 55%)`);

  const onMove = (e: PointerEvent<HTMLButtonElement>) => {
    if (reduce || e.pointerType !== "mouse") return;
    const r = e.currentTarget.getBoundingClientRect();
    px.set((e.clientX - r.left) / r.width - 0.5);
    py.set((e.clientY - r.top) / r.height - 0.5);
  };
  const onLeave = () => {
    px.set(0);
    py.set(0);
  };

  return (
    <button
      type="button"
      onClick={onOpen}
      onPointerMove={onMove}
      onPointerLeave={onLeave}
      className="group flex min-w-0 flex-col gap-2.5 rounded-xl text-start [perspective:900px] focus-visible:outline-none"
    >
      <m.div
        {...(reduce ? {} : { style: { rotateX, rotateY, transformStyle: "preserve-3d" as const }, whileHover: { y: -6, scale: 1.015 } })}
        whileTap={{ scale: 0.985 }}
        transition={{ type: "spring", stiffness: 420, damping: 30 }}
        className={`relative overflow-hidden rounded-xl shadow-sm ring-1 ring-line transition-shadow duration-200 group-hover:shadow-lg group-focus-visible:ring-2 group-focus-visible:ring-[var(--focus)] ${
          game.hidden ? "opacity-55" : ""
        } ${playing ? "ring-2 ring-accent" : ""}`}
      >
        <GameCover game={game} />
        {!reduce && (
          <m.div aria-hidden="true" style={{ backgroundImage: glare }} className="pointer-events-none absolute inset-0 opacity-0 transition-opacity duration-200 group-hover:opacity-100" />
        )}
        {playing && (
          <span className="glass-thick absolute top-2 left-2 inline-flex h-6 items-center gap-1.5 rounded-full px-2 text-2xs font-semibold text-fg">
            <span className="size-1.5 rounded-full bg-ok" aria-hidden="true" />
            {t("heroPlaying")}
          </span>
        )}
        {indicator !== "none" && (
          <span
            className={`absolute top-2 right-2 grid size-7 bg-surface-raised shadow-xs place-items-center rounded-full ${indicator === "active" ? "text-accent-text" : "text-fg-subtle"}`}
            aria-hidden="true"
          >
            <Icon name="sliders" size={14} />
          </span>
        )}
      </m.div>
      <span className="min-w-0 px-0.5">
        <span className="block truncate text-sm font-semibold text-fg">{game.title}</span>
        <span className="mt-1 flex flex-wrap items-center gap-1.5">
          <Tag>{t(`source_${game.source}`)}</Tag>
          {game.hidden && <Tag>{t("hidden")}</Tag>}
          {indicator !== "none" && <span className="sr-only">{t(indicator === "active" ? "profileActive" : "profileOff")}</span>}
        </span>
      </span>
    </button>
  );
}
