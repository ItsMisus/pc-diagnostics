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

function GeneratedCover({ title, icon, size, label }: { title: string; icon: string | null; size: "sm" | "md" | "lg"; label: string }) {
  const text = size === "sm" ? "text-md" : size === "lg" ? "text-2xl" : "text-lg";
  return (
    <div className="absolute inset-0 flex flex-col justify-between p-3" style={{ backgroundImage: coverBackground(title) }} title={label}>
      {/* Fine diagonal grain so the placeholder reads as a printed label, not as missing art. */}
      <div className="pointer-events-none absolute inset-0 opacity-[0.12] [background-image:repeating-linear-gradient(135deg,rgb(255_255_255/0.5)_0_1px,transparent_1px_7px)]" />
      <div className="relative flex flex-1 items-center justify-center">
        {icon ? (
          <img src={icon} alt="" draggable={false} className="w-1/2 max-w-32 drop-shadow-[0_8px_18px_rgb(0_0_0/0.35)]" />
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
