# Security

## Reporting a vulnerability

Please report security problems privately through GitHub: open the repository's
**Security** tab and choose **Report a vulnerability**. Don't open a public issue for them.

Include what an attacker could do, the steps to reproduce it and the app version. You
should get a first answer within a week.

## Scope

Things we especially want to hear about:

- Any way for the web interface to run commands, read files or call Windows APIs beyond
  the app's Tauri commands.
- Personal data leaving the PC, or appearing in a redacted report export.
- The optional "Risolvi" assistant: data sent without the user's consent, the API key
  leaking (it is stored in Windows Credential Manager), or an action running that is not on
  the built-in list or without its own confirmation.
- The installer or uninstaller touching files outside its own folders.

## Design limits

The app runs without admin rights, works offline, sends no telemetry and never changes
drivers, the registry or the BIOS. The only network use is the optional Windows Update
driver check and the optional assistant, both after explicit consent.

Release installers are currently unsigned; check the SHA-256 published next to each
installer on the Releases page.
