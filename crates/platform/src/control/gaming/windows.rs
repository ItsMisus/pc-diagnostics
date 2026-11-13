//! Windows registry reads for the game library (read only: `KEY_READ`, never a write).

use ::windows::Win32::Foundation::ERROR_SUCCESS;
use ::windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey,
    RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
};
use ::windows::core::{PCWSTR, PWSTR};

#[derive(Debug, Clone, Copy)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

impl Hive {
    fn hkey(self) -> HKEY {
        match self {
            Self::CurrentUser => HKEY_CURRENT_USER,
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A `REG_SZ` value, `None` when the key or value is missing.
pub fn reg_string(hive: Hive, subkey: &str, value: &str) -> Option<String> {
    let subkey_w = wide(subkey);
    let value_w = wide(value);
    let mut bytes: u32 = 0;
    // SAFETY: NUL-terminated strings that outlive the call; size query without a buffer.
    let status = unsafe {
        RegGetValueW(
            hive.hkey(),
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS || bytes < 2 {
        return None;
    }
    let mut buf = vec![0u16; usize::try_from(bytes).ok()? / 2 + 1];
    let mut size = u32::try_from(buf.len() * 2).ok()?;
    // SAFETY: `buf` holds `size` bytes and outlives the call.
    let status = unsafe {
        RegGetValueW(
            hive.hkey(),
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..len]);
    (!s.trim().is_empty()).then(|| s.trim().to_owned())
}

/// Names of the direct subkeys of `subkey`.
pub fn reg_subkeys(hive: Hive, subkey: &str) -> Vec<String> {
    let subkey_w = wide(subkey);
    let mut key = HKEY::default();
    // SAFETY: NUL-terminated string; `key` receives the handle, closed below.
    let status = unsafe {
        RegOpenKeyExW(
            hive.hkey(),
            PCWSTR(subkey_w.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut out = Vec::new();
    for index in 0..4096u32 {
        let mut name = [0u16; 256];
        let mut len = u32::try_from(name.len()).unwrap_or(256);
        // SAFETY: `name` holds `len` UTF-16 units and outlives the call.
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if status != ERROR_SUCCESS {
            break;
        }
        let len = usize::try_from(len).unwrap_or(0).min(name.len());
        out.push(String::from_utf16_lossy(&name[..len]));
    }
    // SAFETY: `key` was opened above.
    unsafe {
        let _ = RegCloseKey(key);
    }
    out
}

/// A GOG Galaxy install (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>`).
#[derive(Debug, Clone)]
pub struct GogGame {
    pub id: String,
    pub name: String,
    pub path: std::path::PathBuf,
    pub exe: std::path::PathBuf,
}

const GOG_GAMES: &str = "SOFTWARE\\WOW6432Node\\GOG.com\\Games";

/// Whether GOG has registered any install folder on this computer.
pub fn gog_present() -> bool {
    !reg_subkeys(Hive::LocalMachine, GOG_GAMES).is_empty()
}

pub fn gog_games() -> Vec<GogGame> {
    reg_subkeys(Hive::LocalMachine, GOG_GAMES)
        .into_iter()
        .filter_map(|id| {
            let key = format!("{GOG_GAMES}\\{id}");
            let name = reg_string(Hive::LocalMachine, &key, "gameName")?;
            let path = reg_string(Hive::LocalMachine, &key, "path")?;
            let exe = reg_string(Hive::LocalMachine, &key, "exe")?;
            let exe = std::path::PathBuf::from(exe);
            exe.is_file().then(|| GogGame {
                id,
                name,
                path: path.into(),
                exe,
            })
        })
        .collect()
}
