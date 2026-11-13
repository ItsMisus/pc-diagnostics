import { DefinitionList } from "@/components/common/DefinitionList";
import { MetricValue } from "@/components/common/MetricValue";
import { Panel } from "@/components/common/Panel";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { NotReported, Table, Td } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatStorageBytes } from "@/lib/format";
import { busLabel, mediaLabel } from "@/features/monitoring/labels";
import type { Metric } from "@/types/generated/Metric";
import type { PhysicalDiskInfo } from "@/types/generated/PhysicalDiskInfo";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";

/**
 * Disks and volumes as installed. Each physical disk is a short spec sheet; volumes are a
 * table listed by mount point as the OS reports it: drive letters on Windows (`C:\`), POSIX
 * paths on macOS (`/`, `/Volumes/…`); the column header says which. Activity, free space
 * and the OS disk status are on Overview.
 */
export function StorageSection({
  disks,
  volumes,
}: {
  disks: Metric<PhysicalDiskInfo[]>;
  volumes: readonly VolumeInfo[];
}) {
  const t = useT("hardware");
  return (
    <Panel title={t("storageTitle")} icon="storage" level={3}>
      <h4 className="eyebrow mb-2">{t("physicalDisks")}</h4>
      <Disks disks={disks} />
      <h4 className="eyebrow mt-6 mb-2">{t("volumes")}</h4>
      <VolumesTable volumes={volumes} />
    </Panel>
  );
}

function Disks({ disks }: { disks: Metric<PhysicalDiskInfo[]> }) {
  const t = useT("hardware");
  if (disks.status === "unavailable") {
    return (
      <p className="text-sm">
        <MetricValue metric={disks} format={() => ""} />
      </p>
    );
  }
  if (disks.value.length === 0) {
    return <StateView compact kind="empty" title={t("noDisks")} />;
  }
  return (
    <>
      <div className="@container">
        <div className="grid grid-cols-1 gap-x-10 gap-y-5 @2xl:grid-cols-2 @4xl:grid-cols-3">
          {disks.value.map((d) => (
            <DiskBlock key={d.id} disk={d} />
          ))}
        </div>
      </div>
      <p className="mt-2 empty:hidden">
        <SourceCaption metric={disks} />
      </p>
    </>
  );
}

function DiskBlock({ disk }: { disk: PhysicalDiskInfo }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  const name = disk.model ?? tm("diskFallback", { id: disk.id });
  return (
    <article className="min-w-0" aria-label={name}>
      <h5 className="mb-2 min-w-0 text-sm leading-5 font-semibold break-words text-fg">{name}</h5>
      <DefinitionList
        divided
        items={[
          { label: t("capacity"), value: formatStorageBytes(disk.sizeBytes) },
          { label: t("bus"), value: busLabel(disk.bus, tm) },
          { label: t("media"), value: mediaLabel(disk.media, tm) },
          { label: t("firmware"), value: disk.firmware ? <span className="font-mono text-xs">{disk.firmware}</span> : <NotReported /> },
        ]}
      />
    </article>
  );
}

function VolumesTable({ volumes }: { volumes: readonly VolumeInfo[] }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  if (volumes.length === 0) return <StateView compact kind="empty" title={tm("noVolumes")} />;
  return (
    <Table caption={t("volumes")} headers={[t("drive"), t("label"), t("fileSystem"), t("capacity"), t("removable")]}>
      {volumes.map((v) => (
        <tr key={v.mountPoint}>
          <Td className="font-mono text-xs">{v.mountPoint}</Td>
          <Td>{v.label || <NotReported />}</Td>
          <Td>{v.fileSystem ?? <NotReported />}</Td>
          <Td>{formatStorageBytes(v.totalBytes)}</Td>
          <Td>{v.removable ? t("yes") : t("no")}</Td>
        </tr>
      ))}
    </Table>
  );
}
