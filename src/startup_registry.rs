// Windows 시작 프로그램 레지스트리 등록/해제 모듈
// HKCU\Software\Microsoft\Windows\CurrentVersion\Run 에 "gksrmf" 값을 관리한다

use windows::core::PCWSTR;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};

// 레지스트리 값이 존재하지 않을 때 반환되는 오류 코드
const ERROR_FILE_NOT_FOUND: WIN32_ERROR = WIN32_ERROR(2);

const APP_NAME: &str = "gksrmf";
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 현재 레지스트리에 등록되어 있는지 조회한다
pub fn is_registered() -> bool {
    let key_wide = to_wide(RUN_KEY);
    let name_wide = to_wide(APP_NAME);
    unsafe {
        let mut hkey = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_ok()
        {
            let result = RegQueryValueExW(
                hkey,
                PCWSTR(name_wide.as_ptr()),
                None,
                None,
                None,
                None,
            );
            let _ = RegCloseKey(hkey);
            result.is_ok()
        } else {
            false
        }
    }
}

/// 레지스트리에 현재 실행 파일 경로로 시작 프로그램을 등록한다
pub fn register() -> bool {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let exe_str = match exe_path.to_str() {
        Some(s) => s.to_string(),
        None => return false,
    };

    let key_wide = to_wide(RUN_KEY);
    let name_wide = to_wide(APP_NAME);
    let val_wide = to_wide(&exe_str);
    // REG_SZ 값은 u8 바이트 슬라이스로 전달
    let val_bytes: Vec<u8> = val_wide
        .iter()
        .flat_map(|&w| w.to_le_bytes())
        .collect();

    unsafe {
        let mut hkey = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_wide.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
        .is_ok()
        {
            let result = RegSetValueExW(
                hkey,
                PCWSTR(name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(&val_bytes),
            );
            let _ = RegCloseKey(hkey);
            result.is_ok()
        } else {
            false
        }
    }
}

/// 레지스트리에서 시작 프로그램 등록을 해제한다
pub fn unregister() -> bool {
    let key_wide = to_wide(RUN_KEY);
    let name_wide = to_wide(APP_NAME);
    unsafe {
        let mut hkey = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_wide.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
        .is_ok()
        {
            let result = RegDeleteValueW(hkey, PCWSTR(name_wide.as_ptr()));
            let _ = RegCloseKey(hkey);
            // 이미 없는 경우(ERROR_FILE_NOT_FOUND)도 성공으로 처리
            result.is_ok() || result == ERROR_FILE_NOT_FOUND
        } else {
            false
        }
    }
}

/// 앱 시작 시 레지스트리와 설정값의 정합성을 맞춘다
pub fn sync_on_startup(want_startup: bool) {
    let actual = is_registered();
    if want_startup && !actual {
        register();
    } else if !want_startup && actual {
        unregister();
    }
}
