import { useEffect, useState } from "react";
import { Button } from "@/components/common/Button";
import { Icon, type IconName } from "@/components/common/Icon";
import { Modal } from "@/components/common/Modal";
import { Panel } from "@/components/common/Panel";
import { Tag } from "@/components/common/Tag";
import { ProfileSwitcher, type ProfileOption } from "@/components/control/ProfileSwitcher";
import { useT } from "@/i18n";
import type { LightingEffect } from "@/types/generated/LightingEffect";
import type { LightingProfile } from "@/types/generated/LightingProfile";
import { useLighting } from "./store";

const EFFECT_ICON: Record<LightingEffect["kind"], IconName> = {
  off: "power",
  static: "bulb",
  breathing: "pulse",
  rainbow: "palette",
  wave: "ledStrip",
  gradient: "sliders",
  colorCycle: "refresh",
};

/**
 * Profiles: built-in and the user's own as tiles (choosing one loads it into the editor;
 * "Applica" sends it), save as new, rename or update and delete for the user's profiles, and
 * the game override strip.
 */
export function ProfilesPanel({ profiles, gameOverride }: { profiles: readonly LightingProfile[]; gameOverride: string | null }) {
  const t = useT("lighting");
  const profileId = useLighting((s) => s.profileId);
  const pickProfile = useLighting((s) => s.pickProfile);
  const save = useLighting((s) => s.save);
  const remove = useLighting((s) => s.remove);
  const busy = useLighting((s) => s.busy);
  const [dialog, setDialog] = useState<null | "new" | "rename" | "delete">(null);
  const current = profiles.find((p) => p.id === profileId) ?? null;
  const own = current && !current.builtin ? current : null;
  const game = gameOverride ? profiles.find((p) => p.id === gameOverride) : null;

  const options: ProfileOption[] = profiles.map((p) => ({
    id: p.id,
    name: p.name,
    hint: p.effect.kind === "off" ? t("effect_off") : t("profileHint", { effect: t(`effect_${p.effect.kind}`), brightness: p.brightness }),
    icon: p.builtin ? EFFECT_ICON[p.effect.kind] : "sliders",
    badge: p.builtin ? undefined : <Tag>{t("profileCustom")}</Tag>,
  }));

  return (
    <Panel
      title={t("profilesTitle")}
      icon="sliders"
      aside={
        <div className="flex flex-wrap items-center gap-1">
          <Button size="sm" variant="ghost" icon="plus" onClick={() => setDialog("new")}>
            {t("profileSaveNew")}
          </Button>
          {own && (
            <>
              <Button size="sm" variant="ghost" onClick={() => setDialog("rename")}>
                {t("profileUpdate")}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setDialog("delete")}>
                {t("profileDelete")}
              </Button>
            </>
          )}
        </div>
      }
    >
      <div className="flex flex-col gap-4">
        <ProfileSwitcher label={t("profilesTitle")} options={options} value={profileId} onChange={pickProfile} columns={3} />
        <div className={`flex items-start gap-2 rounded-lg p-3 text-xs ${game ? "bg-accent-soft text-fg" : "bg-surface-2 text-fg-muted"}`}>
          <Icon name="gamepad" size={16} className={`mt-px shrink-0 ${game ? "text-accent-text" : "text-fg-subtle"}`} />
          <span>{game ? t("gameOverrideActive", { name: game.name }) : t("gameOverrideNone")}</span>
        </div>
      </div>

      <NameDialog
        open={dialog === "new" || dialog === "rename"}
        title={dialog === "rename" ? t("profileUpdateTitle") : t("profileSaveNewTitle")}
        initial={dialog === "rename" && own ? own.name : ""}
        busy={busy === "save"}
        onClose={() => setDialog(null)}
        onSave={async (name) => {
          if (await save(name, dialog === "new")) setDialog(null);
        }}
      />
      <Modal
        open={dialog === "delete"}
        onClose={() => setDialog(null)}
        size="sm"
        icon="warning"
        title={t("profileDeleteTitle")}
        description={own?.name}
        footer={
          <>
            <Button onClick={() => setDialog(null)}>{t("cancel")}</Button>
            <Button
              variant="danger"
              loading={busy === "delete"}
              onClick={async () => {
                if (own) await remove(own.id);
                setDialog(null);
              }}
            >
              {t("profileDelete")}
            </Button>
          </>
        }
      >
        <p className="text-base text-fg-muted">{t("profileDeleteHint")}</p>
      </Modal>
    </Panel>
  );
}

function NameDialog({ open, title, initial, busy, onClose, onSave }: { open: boolean; title: string; initial: string; busy: boolean; onClose: () => void; onSave: (name: string) => void }) {
  const t = useT("lighting");
  const [name, setName] = useState(initial);
  useEffect(() => {
    if (open) setName(initial);
  }, [open, initial]);
  const valid = name.trim().length > 0 && name.trim().length <= 40;
  return (
    <Modal
      open={open}
      onClose={onClose}
      size="sm"
      icon="sliders"
      title={title}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button variant="primary" disabled={!valid} loading={busy} onClick={() => onSave(name)}>
            {t("profileSave")}
          </Button>
        </>
      }
    >
      <label className="flex flex-col gap-1.5 text-sm text-fg">
        {t("profileName")}
        <input
          autoFocus
          type="text"
          value={name}
          maxLength={40}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && valid && onSave(name)}
          className="h-9 text-sm"
          placeholder={t("profileNamePlaceholder")}
        />
      </label>
      <p className="mt-2 text-xs text-fg-subtle">{t("profileSaveHint")}</p>
    </Modal>
  );
}
