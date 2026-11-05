# Driver verification (phase 4)

How the app builds the device and driver inventory and how it checks for driver updates.
Everything here is **read-only**. The app never installs, removes, rolls back, enables or
disables a device or a driver. It never downloads or runs anything, and it needs no admin
rights.

Code: `crates/drivers` (mapping, matching, assembly) and `crates/platform`
(`devices.rs`, `windows/{devices,pnp_wmi,wua}.rs`).

## Public API (`drivers`)

```rust
pub fn collect_inventory() -> Result<DriverInventory, AppError>;            // local, no network
pub fn check_updates(inventory: &DriverInventory) -> Result<DriverInventory, AppError>; // network
```

The caller handles consent (`allowOnlineChecks`) and concurrency. If `check_updates` fails,
it returns `Err` and the caller keeps the previous inventory.

## Inventory

1. **Enumeration** uses `SetupDiGetClassDevsW(DIGCF_PRESENT | DIGCF_ALLCLASSES)` and
   `SetupDiEnumDeviceInfo`. Only present devices are included. The device info set is
   destroyed in a RAII guard. For each device:
   - Instance id: `SetupDiGetDeviceInstanceIdW`.
   - Properties, read with `SetupDiGetDevicePropertyW`:
     - `DEVPKEY_Device_FriendlyName` and `DeviceDesc`
     - `Manufacturer` and `Class`
     - `HardwareIds` (a string list in multi-sz form, most specific first)
     - `DriverVersion`, `DriverProvider` and `DriverInfPath`
     - `DriverDate`: a FILETIME, converted to an ISO date. Driver dates are stored as
       midnight UTC of the INF `DriverVer` date.
   - Class GUID: taken from `SP_DEVINFO_DATA`.
   - Status and problem code: `CM_Get_DevNode_Status`, which reports `DN_HAS_PROBLEM`,
     `DN_STARTED` and `CM_PROB_*`.
2. **Signatures** come from one WMI query:
   `SELECT DeviceID, IsSigned, Signer, DriverVersion, DriverDate, DriverProviderName, InfName FROM Win32_PnPSignedDriver`.
   - The query runs on its own thread (MTA via the `wmi` crate) with a 60 s timeout, in
     parallel with step 1.
   - Rows are joined on `DeviceID` = instance id, case-insensitive.
   - SetupAPI values take priority. WMI values only fill gaps.
   - If WMI fails, every `signed` is `Unavailable` with the real reason (`PermissionDenied`
     or `Failed(AppError)`). All other data is still returned.
   - If a device has a driver but no WMI row, `signed` is
     `Unavailable(Failed(NotFound))`.
   - If the row has `IsSigned = NULL`, `signed` is `Unavailable(NotSupported)`.
3. **`driver`** is set when a version, provider or INF is known. Devices without a driver
   (for example Code 28) have `driver: None`.
4. **`is_inbox`** is true when the provider equals `"Microsoft"` (case-insensitive). A
   generic Microsoft driver is a normal state. It never affects health and never means
   "missing".
5. **Name** is the friendly name, then the device description, then the instance id. The
   instance id is the fallback for devices with no driver and no description.
6. **Sort order:** health (DriverMissing, ProblemDetected, Disabled, Working), then
   category (in enum order), then name (case-insensitive), then instance id.

### Health mapping

The only input is the CfgMgr32 problem code. Driver age and provider play no part.

| Condition | Health |
|---|---|
| No `DN_HAS_PROBLEM` | `Working` (as Device Manager does, even when `DN_STARTED` is not set) |
| `CM_PROB_DISABLED` (22) | `Disabled` |
| `CM_PROB_NOT_CONFIGURED` (1), `CM_PROB_REINSTALL` (18), `CM_PROB_FAILED_INSTALL` (28) | `DriverMissing` |
| Any other code (e.g. 10, 31, 37, 39, 43, 48, 52, 29) | `ProblemDetected` |
| `CM_Get_DevNode_Status` failed | `ProblemDetected`, no code, with the failure as the description |

- `DriverMissing` is used only for codes that mean no driver is installed or selected (1,
  28), or that the installation is incomplete (18).
- Code 39 ("driver may be corrupted or missing") and code 31 ("cannot load the drivers")
  are load failures of a driver that is already associated with the device, so they map to
  `ProblemDetected`.
- Code 29 (disabled by firmware) is not a user or policy choice, so it maps to
  `ProblemDetected`.

`problem_description` comes from a fixed table that follows Device Manager's wording, for
example `"Code 43: Windows has stopped this device because it has reported problems."`.
The table covers codes 1–57. Any other code gets `"Code N: Windows reports a problem with
this device."`. Unit tests check that every code from 1 to 57 has exactly one entry.

### Category mapping

The setup class name is the primary key. INF class names are not localized. The class
GUID (from `devguid.h`) is the fallback.

| Class | Category |
|---|---|
| Display | Gpu (this includes virtual display adapters, e.g. Parsec) |
| System, Firmware | Chipset |
| MEDIA, AudioEndpoint | Audio |
| Net | Wifi / Ethernet / Other (see below) |
| Bluetooth | Bluetooth |
| SCSIAdapter, HDC | StorageController |
| DiskDrive | Disk |
| USB, USBDevice | Usb |
| HIDClass, Keyboard, Mouse | Input |
| Monitor | Display |
| anything else (Processor, SoftwareDevice, SoftwareComponent, Volume, …) | Other |

**Net heuristic.**

- An adapter counts as physical only when its instance-id enumerator is `PCI`, `USB`, `SD`,
  `SDIO` or `ACPI`.
- Every other enumerator is treated as virtual and maps to `Other`. That includes `ROOT`,
  `SWD`, `BTH`, `VMBUS` and software `{…}` enumerators, so it covers WAN miniports,
  Hyper-V, VPN/TAP adapters, Wi-Fi Direct virtual adapters and Bluetooth PAN.
- A physical adapter is `Wifi` when its name or a hardware id contains one of: `wi-fi`,
  `wifi`, `wireless`, `802.11`, `wlan`, `wi fi`, `airport`. Otherwise it is `Ethernet`.

## Windows Update check

1. On a dedicated thread, COM is initialized as MTA (`CoInitializeEx`). Then:
   1. `CoCreateInstance(UpdateSession)`
   2. `IUpdateSession::CreateUpdateSearcher`
   3. `SetOnline(true)`
   4. `Search("IsInstalled=0 and Type='Driver'")`

   The module never creates `IUpdateDownloader` or `IUpdateInstaller`.
2. Each result is cast to `IWindowsDriverUpdate`. The app reads `Title`,
   `DriverHardwareID`, `DriverProvider`, `DriverManufacturer`, `DriverModel`,
   `DriverClass` and `DriverVerDate` (an OLE `DATE`, converted to an ISO date).
3. The overall timeout is 5 minutes. On timeout the function returns `Platform`
   (`retriable = true`). The synchronous `Search` cannot be aborted, so the worker thread
   finishes on its own.
4. `E_ACCESSDENIED` is reported as `PermissionDenied`. Every other failure, including
   network and `WU_E_*` errors and `orcFailed`/`orcAborted`, is reported as `Platform` with
   the HRESULT in `detail`.

### Matching rules

- An update applies to a device only when its `DriverHardwareID` equals one of the
  device's hardware ids. The comparison is case-insensitive and exact: no prefix or fuzzy
  matching.
- If several offers match one device, the offer for the device's most specific hardware
  id wins. Devices that share a hardware id all get the update.
- `UpdateAvailable` carries:
  - `title`.
  - `date`, from `DriverVerDate`.
  - `hardware_id`: the device's matched id.
  - `version`, but only when the title ends in `" - <dotted numeric version>"` or
    `"(<dotted numeric version>)"`. Windows
    Update has no version field, so the app never guesses one.
- Devices with no matching offer get `NotVerifiable { reason: "Windows Update non offre
  aggiornamenti per questo dispositivo: non conferma che la versione installata sia la più
  recente." }`.
- Devices with no hardware ids get a separate `NotVerifiable` reason.
- **`Verified` is never produced.** When Windows Update offers nothing, that does not
  prove the installed driver is current. `Verified` is reserved for future vendor catalogs
  with documented criteria.
- Versions are never compared across vendors, and a driver's date never makes it
  "outdated". Many current inbox drivers are dated 2006-06-21 on purpose.
- `update_check` records `checked_at`, `source`, `duration_ms`, `offered` (the number of
  driver updates returned) and `error: None`.

## Limitations

- Windows Update only knows about drivers that the vendor publishes to it. On WSUS-managed
  machines the search goes to the configured server. Many devices will therefore be
  `NotVerifiable`, and that is the honest answer.
- WU sometimes offers a driver whose date is older than the installed one, because the
  vendor publishes several branches. The app reports what WU offers and does not judge it.
- `Win32_PnPSignedDriver` takes about a second or more. It is the main cost of
  `collect_inventory`.
- Only present devices are listed. Phantom (disconnected) devices are out of scope.
