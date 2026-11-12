import { Card } from "@/components/common/Card";
import { MetricValue } from "@/components/common/MetricValue";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { NotReported, Table, Td } from "@/components/common/Table";
import { useT } from "@/i18n";
import { formatBytes } from "@/lib/format";
import { busLabel, mediaLabel } from "@/features/monitoring/labels";
import type { Metric } from "@/types/generated/Metric";
import type { PhysicalDiskInfo } from "@/types/generated/PhysicalDiskInfo";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";

/**
 * Disks and volumes as installed. Volumes are listed by mount point as the OS reports it: drive
 * letters on Windows (`C:\`), POSIX paths on macOS (`/`, `/Volumes/…`); the column header says
 * which. Activity, free space and the OS disk status are on Overview.
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
    <Card title={t("storageTitle")}>
      <h3 className="eyebrow mb-2.5">{t("physicalDisks")}</h3>
      <DisksTable disks={disks} />
      <h3 className="eyebrow mt-5 mb-2.5">{t("volumes")}</h3>
      <VolumesTable volumes={volumes} />
    </Card>
  );
}

function DisksTable({ disks }: { disks: Metric<PhysicalDiskInfo[]> }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  if (disks.status === "unavailable") {
    return <MetricValue metric={disks} format={() => ""} className="text-sm" />;
  }
  if (disks.value.length === 0) {
    return <StateView compact kind="empty" title={t("noDisks")} />;
  }
  return (
    <>
      <Table caption={t("physicalDisks")} headers={[t("model"), t("bus"), t("media"), t("capacity"), t("firmware")]}>
        {disks.value.map((d) => (
          <tr key={d.id}>
            <Td>{d.model ?? tm("diskFallback", { id: d.id })}</Td>
            <Td>{busLabel(d.bus, tm)}</Td>
            <Td>{mediaLabel(d.media, tm)}</Td>
            <Td>{formatBytes(d.sizeBytes)}</Td>
            <Td className="font-mono text-xs">{d.firmware ?? <NotReported />}</Td>
          </tr>
        ))}
      </Table>
      <p className="mt-2 empty:hidden">
        <SourceCaption metric={disks} />
      </p>
    </>
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
          <Td>{formatBytes(v.totalBytes)}</Td>
          <Td>{v.removable ? t("yes") : t("no")}</Td>
        </tr>
      ))}
    </Table>
  );
}
