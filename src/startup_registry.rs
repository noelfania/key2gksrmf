// Windows 시작 프로그램 레지스트리 등록/해제 모듈
// HKCU\Software\Microsoft\Windows\CurrentVersion\Run 에 "key2gksrmf" 값을 관리한다

use windows::core::PCWSTR;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};

use crate::app_info;

// 레지스트리 값이 존재하지 않을 때 반환되는 오류 코드
const ERROR_FILE_NOT_FOUND: WIN32_ERROR = WIN32_ERROR(2);

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const LEGACY_APP_NAME: &str = "gksrmf";

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 현재 레지스트리에 등록되어 있는지 조회한다
pub fn is_registered() -> bool {
    let key_wide = to_wide(RUN_KEY);
    let name_wide = to_wide(app_info::APP_NAME);
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
    // 디버그 빌드는 사용자의 실제 시작 프로그램 등록을 건드리지 않는다
    if cfg!(debug_assertions) {
        return true;
    }
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let exe_str = match exe_path.to_str() {
        Some(s) => s.to_string(),
        None => return false,
    };

    let key_wide = to_wide(RUN_KEY);
    let name_wide = to_wide(app_info::APP_NAME);
    // 공백이 있는 경로도 정확히 실행되도록 따옴표로 감싼다
    let val_wide = to_wide(&format!("\"{exe_str}\""));
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
    if cfg!(debug_assertions) {
        return true;
    }
    let key_wide = to_wide(RUN_KEY);
    let name_wide = to_wide(app_info::APP_NAME);
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

fn unregister_legacy_app_name() {
    let key_wide = to_wide(RUN_KEY);
    let name_wide = to_wide(LEGACY_APP_NAME);
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
            let _ = RegDeleteValueW(hkey, PCWSTR(name_wide.as_ptr()));
            let _ = RegCloseKey(hkey);
        }
    }
}

/// 앱 시작 시 레지스트리와 설정값의 정합성을 맞춘다
pub fn sync_on_startup(want_startup: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    unregister_legacy_app_name();
    if want_startup {
        // exe를 옮겼거나 이전 형식(따옴표 없음)으로 등록돼 있어도 현재 경로로 갱신
        register();
    } else if is_registered() {
        unregister();
    }
}
