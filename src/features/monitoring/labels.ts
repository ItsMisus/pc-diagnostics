import { type Translator, translatorFor } from "@/i18n";
import type { BusKind } from "@/types/generated/BusKind";
import type { GpuVendor } from "@/types/generated/GpuVendor";
import type { MediaKind } from "@/types/generated/MediaKind";

export function gpuVendorLabel(
  vendor: GpuVendor,
  vendorId: number,
  t: Translator<"monitoring"> = translatorFor("monitoring"),
): string {
  switch (vendor) {
    case "nvidia":
      return "NVIDIA";
    case "amd":
      return "AMD";
    case "intel":
      return "Intel";
    case "microsoft":
      return "Microsoft";
    case "apple":
      return "Apple";
    case "other":
      // 0 is not a valid PCI vendor id: it means the OS reported none.
      if (vendorId === 0) return t("vendorNotReported");
      return t("vendorOther", { id: vendorId.toString(16).toUpperCase().padStart(4, "0") });
  }
}

export function busLabel(bus: BusKind, t: Translator<"monitoring"> = translatorFor("monitoring")): string {
  switch (bus) {
    case "nvme":
      return "NVMe";
    case "sata":
      return "SATA";
    case "usb":
      return "USB";
    case "sas":
      return "SAS";
    case "raid":
      return "RAID";
    case "virtual":
      return t("busVirtual");
    case "other":
      return t("busOther");
  }
}

export function mediaLabel(media: MediaKind, t: Translator<"monitoring"> = translatorFor("monitoring")): string {
  switch (media) {
    case "ssd":
      return "SSD";
    case "hdd":
      return "HDD";
    case "unknown":
      return t("mediaUnknown");
  }
}
