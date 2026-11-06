//! Lightweight disposable Windows GUI target for external window targeting tests.
//! Creates a visible Win32 window with title "Cyron External Target App" and pumps messages.

use windows_sys::Win32::UI::WindowsAndMessaging::*;

fn main() {
    unsafe {
        let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let title_name: Vec<u16> = "Cyron External Target App\0".encode_utf16().collect();

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title_name.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            150,
            150,
            400,
            300,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );

        if hwnd.is_null() {
            eprintln!("Failed to create window");
            std::process::exit(1);
        }

        ShowWindow(hwnd, SW_SHOW);

        let pid = std::process::id();
        println!("TARGET_HWND:{:p} PID:{}", hwnd, pid);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
