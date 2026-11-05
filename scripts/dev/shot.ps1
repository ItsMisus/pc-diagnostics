param([string]$Out, [int]$ClickX = -1, [int]$ClickY = -1, [int]$Wheel = 0)
# Captures the PC Diagnostics window; optionally clicks a client-relative point first.
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class PdWin {
  public delegate bool P(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(P p, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
  public struct RECT { public int L, T, R, B; }
  public static IntPtr Find(uint target) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target) { var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
        if (sb.ToString() == "PC Diagnostics") { found = h; return false; } }
      return true; }, IntPtr.Zero);
    return found;
  }
}
"@
Add-Type -AssemblyName System.Drawing
$proc = Get-Process pc-diagnostics -ErrorAction Stop | Select-Object -First 1
$h = [PdWin]::Find([uint32]$proc.Id)
if ($h -eq [IntPtr]::Zero) { throw "window not found" }
[PdWin]::ShowWindow($h, 9) | Out-Null
[PdWin]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x0003) | Out-Null
[PdWin]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [PdWin]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[PdWin]::SetForegroundWindow($h) | Out-Null
$r = New-Object PdWin+RECT
[PdWin]::GetWindowRect($h, [ref]$r) | Out-Null
if ($ClickX -ge 0) {
  [PdWin]::SetForegroundWindow($h) | Out-Null
  Start-Sleep -Milliseconds 300
  [PdWin]::SetCursorPos($r.L + $ClickX, $r.T + $ClickY) | Out-Null
  [PdWin]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
  [PdWin]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 1500
}
if ($Wheel -ne 0) {
  [PdWin]::SetForegroundWindow($h) | Out-Null
  Start-Sleep -Milliseconds 200
  [PdWin]::SetCursorPos($r.L + 700, $r.T + 500) | Out-Null
  [PdWin]::mouse_event(0x0800, 0, 0, [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]$Wheel), 0), [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 800
}
$w = $r.R - $r.L; $hh = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc(); [PdWin]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc)
$bmp.Save($Out); $g.Dispose(); $bmp.Dispose()
"$w x $hh"
