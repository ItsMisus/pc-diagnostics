import { m } from "motion/react";
import { memo, type MouseEvent, type ReactNode, useId, useMemo, useState } from "react";
import { Card } from "@/components/common/Card";
import { DefinitionList, type DefinitionItem } from "@/components/common/DefinitionList";
import { Disclosure } from "@/components/common/Disclosure";
import { Icon } from "@/components/common/Icon";
import { MetricValue } from "@/components/common/MetricValue";
import { StatusBadge } from "@/components/common/StatusBadge";
import { NotReported, Table, Td } from "@/components/common/Table";
import { useIsSimple } from "@/hooks/useViewMode";
import { type Translator, useT } from "@/i18n";
import { formatIsoDate } from "@/lib/format";
import { duration, ease, spring } from "@/lib/motion";
import type { DeviceCategory } from "@/types/generated/DeviceCategory";
import type { DeviceRecord } from "@/types/generated/DeviceRecord";
import type { UpdateStatus } from "@/types/generated/UpdateStatus";
import {
  CATEGORY_ORDER,
  categoryLabel,
  type DeviceFilter,
  filterCounts,
  filterDevices,
  filterLabel,
  groupDevices,
  healthLabel,
  updateLabel,
  updateSourceLabel,
} from "./model";
import { SearchField } from "@/components/common/SearchField";
import { Tag } from "@/components/common/Tag";
import {
  deviceProblemExplanation,
  type ProblemExplanation,
  simpleHealthLabel,
  simpleUpdateLabel,
  simpleDeviceName,
} from "./simple";

type T = Translator<"drivers">;

const FILTERS: readonly DeviceFilter[] = ["all", "problems", "updates", "notVerifiable"];
const HEADERS = ["colName", "colVersion", "colProvider", "colStatus", "colUpdate"] as const;
const SIMPLE_HEADERS = ["colName", "colStatus", "colUpdate"] as const;

/**
 * Header cell style, mirroring `Table`'s own header cells. The first column is inset by
 * 8 px (cells too) so the row tint never touches the chevron.
 */
const TH = "pr-4 pb-2 text-left text-2xs font-medium whitespace-nowrap text-fg-subtle";

function headCells(headers: readonly (typeof HEADERS)[number][], t: T) {
  return headers.map((h, i) => (
    <th key={h} scope="col" className={`${TH} ${i === 0 ? "pl-2" : ""}`}>
      {t(h)}
    </th>
  ));
}

/** Filterable, category-grouped device list. Filtering and grouping are memoized. */
export function DeviceList({ devices }: { devices: readonly DeviceRecord[] }) {
  const t = useT("drivers");
  const [filter, setFilter] = useState<DeviceFilter>("all");
  const [category, setCategory] = useState<DeviceCategory | null>(null);
  const [search, setSearch] = useState("");
  const simple = useIsSimple();

  const counts = useMemo(() => filterCounts(devices), [devices]);
  const presentCategories = useMemo(() => {
    const present = new Set(devices.map((d) => d.category));
    return CATEGORY_ORDER.filter((c) => present.has(c));
  }, [devices]);
  const groups = useMemo(
    () => groupDevices(filterDevices(devices, { filter, category, search }), t),
    [devices, filter, category, search, t],
  );

  return (
    // The reading notes live here, once: in "Semplice" the plain summary already says them.
    <Card title={t("devicesTitle")} {...(simple ? {} : { description: t("readingNote") })}>
      <div className="mb-4 flex flex-wrap items-center gap-3">
        {/* In "Semplice" the plain summary states the counts, so the tabs do not repeat them. */}
        <FilterGroup value={filter} counts={simple ? null : counts} onChange={setFilter} />
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <label>
            <span className="sr-only">{t("categorySelect")}</span>
            <select
              value={category ?? ""}
              onChange={(e) => setCategory(e.target.value === "" ? null : (e.target.value as DeviceCategory))}
              className="h-8 pl-2.5"
            >
              <option value="">{t("allCategories")}</option>
              {presentCategories.map((c) => (
                <option key={c} value={c}>
                  {categoryLabel(c, t)}
                </option>
              ))}
            </select>
          </label>
          <SearchField
            label={t("searchLabel")}
            value={search}
            onChange={setSearch}
            placeholder={t("searchPlaceholder")}
            className="w-52"
          />
        </div>
      </div>

      {groups.length === 0 ? (
        <p role="status" className="py-4 text-sm text-fg-muted">
          {t("noMatch")}
        </p>
      ) : (
        <div className="flex flex-col gap-6">
          {groups.map((g) => (
            <section key={g.category} aria-label={g.label}>
              <h3 className="mb-2 flex items-baseline gap-2 text-[13px] font-medium text-fg">
                {g.label} <span className="text-xs font-normal text-fg-subtle tabular">{g.devices.length}</span>
              </h3>
              <Table caption={t("groupCaption", { label: g.label })} head={headCells(simple ? SIMPLE_HEADERS : HEADERS, t)}>
                {g.devices.map((d) =>
                  simple ? <SimpleDeviceRow key={d.instanceId} device={d} /> : <DeviceRow key={d.instanceId} device={d} />,
                )}
              </Table>
            </section>
          ))}
        </div>
      )}
    </Card>
  );
}

/**
 * Status filter: toggle buttons (`aria-pressed`, exactly one pressed) drawn like the
 * `Segmented` control, with the same sliding thumb. The tabs are the page's one home of the
 * counts ("Avanzata"); they are tabular so the labels do not shift when the inventory changes.
 */
function FilterGroup({
  value,
  counts,
  onChange,
}: {
  value: DeviceFilter;
  /** null: labels only (the counts are stated elsewhere on the page). */
  counts: Record<DeviceFilter, number> | null;
  onChange: (filter: DeviceFilter) => void;
}) {
  const t = useT("drivers");
  const layoutId = `device-filter-${useId()}`;
  return (
    <div
      role="group"
      aria-label={t("filterGroupLabel")}
      className="inline-flex max-w-full flex-wrap rounded-lg border border-line bg-surface-2 p-0.5"
    >
      {FILTERS.map((f) => {
        const selected = value === f;
        return (
          <button
            key={f}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(f)}
            className={`relative h-7 rounded-md px-3 text-xs font-medium whitespace-nowrap transition-colors duration-150 ${
              selected ? "text-fg" : "text-fg-muted hover:text-fg"
            }`}
          >
            {selected && (
              <m.span
                layoutId={layoutId}
                transition={spring.indicator}
                aria-hidden="true"
                className="absolute inset-0 rounded-md border border-line-strong bg-surface-3"
              />
            )}
            <span className="relative">
              {filterLabel(f, t)}
              {counts && (
                <span className={`ml-1.5 tabular transition-colors duration-150 ${selected ? "text-fg-muted" : "text-fg-subtle"}`}>
                  {counts[f]}
                </span>
              )}
            </span>
          </button>
        );
      })}
    </div>
  );
}

/**
 * Row click toggles the detail (the name button is the keyboard and screen reader control;
 * its click bubbles to the row). A mouse click that ends a text selection does not toggle,
 * so versions and providers stay copyable.
 */
function useExpandable() {
  const [open, setOpen] = useState(false);
  const onRowClick = (e: MouseEvent<HTMLTableRowElement>) => {
    const selection = window.getSelection();
    if (e.detail > 0 && selection && !selection.isCollapsed && selection.toString().trim() !== "") return;
    setOpen((o) => !o);
  };
  return { open, onRowClick };
}

const ROW = "cursor-pointer transition-colors duration-150";
const rowTint = (open: boolean) => (open ? "bg-surface-2/50" : "hover:bg-surface-2/50");

/** Name cell control: chevron rotates a quarter turn when the row is expanded. */
function ExpandToggle({ open, controls, children }: { open: boolean; controls: string; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-expanded={open}
      aria-controls={controls}
      className="flex w-full min-w-0 items-start gap-1.5 rounded-sm text-left"
    >
      <Icon
        name="chevronRight"
        size={12}
        strokeWidth={1.75}
        className={`mt-1 shrink-0 text-fg-subtle transition-transform duration-200 ease-out ${open ? "rotate-90" : ""}`}
      />
      <span className="min-w-0 break-words">{children}</span>
    </button>
  );
}

/**
 * Expanded detail row, joined to its parent by the same tint. The content fades in once,
 * when the user opens it; data refreshes keep the row mounted, so nothing replays. The
 * left inset lines the content up with the device name (8 px cell inset + chevron + gap).
 */
function DetailRow({ id, colSpan, children }: { id: string; colSpan: number; children: ReactNode }) {
  return (
    <tr id={id}>
      <td colSpan={colSpan} className="bg-surface-2/50 pt-1 pr-4 pb-3 pl-6.5">
        <m.div
          initial={{ opacity: 0, y: -2 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: duration.component, ease: ease.out }}
        >
          {children}
        </m.div>
      </td>
    </tr>
  );
}


const DeviceRow = memo(function DeviceRow({ device }: { device: DeviceRecord }) {
  const t = useT("drivers");
  const { open, onRowClick } = useExpandable();
  const detailId = useId();
  const health = healthLabel(device.health, t);
  const update = updateLabel(device.update, t);
  const driver = device.driver;

  return (
    <>
      <tr onClick={onRowClick} className={`${ROW} ${rowTint(open)}`}>
        <Td className="w-[34%] max-w-0 pl-2">
          <ExpandToggle open={open} controls={detailId}>
            {device.name}
          </ExpandToggle>
        </Td>
        <Td className="whitespace-nowrap">{driver?.version ?? <NotReported />}</Td>
        <Td>
          {driver?.provider ? (
            <span className="flex flex-wrap items-center gap-1.5">
              <span>{driver.provider}</span>
              {driver.isInbox && (
                <Tag title={t("inboxTagTitle")}>{t("inboxTag")}</Tag>
              )}
            </span>
          ) : driver ? (
            <NotReported />
          ) : (
            <span className="text-fg-subtle">{t("noDriver")}</span>
          )}
        </Td>
        <Td className="whitespace-nowrap">
          <StatusBadge tone={health.tone} label={health.label} />
        </Td>
        <Td>
          <StatusBadge tone={update.tone} label={update.label} />
          {device.update.kind === "updateAvailable" && (
            <div className="mt-1 text-2xs text-fg-subtle">{device.update.title}</div>
          )}
        </Td>
      </tr>
      {open && (
        <DetailRow id={detailId} colSpan={HEADERS.length}>
          <DefinitionList items={detailItems(device, t)} className="text-xs" />
        </DetailRow>
      )}
    </>
  );
});

/** "Semplice": name, plain state, plain update status; explanation and technical data on expand. */
const SimpleDeviceRow = memo(function SimpleDeviceRow({ device }: { device: DeviceRecord }) {
  const t = useT("drivers");
  const { open, onRowClick } = useExpandable();
  const detailId = useId();
  const health = simpleHealthLabel(device.health, t);
  const update = simpleUpdateLabel(device.update, t);
  const explanation = deviceProblemExplanation(device, t);

  return (
    <>
      <tr onClick={onRowClick} className={`${ROW} ${rowTint(open)}`}>
        <Td className="w-[55%] max-w-0 pl-2">
          <ExpandToggle open={open} controls={detailId}>
            {simpleDeviceName(device, t)}
          </ExpandToggle>
        </Td>
        <Td className="whitespace-nowrap">
          <StatusBadge tone={health.tone} label={health.label} />
        </Td>
        <Td className="whitespace-nowrap">
          <StatusBadge tone={update.tone} label={update.label} />
        </Td>
      </tr>
      {open && (
        <DetailRow id={detailId} colSpan={SIMPLE_HEADERS.length}>
          {explanation ? (
            <ProblemExplanationView device={device} explanation={explanation} technical={false} />
          ) : (
            <p className="text-sm text-fg-muted">{t("simpleNoProblem")}</p>
          )}
          {device.update.kind === "updateAvailable" && (
            <p className="mt-2 text-sm text-fg-muted">
              {t.rich("simpleUpdateOffered", {
                arrow: (
                  <>
                    <Icon
                      name="chevronRight"
                      size={12}
                      strokeWidth={1.75}
                      className="mx-0.5 inline-block align-[-1px] text-fg-subtle"
                    />
                    <span className="sr-only">→</span>
                  </>
                ),
              })}
            </p>
          )}
          <Disclosure summary={t("showTechnical")} className="mt-3">
            <DefinitionList items={detailItems(device, t)} className="text-xs" />
          </Disclosure>
        </DetailRow>
      )}
    </>
  );
});

/**
 * Plain explanation of a device problem with safe steps to try. With `technical`, the
 * problem code, Windows' own description and the instance id sit behind a disclosure.
 */
export function ProblemExplanationView({
  device,
  explanation,
  technical = true,
}: {
  device: DeviceRecord;
  explanation: ProblemExplanation;
  technical?: boolean;
}) {
  const t = useT("drivers");
  return (
    <div className="mt-1 flex flex-col gap-3 text-sm">
      <p className="font-medium text-fg">{explanation.title}</p>
      <div>
        <h4 className="eyebrow mb-1">{t("whatItMeans")}</h4>
        <p>{explanation.meaning}</p>
      </div>
      <div>
        <h4 className="eyebrow mb-1">{t("whatYouCanDo")}</h4>
        <ol className="list-decimal space-y-0.5 pl-5 marker:text-fg-subtle">
          {explanation.actions.map((a) => (
            <li key={a}>{a}</li>
          ))}
        </ol>
      </div>
      {technical && (
        <Disclosure summary={t("showTechnical")}>
          <DefinitionList
            className="text-xs"
            items={[
              ...(device.problemCode !== null
                ? [
                    {
                      label: t("problemCode"),
                      value: problemText(device),
                    },
                  ]
                : []),
              { label: t("reportedState"), value: healthLabel(device.health, t).label },
              {
                label: t("instanceId"),
                value: <span className="font-mono break-all">{device.instanceId}</span>,
                caption: <span className="text-fg-subtle">{t("localInfoCaption")}</span>,
              },
            ]}
          />
        </Disclosure>
      )}
    </div>
  );
}

/** Windows' text already starts with the code ("Code 43: …"): show the code alone only without it. */
function problemText(d: DeviceRecord): string {
  return d.problemDescription ?? String(d.problemCode);
}

function detailItems(d: DeviceRecord, t: T): DefinitionItem[] {
  const driver = d.driver;
  const signedText = (v: boolean) => (v ? t("signed") : t("unsigned"));
  const items: DefinitionItem[] = [
    { label: t("deviceManufacturer"), value: d.manufacturer ?? <NotReported /> },
    { label: t("deviceClass"), value: d.className ?? <NotReported /> },
    {
      label: t("hardwareId"),
      value:
        d.hardwareIds.length > 0 ? (
          <span className="flex flex-col font-mono">
            {d.hardwareIds.map((id) => (
              <span key={id}>{id}</span>
            ))}
          </span>
        ) : (
          <NotReported />
        ),
    },
    {
      label: t("instanceId"),
      value: <span className="font-mono break-all">{d.instanceId}</span>,
      caption: <span className="text-fg-subtle">{t("localInfoCaption")}</span>,
    },
  ];

  if (driver) {
    items.push(
      {
        label: t("driverDate"),
        value: driver.date ? formatIsoDate(driver.date) : <NotReported />,
        ...(driver.date ? { caption: <span className="text-fg-subtle">{t("driverDateCaption")}</span> } : {}),
      },
      { label: t("infFile"), value: driver.infName ? <span className="font-mono">{driver.infName}</span> : <NotReported /> },
      {
        label: t("digitalSignature"),
        value: (
          <>
            <MetricValue metric={driver.signed} format={signedText} />
            {driver.signer && <span className="text-fg-muted"> · {driver.signer}</span>}
          </>
        ),
      },
    );
  } else {
    items.push({ label: t("driverLabel"), value: t("noDriverAssociated") });
  }

  if (d.problemCode !== null) {
    items.push({
      label: t("problemCode"),
      value: problemText(d),
    });
  }

  items.push({ label: t("colUpdate"), value: updateDetail(d.update, t) });
  return items;
}

function updateDetail(u: UpdateStatus, t: T) {
  switch (u.kind) {
    case "updateAvailable":
      return (
        <span className="flex flex-col">
          <span>
            {updateSourceLabel(u.source)}: {u.title}
          </span>
          <span className="text-fg-muted">
            {t("updateVersionDate", {
              version: u.version ?? t("notIndicated"),
              date: u.date ? formatIsoDate(u.date) : t("notIndicated"),
            })}
          </span>
          <span className="font-mono text-fg-muted">{t("matchingHardwareId", { id: u.hardwareId })}</span>
          <span className="text-fg-subtle">{t("installFromWindowsUpdate")}</span>
        </span>
      );
    case "verified":
      return `${updateSourceLabel(u.source)}: ${u.detail}`;
    case "notVerifiable":
      return t("notVerifiableReason", { reason: u.reason });
    case "notChecked":
      return t("notCheckedDetail");
  }
}
