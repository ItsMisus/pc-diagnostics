# Security

## Reporting a vulnerability

Please report security problems privately through GitHub: open the repository's
**Security** tab and choose **Report a vulnerability**. Don't open a public issue for them.

Include what an attacker could do, the steps to reproduce it and the app version. You
should get a first answer within a week.

## Scope

Things we especially want to hear about:

- Any way for the web interface to run commands, read files or call Windows or macOS APIs
  beyond the app's Tauri commands.
- Personal data leaving the computer, or appearing in a redacted report export.
- The optional "Risolvi" assistant: data sent without the user's consent, the API key
  leaking (it is stored in the system credential store: Windows Credential Manager; on
  macOS the Keychain), or an action running that is not on the built-in list or without
  its own confirmation.
- The Windows installer or uninstaller touching files outside its own folders, or the
  macOS app bundle asking for entitlements or access it does not need.

## Design limits

The app runs without admin rights, works offline, sends no telemetry and never changes
drivers, the registry or the BIOS. The only network use is the optional Windows Update
driver check and the optional assistant, both after explicit consent.

Release builds are not signed with a trusted certificate yet: the Windows installer is
unsigned and the macOS app is ad-hoc signed and not notarized. Check the SHA-256 published
next to each file on the Releases page before running it.
