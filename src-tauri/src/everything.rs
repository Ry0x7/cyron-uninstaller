//! Read-only search against the local Everything (voidtools) index.
//!
//! Everything exposes a documented local IPC protocol over a hidden window.
//! This module speaks that protocol directly with `WM_COPYDATA` QUERY2W
//! messages. It never loads the Everything SDK DLL, never starts or stops
//! Everything, never changes its index or settings, and never uses the
//! Everything HTTP/ETP network servers.
//!
//! Protocol constants and structure layouts follow the Everything SDK
//! (`Everything_IPC.h`, MIT, Copyright (C) 2022 David Carpenter).
//! Every reply field is treated as untrusted display data: sizes are bounded,
//! strings are length-checked, and no result path is ever executed here.

use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use std::time::Instant;

use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    FindWindowW, GetMessageW, KillTimer, RegisterClassW, SendMessageTimeoutW, SetTimer,
    TranslateMessage, MSG, MSGFLT_ALLOW, SMTO_ABORTIFHUNG, SMTO_BLOCK, WM_COPYDATA, WM_TIMER,
    WM_USER, WNDCLASSW,
};

const EVERYTHING_WINDOW_CLASS: &str = "EVERYTHING_TASKBAR_NOTIFICATION";
const CYRON_REPLY_WINDOW_CLASS: &str = "CyronEverythingIpcReply";
const REPLY_MESSAGE_ID: u32 = 0x4359_524F;
const QUERY_TIMEOUT_MS: u32 = 5_000;
const IPC_TIMEOUT_MS: u32 = 2_000;
const REPLY_TIMER_ID: usize = 1;
const MAX_REPLY_BYTES: usize = 32 * 1024 * 1024;
const MAX_STRING_UTF16: usize = 32_768;
const QUERY2_HEADER_SIZE: usize = 28;
const LIST2_HEADER_SIZE: usize = 20;
const ITEM2_SIZE: usize = 8;

const IPC_GET_MAJOR_VERSION: u32 = 0;
const IPC_GET_MINOR_VERSION: u32 = 1;
const IPC_GET_REVISION: u32 = 2;
const IPC_GET_BUILD_NUMBER: u32 = 3;
const IPC_IS_DB_LOADED: u32 = 401;
const IPC_IS_DB_BUSY: u32 = 402;

const COPYDATA_QUERY2W: usize = 18;
const SORT_NAME_ASCENDING: u32 = 1;

const REQUEST_NAME: u32 = 0x0000_0001;
const REQUEST_PATH: u32 = 0x0000_0002;
const REQUEST_FULL_PATH_AND_NAME: u32 = 0x0000_0004;
const REQUEST_SIZE: u32 = 0x0000_0010;
const REQUEST_DATE_MODIFIED: u32 = 0x0000_0040;
const QUERY2_REQUEST_FLAGS: u32 =
    REQUEST_NAME | REQUEST_PATH | REQUEST_FULL_PATH_AND_NAME | REQUEST_SIZE | REQUEST_DATE_MODIFIED;

const ITEM_FOLDER: u32 = 0x0000_0001;

/// Search text is limited so a single IPC query stays small and predictable.
pub const MAX_QUERY_UTF16: usize = 256;
/// Largest page the UI may request.
pub const MAX_LIMIT: u32 = 200;
/// Default page size used when the frontend omits a limit.
pub const DEFAULT_LIMIT: u32 = 100;
/// Furthest result offset accepted, preventing pathological pagination requests.
pub const MAX_OFFSET: u32 = 1_000_000;

static QUERY_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static REPLY: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

thread_local! {
    static REPLY_MESSAGE: Cell<u32> = const { Cell::new(0) };
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverythingStatus {
    pub running: bool,
    pub available: bool,
    pub version: Option<String>,
    pub db_loaded: bool,
    pub db_busy: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverythingItem {
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub is_folder: bool,
    pub size: u64,
    pub modified: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverythingPage {
    pub items: Vec<EverythingItem>,
    pub total: u32,
    pub offset: u32,
    pub limit: u32,
    pub has_more: bool,
    pub elapsed_ms: u64,
}

impl EverythingStatus {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            running: false,
            available: false,
            version: None,
            db_loaded: false,
            db_busy: false,
            message: message.into(),
        }
    }
}

/// Checks the local Everything IPC endpoint without changing any state.
pub fn status() -> EverythingStatus {
    let Some(everything_hwnd) = find_everything_window() else {
        return EverythingStatus::unavailable(
            "Everything is not running. Start Everything 1.4.1 or newer to search its local index.",
        );
    };

    let Some((major, minor, revision, build)) = read_version(everything_hwnd) else {
        return EverythingStatus {
            running: true,
            available: false,
            version: None,
            db_loaded: false,
            db_busy: false,
            message: "Everything is running but did not answer Cyron's local IPC version check. Restart Everything and try again.".into(),
        };
    };

    let version = format!("{major}.{minor}.{revision}.{build}");
    if !ipc_version_supported(major, minor, revision) {
        return EverythingStatus {
            running: true,
            available: false,
            message: format!(
                "Everything {version} is running, but index search needs Everything 1.4.1 or newer."
            ),
            version: Some(version),
            db_loaded: false,
            db_busy: false,
        };
    }

    let db_loaded = send_ipc(everything_hwnd, IPC_IS_DB_LOADED, 0)
        .map(|value| value != 0)
        .unwrap_or(false);
    let db_busy = send_ipc(everything_hwnd, IPC_IS_DB_BUSY, 0)
        .map(|value| value != 0)
        .unwrap_or(false);

    let message = if !db_loaded {
        format!(
            "Everything {version} is running, but its index is still loading. Search may return no results until it finishes."
        )
    } else if db_busy {
        format!(
            "Everything {version} is running and updating its index. Results may be incomplete while it catches up."
        )
    } else {
        format!("Everything {version} is running. Cyron only reads the local index.")
    };

    EverythingStatus {
        running: true,
        available: true,
        version: Some(version),
        db_loaded,
        db_busy,
        message,
    }
}

/// Runs a bounded QUERY2W search against the running Everything client.
pub fn search(query: &str, offset: u32, limit: u32) -> Result<EverythingPage, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Enter a search term.".into());
    }
    if query.encode_utf16().count() > MAX_QUERY_UTF16 {
        return Err(format!(
            "Search terms are limited to {MAX_QUERY_UTF16} characters."
        ));
    }

    let everything_hwnd = find_everything_window().ok_or_else(|| {
        "Everything is not running. Start Everything 1.4.1 or newer to search its local index."
            .to_string()
    })?;
    let offset = offset.min(MAX_OFFSET);
    let limit = limit.clamp(1, MAX_LIMIT);

    let started = Instant::now();
    let mut page = query_window(everything_hwnd, query, offset, limit)?;
    page.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(page)
}

fn ipc_version_supported(major: u32, minor: u32, revision: u32) -> bool {
    (major, minor, revision) >= (1, 4, 1)
}

fn find_everything_window() -> Option<HWND> {
    let class = wide(EVERYTHING_WINDOW_CLASS);
    let hwnd = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
    (!hwnd.is_null()).then_some(hwnd)
}

fn read_version(everything_hwnd: HWND) -> Option<(u32, u32, u32, u32)> {
    let major = send_ipc(everything_hwnd, IPC_GET_MAJOR_VERSION, 0)?;
    let minor = send_ipc(everything_hwnd, IPC_GET_MINOR_VERSION, 0)?;
    let revision = send_ipc(everything_hwnd, IPC_GET_REVISION, 0)?;
    let build = send_ipc(everything_hwnd, IPC_GET_BUILD_NUMBER, 0)?;
    Some((
        major.max(0) as u32,
        minor.max(0) as u32,
        revision.max(0) as u32,
        build.max(0) as u32,
    ))
}

fn send_ipc(hwnd: HWND, command: u32, lparam: isize) -> Option<isize> {
    let mut result: usize = 0;
    let sent = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_USER,
            command as WPARAM,
            lparam,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            IPC_TIMEOUT_MS,
            &mut result,
        )
    };
    (sent != 0).then_some(result as isize)
}

fn query_window(
    everything_hwnd: HWND,
    query: &str,
    offset: u32,
    limit: u32,
) -> Result<EverythingPage, String> {
    // Everything documents one query per reply window. Serializing avoids two
    // UI searches cancelling each other and keeps each reply unambiguous.
    let _guard = QUERY_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let reply_hwnd = create_reply_window()?;
    let result = (|| {
        unsafe {
            // Everything may run with a different integrity level. Allow it to
            // post WM_COPYDATA replies to this dedicated hidden window only.
            ChangeWindowMessageFilterEx(
                reply_hwnd,
                WM_COPYDATA,
                MSGFLT_ALLOW,
                std::ptr::null_mut(),
            );
        }

        REPLY.with(|reply| *reply.borrow_mut() = None);
        REPLY_MESSAGE.with(|message| message.set(REPLY_MESSAGE_ID));

        let request = build_query(reply_hwnd, query, offset, limit);
        let mut copy_data = COPYDATASTRUCT {
            dwData: COPYDATA_QUERY2W,
            cbData: request.len() as u32,
            lpData: request.as_ptr() as *mut _,
        };
        let mut accepted: usize = 0;
        let sent = unsafe {
            SendMessageTimeoutW(
                everything_hwnd,
                WM_COPYDATA,
                reply_hwnd as WPARAM,
                &mut copy_data as *mut COPYDATASTRUCT as LPARAM,
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                IPC_TIMEOUT_MS,
                &mut accepted,
            )
        };
        if sent == 0 {
            return Err(
                "Everything did not accept the local search query. Make sure it is running and not busy."
                    .into(),
            );
        }

        let reply = wait_for_reply(reply_hwnd, QUERY_TIMEOUT_MS).ok_or_else(|| {
            "Everything did not return a result in time. Try a shorter query or restart Everything."
                .to_string()
        })?;
        let mut page = parse_reply(&reply, limit)?;
        page.offset = offset;
        page.limit = limit;
        page.has_more = offset.saturating_add(page.items.len() as u32) < page.total;
        Ok(page)
    })();
    unsafe {
        DestroyWindow(reply_hwnd);
    }
    result
}

fn create_reply_window() -> Result<HWND, String> {
    let class = wide(CYRON_REPLY_WINDOW_CLASS);
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let mut window_class: WNDCLASSW = unsafe { std::mem::zeroed() };
    window_class.lpfnWndProc = Some(reply_window_proc);
    window_class.hInstance = instance;
    window_class.lpszClassName = class.as_ptr();

    unsafe {
        // A second registration fails with ERROR_CLASS_ALREADY_EXISTS; the
        // existing process-local class is the same one, so creation is safe.
        RegisterClassW(&window_class);
    }

    let title = wide("Cyron Everything IPC");
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(format!(
            "Cyron could not create its local Everything IPC window (error {}).",
            unsafe { GetLastError() }
        ));
    }
    Ok(hwnd)
}

fn wait_for_reply(hwnd: HWND, timeout_ms: u32) -> Option<Vec<u8>> {
    let timer = unsafe { SetTimer(hwnd, REPLY_TIMER_ID, timeout_ms, None) };
    if timer == 0 {
        return None;
    }

    let reply = loop {
        if let Some(bytes) = take_reply() {
            break Some(bytes);
        }
        let mut message: MSG = unsafe { std::mem::zeroed() };
        let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break None;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if let Some(bytes) = take_reply() {
            break Some(bytes);
        }
        if message.message == WM_TIMER {
            break None;
        }
    };

    unsafe {
        KillTimer(hwnd, REPLY_TIMER_ID);
    }
    reply
}

fn take_reply() -> Option<Vec<u8>> {
    REPLY.with(|reply| reply.borrow_mut().take())
}

fn build_query(reply_hwnd: HWND, query: &str, offset: u32, limit: u32) -> Vec<u8> {
    let mut request = Vec::with_capacity(QUERY2_HEADER_SIZE + (query.len() + 1) * 2);
    for value in [
        reply_hwnd as usize as u32,
        REPLY_MESSAGE_ID,
        0,
        offset,
        limit,
        QUERY2_REQUEST_FLAGS,
        SORT_NAME_ASCENDING,
    ] {
        request.extend_from_slice(&value.to_le_bytes());
    }
    for unit in query.encode_utf16() {
        request.extend_from_slice(&unit.to_le_bytes());
    }
    request.extend_from_slice(&0u16.to_le_bytes());
    request
}

fn parse_reply(bytes: &[u8], requested_limit: u32) -> Result<EverythingPage, String> {
    if bytes.len() < LIST2_HEADER_SIZE {
        return Err("Everything returned a short result list.".into());
    }
    let total = read_u32(bytes, 0)?;
    let available = read_u32(bytes, 4)?;
    let offset = read_u32(bytes, 8)?;
    let request_flags = read_u32(bytes, 12)?;
    read_u32(bytes, 16)?; // sort type
    if request_flags & !QUERY2_REQUEST_FLAGS != 0 {
        return Err("Everything returned a result list with unsupported fields.".into());
    }

    let item_capacity = (bytes.len() - LIST2_HEADER_SIZE) / ITEM2_SIZE;
    let item_count = (available as usize)
        .min(item_capacity)
        .min(requested_limit as usize);
    let mut items = Vec::with_capacity(item_count);

    for index in 0..item_count {
        let table_offset = LIST2_HEADER_SIZE + index * ITEM2_SIZE;
        let flags = read_u32(bytes, table_offset)?;
        let data_offset = read_u32(bytes, table_offset + 4)? as usize;
        if data_offset < LIST2_HEADER_SIZE || data_offset > bytes.len() {
            return Err("Everything returned an item outside its result buffer.".into());
        }

        let mut cursor = data_offset;
        let name = read_optional_string(bytes, &mut cursor, request_flags, REQUEST_NAME)?;
        let path = read_optional_string(bytes, &mut cursor, request_flags, REQUEST_PATH)?;
        let reported_full_path = read_optional_string(
            bytes,
            &mut cursor,
            request_flags,
            REQUEST_FULL_PATH_AND_NAME,
        )?;
        let size = if request_flags & REQUEST_SIZE != 0 {
            let raw = read_u64(bytes, cursor)?;
            cursor += 8;
            raw
        } else {
            0
        };
        let modified = if request_flags & REQUEST_DATE_MODIFIED != 0 {
            filetime_to_unix_seconds(read_u64(bytes, cursor)?)
        } else {
            None
        };

        let full_path = if reported_full_path.is_empty() {
            join_path(&path, &name)
        } else {
            reported_full_path
        };
        items.push(EverythingItem {
            name,
            path,
            full_path,
            is_folder: flags & ITEM_FOLDER != 0,
            size,
            modified,
        });
    }

    let has_more = offset.saturating_add(item_count as u32) < total;
    Ok(EverythingPage {
        items,
        total,
        offset,
        limit: requested_limit,
        has_more,
        elapsed_ms: 0,
    })
}

fn read_optional_string(
    bytes: &[u8],
    cursor: &mut usize,
    request_flags: u32,
    flag: u32,
) -> Result<String, String> {
    if request_flags & flag == 0 {
        return Ok(String::new());
    }
    read_string(bytes, cursor)
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, String> {
    let length = read_u32(bytes, *cursor)? as usize;
    *cursor += 4;
    if length > MAX_STRING_UTF16 {
        return Err("Everything returned an oversized text field.".into());
    }
    let byte_length = length
        .checked_mul(2)
        .ok_or_else(|| "Everything returned an invalid text length.".to_string())?;
    let end = cursor
        .checked_add(byte_length)
        .ok_or_else(|| "Everything returned an invalid text length.".to_string())?;
    if end + 2 > bytes.len() {
        return Err("Everything returned a truncated text field.".into());
    }
    let mut units = Vec::with_capacity(length);
    for pair in bytes[*cursor..end].chunks_exact(2) {
        units.push(u16::from_le_bytes([pair[0], pair[1]]));
    }
    *cursor = end + 2; // skip the UTF-16 null terminator
    Ok(String::from_utf16_lossy(&units))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| "Everything returned an invalid result offset.".to_string())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "Everything returned a truncated result field.".to_string())?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "Everything returned an invalid result offset.".to_string())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "Everything returned a truncated result field.".to_string())?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn filetime_to_unix_seconds(filetime: u64) -> Option<u64> {
    const UNIX_EPOCH_FILETIME: u64 = 11_644_473_600 * 10_000_000;
    if filetime <= UNIX_EPOCH_FILETIME {
        return None;
    }
    Some((filetime - UNIX_EPOCH_FILETIME) / 10_000_000)
}

fn join_path(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else if path.ends_with('\\') || path.ends_with('/') {
        format!("{path}{name}")
    } else {
        format!("{path}\\{name}")
    }
}

unsafe extern "system" fn reply_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_COPYDATA {
        let copy_data = &*(lparam as *const COPYDATASTRUCT);
        let expected = REPLY_MESSAGE.with(|value| value.get());
        if copy_data.dwData as u32 == expected && !copy_data.lpData.is_null() {
            let length = (copy_data.cbData as usize).min(MAX_REPLY_BYTES);
            let bytes = std::slice::from_raw_parts(copy_data.lpData as *const u8, length).to_vec();
            REPLY.with(|reply| *reply.borrow_mut() = Some(bytes));
            return 1;
        }
        return 1;
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        PostMessageW, PostQuitMessage, SendMessageW, WM_APP, WM_CLOSE,
    };

    static FAKE_SERVER_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn query_layout_matches_query2w_header_and_utf16_terminator() {
        let request = build_query(0x1234 as HWND, "alpha", 7, 25);
        assert_eq!(request.len(), QUERY2_HEADER_SIZE + 6 * 2);
        assert_eq!(read_u32(&request, 0).unwrap(), 0x1234);
        assert_eq!(read_u32(&request, 4).unwrap(), REPLY_MESSAGE_ID);
        assert_eq!(read_u32(&request, 8).unwrap(), 0);
        assert_eq!(read_u32(&request, 12).unwrap(), 7);
        assert_eq!(read_u32(&request, 16).unwrap(), 25);
        assert_eq!(read_u32(&request, 20).unwrap(), QUERY2_REQUEST_FLAGS);
        assert_eq!(read_u32(&request, 24).unwrap(), SORT_NAME_ASCENDING);
        let text = &request[QUERY2_HEADER_SIZE..];
        assert_eq!(
            &text[..10],
            "alpha"
                .encode_utf16()
                .flat_map(|unit| unit.to_le_bytes())
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(&text[text.len() - 2..], &[0, 0]);
    }

    #[test]
    fn parses_file_and_folder_metadata_from_a_query2_reply() {
        let modified = (11_644_473_600u64 + 1_700_000_000) * 10_000_000;
        let reply = fake_list(
            3,
            0,
            &[
                FakeItem {
                    flags: 0,
                    name: "alpha.txt",
                    path: "C:\\Users\\Test",
                    full: "C:\\Users\\Test\\alpha.txt",
                    size: 1_234,
                    filetime: modified,
                },
                FakeItem {
                    flags: ITEM_FOLDER,
                    name: "Docs",
                    path: "C:\\Users\\Test",
                    full: "C:\\Users\\Test\\Docs",
                    size: 0,
                    filetime: 0,
                },
            ],
        );
        let page = parse_reply(&reply, 100).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 2);
        assert!(page.has_more);
        assert_eq!(page.items[0].full_path, "C:\\Users\\Test\\alpha.txt");
        assert_eq!(page.items[0].size, 1_234);
        assert_eq!(page.items[0].modified, Some(1_700_000_000));
        assert!(!page.items[0].is_folder);
        assert!(page.items[1].is_folder);
        assert_eq!(page.items[1].modified, None);
    }

    #[test]
    fn pagination_reports_more_only_when_the_total_exceeds_the_page() {
        let items = [FakeItem {
            flags: 0,
            name: "one.txt",
            path: "C:\\",
            full: "C:\\one.txt",
            size: 1,
            filetime: 0,
        }];
        let first = parse_reply(&fake_list(10, 0, &items), 5).unwrap();
        assert!(first.has_more);
        let last = parse_reply(&fake_list(10, 9, &items), 5).unwrap();
        assert!(!last.has_more);
    }

    #[test]
    fn falls_back_to_path_and_name_when_full_path_is_empty() {
        let reply = fake_list(
            1,
            1,
            &[FakeItem {
                flags: 0,
                name: "note.md",
                path: "D:\\Work\\",
                full: "",
                size: 4,
                filetime: 0,
            }],
        );
        let page = parse_reply(&reply, 10).unwrap();
        assert_eq!(page.items[0].full_path, "D:\\Work\\note.md");
        assert!(!page.has_more);
    }

    #[test]
    fn rejects_truncated_and_oversized_reply_fields() {
        assert!(parse_reply(&[0; 4], 10).is_err());
        let mut reply = fake_list(
            1,
            1,
            &[FakeItem {
                flags: 0,
                name: "x",
                path: "C:\\",
                full: "C:\\x",
                size: 1,
                filetime: 0,
            }],
        );
        reply.truncate(reply.len() - 3);
        assert!(parse_reply(&reply, 10).is_err());

        let mut request = vec![0u8; LIST2_HEADER_SIZE + ITEM2_SIZE + 4];
        write_u32(&mut request, 0, 1);
        write_u32(&mut request, 4, 1);
        write_u32(&mut request, 12, REQUEST_NAME);
        write_u32(&mut request, 20, 0);
        write_u32(
            &mut request,
            24,
            LIST2_HEADER_SIZE as u32 + ITEM2_SIZE as u32,
        );
        write_u32(&mut request, 28, MAX_STRING_UTF16 as u32 + 1);
        assert!(parse_reply(&request, 10).is_err());
    }

    #[test]
    fn version_gate_requires_everything_1_4_1() {
        assert!(!ipc_version_supported(1, 3, 4));
        assert!(!ipc_version_supported(1, 4, 0));
        assert!(ipc_version_supported(1, 4, 1));
        assert!(ipc_version_supported(1, 5, 0));
    }

    #[test]
    fn rejects_empty_and_oversized_queries_without_touching_ipc() {
        assert!(search("  ", 0, 10)
            .unwrap_err()
            .contains("Enter a search term"));
        let long = "x".repeat(MAX_QUERY_UTF16 + 1);
        assert!(search(&long, 0, 10).unwrap_err().contains("limited"));
    }

    #[test]
    fn fake_everything_server_round_trip() {
        let _guard = FAKE_SERVER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (server, handle) = spawn_fake_server();
        let page = query_window(server, "alpha", 0, 10).expect("round-trip query");
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].name, "alpha.txt");
        assert_eq!(page.items[0].full_path, "C:\\Users\\Test\\alpha.txt");
        assert_eq!(page.items[0].size, 1_234);
        assert_eq!(page.items[1].name, "Docs");
        assert!(page.items[1].is_folder);
        assert!(!page.has_more);
        unsafe {
            PostMessageW(server, WM_CLOSE, 0, 0);
        }
        handle.join().expect("fake server thread");
    }

    struct FakeItem {
        flags: u32,
        name: &'static str,
        path: &'static str,
        full: &'static str,
        size: u64,
        filetime: u64,
    }

    fn fake_list(total: u32, offset: u32, items: &[FakeItem]) -> Vec<u8> {
        let mut reply = vec![0u8; LIST2_HEADER_SIZE + items.len() * ITEM2_SIZE];
        write_u32(&mut reply, 0, total);
        write_u32(&mut reply, 4, items.len() as u32);
        write_u32(&mut reply, 8, offset);
        write_u32(&mut reply, 12, QUERY2_REQUEST_FLAGS);
        write_u32(&mut reply, 16, SORT_NAME_ASCENDING);

        for (index, item) in items.iter().enumerate() {
            let table = LIST2_HEADER_SIZE + index * ITEM2_SIZE;
            write_u32(&mut reply, table, item.flags);
            let data_offset = reply.len() as u32;
            write_u32(&mut reply, table + 4, data_offset);
            append_string(&mut reply, item.name);
            append_string(&mut reply, item.path);
            append_string(&mut reply, item.full);
            reply.extend_from_slice(&item.size.to_le_bytes());
            reply.extend_from_slice(&item.filetime.to_le_bytes());
        }
        reply
    }

    fn append_string(reply: &mut Vec<u8>, value: &str) {
        let units: Vec<u16> = value.encode_utf16().collect();
        reply.extend_from_slice(&(units.len() as u32).to_le_bytes());
        for unit in units {
            reply.extend_from_slice(&unit.to_le_bytes());
        }
        reply.extend_from_slice(&0u16.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    const FAKE_SERVER_CLASS: &str = "CyronFakeEverythingServer";

    // WM_COPYDATA is a synchronous-only message, so the fake server records
    // the reply target in its window procedure and sends the result after the
    // procedure returns, while the client is pumping its reply window.
    static FAKE_PENDING: Mutex<Vec<(usize, u32)>> = Mutex::new(Vec::new());

    unsafe extern "system" fn fake_server_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_CLOSE {
            DestroyWindow(hwnd);
            PostQuitMessage(0);
            return 0;
        }
        if message == WM_COPYDATA {
            let copy_data = &*(lparam as *const COPYDATASTRUCT);
            if copy_data.dwData == COPYDATA_QUERY2W
                && !copy_data.lpData.is_null()
                && copy_data.cbData as usize >= QUERY2_HEADER_SIZE
            {
                let request = std::slice::from_raw_parts(
                    copy_data.lpData as *const u8,
                    copy_data.cbData as usize,
                );
                let reply_hwnd = read_u32(request, 0).unwrap_or(0) as usize;
                let reply_id = read_u32(request, 4).unwrap_or(0);
                let mut pending = FAKE_PENDING
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                pending.push((reply_hwnd, reply_id));
                // Wake the server's message loop after this synchronous send
                // returns, so the reply can be sent while the client pumps.
                PostMessageW(hwnd, WM_APP, 0, 0);
                return 1;
            }
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }

    fn send_fake_replies(server: HWND) {
        let pending: Vec<(usize, u32)> = {
            let mut queue = FAKE_PENDING
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            queue.drain(..).collect()
        };

        for (reply_hwnd, reply_id) in pending {
            if reply_hwnd == 0 {
                continue;
            }
            let mut data = fake_list(
                2,
                0,
                &[
                    FakeItem {
                        flags: 0,
                        name: "alpha.txt",
                        path: "C:\\Users\\Test",
                        full: "C:\\Users\\Test\\alpha.txt",
                        size: 1_234,
                        filetime: 0,
                    },
                    FakeItem {
                        flags: ITEM_FOLDER,
                        name: "Docs",
                        path: "C:\\Users\\Test",
                        full: "C:\\Users\\Test\\Docs",
                        size: 0,
                        filetime: 0,
                    },
                ],
            );
            let mut copy_data = COPYDATASTRUCT {
                dwData: reply_id as usize,
                cbData: data.len() as u32,
                lpData: data.as_mut_ptr() as *mut _,
            };
            unsafe {
                SendMessageW(
                    reply_hwnd as HWND,
                    WM_COPYDATA,
                    server as WPARAM,
                    &mut copy_data as *mut COPYDATASTRUCT as LPARAM,
                );
            }
        }
    }

    fn spawn_fake_server() -> (HWND, thread::JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel::<Option<usize>>();
        let handle = thread::spawn(move || {
            let class = wide(FAKE_SERVER_CLASS);
            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let mut window_class: WNDCLASSW = unsafe { std::mem::zeroed() };
            window_class.lpfnWndProc = Some(fake_server_proc);
            window_class.hInstance = instance;
            window_class.lpszClassName = class.as_ptr();
            unsafe {
                RegisterClassW(&window_class);
            }
            let title = wide("Cyron fake Everything server");
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    title.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null(),
                )
            };
            if hwnd.is_null() {
                let _ = sender.send(None);
                return;
            }
            let _ = sender.send(Some(hwnd as usize));
            loop {
                let mut message: MSG = unsafe { std::mem::zeroed() };
                let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
                if result <= 0 {
                    break;
                }

                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                send_fake_replies(hwnd);
            }
        });
        let hwnd = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("fake server ready")
            .expect("fake server window") as HWND;
        (hwnd, handle)
    }
}
