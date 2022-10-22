#![allow(non_snake_case)]

use std::{mem, ptr, iter, ffi::OsStr};
use std::os::windows::ffi::OsStrExt;

use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winnt::*;
use winapi::um::winuser::*;

use gml::vm;
use crate::Context;

pub struct Draw {
    pub hwnd: HWND,
}

impl Default for Draw {
    fn default() -> Draw {
        Draw {
            hwnd: ptr::null_mut(),
        }
    }
}

extern "C" {
    #[allow(improper_ctypes)]
    static __ImageBase: ();
}

pub fn run(mut cx: Context) { unsafe {
    let hInstance = &__ImageBase as *const _ as HINSTANCE;
    let nCmdShow = SW_SHOW;

    // This should really happen in a manifest, but Rust support for them is abysmal.
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

    let dejavu = OsStr::new("Dejavu");
    let dejavu: Vec<u16> = Iterator::chain(dejavu.encode_wide(), iter::once(0)).collect();

    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(WindowProc),
        hInstance: hInstance,
        hIcon: LoadIconW(ptr::null_mut(), IDI_APPLICATION),
        hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
        lpszClassName: dejavu.as_ptr(),
        hIconSm: ptr::null_mut(),
        ..mem::zeroed()
    };
    if RegisterClassExW(&wc) == 0 {
        panic!("failed to register window class");
    }

    let dwExStyle = WS_EX_APPWINDOW;
    let dwStyle = WS_OVERLAPPEDWINDOW & !WS_SIZEBOX & !WS_MAXIMIZEBOX;

    // Assume the window will be created on a display that matches the system DPI.
    // TODO: It may be necessary to readjust the size based on GetDpiForWindow().
    let dpi = GetDpiForSystem();
    let scale = f32::ceil(dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32);
    let mut rect = RECT {
        right: (scale * 640.0) as LONG,
        bottom: (scale * 480.0) as LONG,
        ..mem::zeroed()
    };
    AdjustWindowRectExForDpi(&mut rect, dwStyle, FALSE, dwExStyle, dpi);

    let hwnd = CreateWindowExW(
        dwExStyle, dejavu.as_ptr(), [b'\0' as u16].as_ptr(), dwStyle,
        CW_USEDEFAULT, CW_USEDEFAULT, rect.right - rect.left, rect.bottom - rect.top,
        ptr::null_mut(), ptr::null_mut(), hInstance, ptr::null_mut()
    );
    if hwnd == ptr::null_mut() {
        panic!("failed to create window");
    }

    let mut thread = vm::Thread::default();

    if let Err(error) = gml::vm::World::load(&mut cx, &mut thread) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    let Context { world, .. } = &mut cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { platform, .. } = draw;

    platform.hwnd = hwnd;
    crate::graphics::load(&mut cx);

    ShowWindow(hwnd, nCmdShow);

    let room = cx.assets.room_order[0] as i32;
    if let Err(error) = crate::room::State::load_room(&mut cx, &mut thread, room) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    'main: loop {
        if let Err(error) = crate::draw::State::draw(&mut cx, &mut thread) {
            let crate::World { show, .. } = &cx.world;
            show.show_vm_error(&*error);
        }
        crate::draw::State::animate(&mut cx);

        let mut msg: MSG = mem::zeroed();
        while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            if msg.message == WM_QUIT { break 'main; }
            DispatchMessageW(&msg);
        }

        if let Err(error) = crate::instance::State::step(&mut cx, &mut thread) {
            let crate::World { show, .. } = &cx.world;
            show.show_vm_error(&*error);
        }
        crate::motion::State::simulate(&mut cx);
    }
} }

unsafe extern "system" fn WindowProc(
    hwnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM
) -> LRESULT {
    match uMsg {
        WM_DESTROY => { PostQuitMessage(0); 0 }
        WM_DPICHANGED => {
            // TODO: This does not always match the integer factor selected to avoid ugly filtering.
            let RECT { left, top, right, bottom } = *(lParam as *const RECT);
            SetWindowPos(
                hwnd, ptr::null_mut(),
                left, top, right - left, bottom - top,
                SWP_NOACTIVATE | SWP_NOZORDER
            );
            0
        }
        _ => DefWindowProcW(hwnd, uMsg, wParam, lParam),
    }
}
