import { useT } from "@/i18n";
import type { GameEntry } from "@/types/generated/GameEntry";
import { coverBackground, coverMonogram } from "./cover";
import { useGameArt } from "./gamingStore";

/**
 * The 2:3 cover of a game: the launcher's own art when it has one, otherwise a typographic
 * placeholder (gradient from the title, the title in the display face, the app icon when the
 * bundle has one). Decorative: the card names the game in text.
 */
export function GameCover({ game, size = "md", className = "" }: { game: GameEntry; size?: "sm" | "md" | "lg"; className?: string }) {
  const cover = useGameArt(game.id, "cover", game.hasCover);
  const icon = useGameArt(game.id, "icon", !game.hasCover && game.hasIcon);
  const t = useT("gaming");
  return (
    <div className={`relative aspect-[2/3] w-full overflow-hidden bg-surface-3 ${className}`} aria-hidden="true">
      {cover ? (
        <img src={cover} alt="" draggable={false} className="absolute inset-0 size-full object-cover" />
      ) : game.hasCover ? null : (
        <GeneratedCover title={game.title} icon={icon} size={size} label={t("generatedCover")} />
      )}
    </div>
  );
}

/**
 * A game without box art. With the app's icon: the icon itself, crisp in the middle, over a
 * deep blurred copy of it (its own colours fill the cover, like a store tile). Without one:
 * a smooth two-tone gradient from the title with a soft light from the top and the initials.
 */
function GeneratedCover({ title, icon, size, label }: { title: string; icon: string | null; size: "sm" | "md" | "lg"; label: string }) {
  const text = size === "sm" ? "text-md" : size === "lg" ? "text-2xl" : "text-lg";
  return (
    <div className="absolute inset-0 flex flex-col justify-between p-3" style={icon ? undefined : { backgroundImage: coverBackground(title) }} title={label}>
      {icon ? (
        <>
          <img src={icon} alt="" draggable={false} className="pointer-events-none absolute inset-0 size-full scale-[1.6] object-cover blur-2xl saturate-150" />
          <div className="pointer-events-none absolute inset-0 bg-black/35" />
        </>
      ) : (
        <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_70%_at_50%_0%,rgb(255_255_255/0.22),transparent_60%)]" />
      )}
      {/* Bottom shade so the title reads on any colour. */}
      <div className="pointer-events-none absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/45 to-transparent" />
      <div className="relative flex flex-1 items-center justify-center">
        {icon ? (
          <img src={icon} alt="" draggable={false} className="w-1/2 max-w-32 drop-shadow-[0_10px_22px_rgb(0_0_0/0.45)]" />
        ) : (
          <span className="font-display text-4xl font-bold tracking-tight text-white/85">{coverMonogram(title)}</span>
        )}
      </div>
      <p className={`relative line-clamp-3 font-display font-semibold leading-tight text-white [text-shadow:0_1px_8px_rgb(0_0_0/0.35)] ${text}`}>
        {title}
      </p>
    </div>
  );
}
