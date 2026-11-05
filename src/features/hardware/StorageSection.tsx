import { memo } from "react";
import { Card } from "@/components/common/Card";
import { DefinitionList } from "@/components/common/DefinitionList";
import { MetricValue } from "@/components/common/MetricValue";
import { SourceCaption } from "@/components/common/SourceCaption";
import { StateView } from "@/components/common/StateView";
import { NotReported, Table, Td } from "@/components/common/Table";
import { UsageBar } from "@/components/common/UsageBar";
import { useT } from "@/i18n";
import { formatBytes, formatBytesPerSec, formatPercent } from "@/lib/format";
import { DiskHealth } from "@/features/monitoring/DiskHealth";
import { busLabel, mediaLabel } from "@/features/monitoring/labels";
import { useLive } from "@/features/monitoring/liveStore";
import { DefinitionSkeleton } from "@/features/monitoring/StatusViews";
import type { Metric } from "@/types/generated/Metric";
import type { PhysicalDiskInfo } from "@/types/generated/PhysicalDiskInfo";
import type { VolumeInfo } from "@/types/generated/VolumeInfo";
import { LiveSlot, SubHeading } from "./LiveSlot";
import { metricItem } from "./metricItem";

const rate = (v: number) => formatBytesPerSec(v);
const pct = (v: number) => formatPercent(v);

export function StorageSection({
  disks,
  volumes,
}: {
  disks: Metric<PhysicalDiskInfo[]>;
  volumes: readonly VolumeInfo[];
}) {
  const t = useT("hardware");
  return (
    <Card title="Storage">
      <SubHeading>{t("storageLive")}</SubHeading>
      <LiveSlot placeholder={<DefinitionSkeleton rows={3} />}>
        <StorageLive />
      </LiveSlot>

      <div className="mt-6">
        <SubHeading>{t("physicalDisks")}</SubHeading>
        <DisksTable disks={disks} />
      </div>

      <div className="mt-6">
        <SubHeading>{t("volumes")}</SubHeading>
        <VolumesTable volumes={volumes} />
      </div>
    </Card>
  );
}

const StorageLive = memo(function StorageLive() {
  const t = useT("hardware");
  const tm = useT("monitoring");
  const storage = useLive((s) => s.latest?.storage);
  if (!storage) return null;
  return (
    <DefinitionList
      items={[
        metricItem(tm("read"), storage.readBytesPerSec, rate),
        metricItem(tm("write"), storage.writeBytesPerSec, rate),
        metricItem(t("activeTime"), storage.activeTimePercent, pct),
      ]}
    />
  );
});

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
      <Table
        caption={t("physicalDisks")}
        headers={[t("model"), t("bus"), t("media"), t("capacity"), t("firmware"), tm("windowsHealth")]}
      >
        {disks.value.map((d) => (
          <tr key={d.id}>
            <Td>{d.model ?? tm("diskFallback", { id: d.id })}</Td>
            <Td>{busLabel(d.bus, tm)}</Td>
            <Td>{mediaLabel(d.media, tm)}</Td>
            <Td>{formatBytes(d.sizeBytes)}</Td>
            <Td className="font-mono text-xs">{d.firmware ?? <NotReported />}</Td>
            <Td>
              <DiskHealth health={d.health} />
            </Td>
          </tr>
        ))}
      </Table>
      <p className="mt-2 text-xs text-fg-subtle">
        {t("healthNote")} <SourceCaption metric={disks} />
      </p>
    </>
  );
}

function VolumesTable({ volumes }: { volumes: readonly VolumeInfo[] }) {
  const t = useT("hardware");
  const tm = useT("monitoring");
  if (volumes.length === 0) return <StateView compact kind="empty" title={tm("noVolumes")} />;
  return (
    <Table
      caption={t("volumes")}
      headers={[t("drive"), t("label"), t("fileSystem"), t("occupancy"), t("removable")]}
    >
      {volumes.map((v) => {
        const used = v.totalBytes - v.availableBytes;
        const percent = v.totalBytes > 0 ? (used / v.totalBytes) * 100 : null;
        const summary = t("freeOfTotal", { free: formatBytes(v.availableBytes), total: formatBytes(v.totalBytes) });
        return (
          <tr key={v.mountPoint}>
            <Td className="font-mono text-xs">{v.mountPoint}</Td>
            <Td>{v.label || <NotReported />}</Td>
            <Td>{v.fileSystem ?? <NotReported />}</Td>
            <Td className="min-w-48">
              <div className="mb-1.5 text-xs tabular">{summary}</div>
              <UsageBar
                label={tm("usedSpaceOn", { mount: v.mountPoint })}
                percent={percent}
                valueText={
                  percent !== null ? t("usedWithSummary", { percent: formatPercent(percent), summary }) : summary
                }
              />
            </Td>
            <Td>{v.removable ? t("yes") : t("no")}</Td>
          </tr>
        );
      })}
    </Table>
  );
}
