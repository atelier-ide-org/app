//! The window on the desktop: where it opens, its taskbar button, and the processes of the
//! other project windows.
//!
//! eframe used to put the window back where it was last: a window closed while minimized came
//! back as a tiny one in the top left corner, and every window of the IDE shared that one
//! place. Now a window always opens centered on the monitor in use (the one under the mouse
//! pointer), at its default size or the most of that monitor it can take.

use std::{
    io,
    process::{Child, Command, Stdio},
};

/// Size of a new window, in points, before it is fitted to its monitor.
pub const DEFAULT_SIZE: [f32; 2] = [1440.0, 920.0];

/// Starts `command` as a process of its own: closing the window that started it, or the
/// console or job it belongs to, leaves it running.
pub fn spawn_detached(command: &mut Command) -> io::Result<Child> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        command.creation_flags(
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB,
        );
        if let Ok(child) = command.spawn() {
            return Ok(child);
        }
        // A job that does not let its processes leave (some terminals): the process stays in it.
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Out of the group of the terminal, which would take it down with it.
        command.process_group(0);
    }
    command.spawn()
}

thread_local! {
    /// The handle of the main window, for the native views placed in it (the Web viewer).
    static MAIN: std::cell::Cell<Option<raw_window_handle::RawWindowHandle>> =
        const { std::cell::Cell::new(None) };
}

/// Keeps the handle of the main window eframe created, on the thread of its interface.
pub fn remember_main(handle: &impl raw_window_handle::HasWindowHandle) {
    MAIN.set(handle.window_handle().ok().map(|h| h.as_raw()));
}

/// The main window, as a parent for native views: on the thread of the interface only.
pub fn main_window() -> Option<MainWindow> {
    MAIN.get().map(MainWindow)
}

/// The handle of the main window, borrowed while it lives (the whole run of the IDE).
#[derive(Clone, Copy)]
pub struct MainWindow(raw_window_handle::RawWindowHandle);

impl raw_window_handle::HasWindowHandle for MainWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        // The window outlives the views placed in it: they go with the plugins, before it.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.0) })
    }
}

/// The native window of the IDE.
pub struct Window {
    #[cfg(windows)]
    hwnd: usize,
    /// The monitor the taskbar button was last put on.
    #[cfg(windows)]
    monitor: usize,
    /// Another monitor the window is on, and since when.
    #[cfg(windows)]
    moved: Option<(usize, std::time::Instant)>,
}

/// How long the window stays on another monitor before its taskbar button follows: not
/// while it only passes over it.
#[cfg(windows)]
const SETTLE: std::time::Duration = std::time::Duration::from_millis(400);

impl Window {
    /// The window eframe created, from its handle.
    pub fn new(handle: &impl raw_window_handle::HasWindowHandle) -> Option<Self> {
        #[cfg(windows)]
        {
            let raw_window_handle::RawWindowHandle::Win32(handle) =
                handle.window_handle().ok()?.as_raw()
            else {
                return None;
            };
            let hwnd = handle.hwnd.get() as usize;
            imp::round_corners(hwnd);
            Some(Self {
                hwnd,
                monitor: imp::monitor(hwnd),
                moved: None,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = handle;
            None
        }
    }

    /// Centers the window on the monitor under the mouse pointer, at `size` points or the
    /// most of the monitor it can take. False when the system did not let it.
    pub fn center(&mut self, size: [f32; 2]) -> bool {
        #[cfg(windows)]
        {
            let placed = imp::center(self.hwnd, size);
            self.monitor = imp::monitor(self.hwnd);
            placed
        }
        #[cfg(not(windows))]
        {
            let _ = size;
            false
        }
    }

    /// Once the window has moved to another monitor, puts its taskbar button on the taskbar
    /// of that monitor: Windows does not always follow a window without a native title bar.
    /// The time to wait before looking again, while the window has just arrived there.
    pub fn follow_monitor(&mut self) -> Option<std::time::Duration> {
        #[cfg(windows)]
        {
            // Minimized, a window is nowhere: it keeps its monitor.
            if imp::minimized(self.hwnd) {
                return None;
            }
            let monitor = imp::monitor(self.hwnd);
            if monitor == 0 || monitor == self.monitor {
                self.moved = None;
                return None;
            }
            match self.moved {
                Some((to, since)) if to == monitor => {
                    let waited = since.elapsed();
                    if waited < SETTLE {
                        return Some(SETTLE - waited);
                    }
                    self.moved = None;
                    self.monitor = monitor;
                    imp::refresh_taskbar_button(self.hwnd);
                    None
                }
                _ => {
                    self.moved = Some((monitor, std::time::Instant::now()));
                    Some(SETTLE)
                }
            }
        }
        #[cfg(not(windows))]
        None
    }

    /// Hands `paths` to the system as a drag of files, which the desktop, a folder or a web
    /// page receives as copies. Returns once they are dropped or the drag is given up; false
    /// when the system cannot take them (or another system than Windows, for now).
    pub fn drag_files(&self, paths: &[std::path::PathBuf]) -> bool {
        #[cfg(windows)]
        {
            imp::drag_files(self.hwnd, paths)
        }
        #[cfg(not(windows))]
        {
            let _ = paths;
            false
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::{ffi::c_void, ptr::null_mut};
    use windows_sys::{
        Win32::{
            Foundation::{HWND, POINT},
            Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
            Graphics::Gdi::ScreenToClient,
            Graphics::Gdi::{
                GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
                MonitorFromPoint, MonitorFromWindow,
            },
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            },
            System::Ole::{DROPEFFECT, DROPEFFECT_COPY, OleInitialize},
            UI::{
                HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
                Shell::{
                    Common::ITEMIDLIST, ILCreateFromPathW, ILFree, SHCreateDataObject, SHDoDragDrop,
                },
                WindowsAndMessaging::{
                    GetCursorPos, IsIconic, PostMessageW, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
                    SetWindowPos, WM_LBUTTONUP, WM_MOUSEMOVE,
                },
            },
        },
        core::{GUID, HRESULT},
    };

    fn hwnd(hwnd: usize) -> HWND {
        hwnd as HWND
    }

    pub fn monitor(window: usize) -> usize {
        unsafe { MonitorFromWindow(hwnd(window), MONITOR_DEFAULTTONEAREST) as usize }
    }

    /// Rounded corners, as the windows of Windows 11 have: without its native title bar the
    /// window kept square ones. Windows squares them itself while the window is maximized or
    /// snapped; Windows 10 has no such corners and ignores the request.
    pub fn round_corners(window: usize) {
        let preference = DWMWCP_ROUND;
        unsafe {
            DwmSetWindowAttribute(
                hwnd(window),
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                (&raw const preference).cast(),
                size_of_val(&preference) as u32,
            );
        }
    }

    pub fn minimized(window: usize) -> bool {
        unsafe { IsIconic(hwnd(window)) != 0 }
    }

    pub fn center(window: usize, size: [f32; 2]) -> bool {
        unsafe {
            let mut cursor = POINT { x: 0, y: 0 };
            GetCursorPos(&mut cursor);
            let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) == 0 {
                return false;
            }
            let work = info.rcWork;
            let flags = SWP_NOZORDER | SWP_NOACTIVATE;
            // Onto the monitor first: a monitor of another scale changes the size of the window
            // as it arrives, and the size is chosen for that scale afterwards.
            SetWindowPos(
                hwnd(window),
                null_mut(),
                work.left + 32,
                work.top + 32,
                0,
                0,
                flags | SWP_NOSIZE,
            );
            let (mut dpi_x, mut dpi_y) = (96, 96);
            let scale = if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == 0
            {
                dpi_x as f32 / 96.0
            } else {
                1.0
            };
            let (area_w, area_h) = (work.right - work.left, work.bottom - work.top);
            let width = ((size[0] * scale) as i32).min(area_w * 9 / 10).max(1);
            let height = ((size[1] * scale) as i32).min(area_h * 9 / 10).max(1);
            let x = work.left + (area_w - width) / 2;
            let y = work.top + (area_h - height) / 2;
            SetWindowPos(hwnd(window), null_mut(), x, y, width, height, flags) != 0
        }
    }

    /// The part of `ITaskbarList` used here, in the order of its methods.
    #[repr(C)]
    struct TaskbarList {
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        hr_init: unsafe extern "system" fn(*mut c_void) -> HRESULT,
        add_tab: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
        delete_tab: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
    }

    const CLSID_TASKBAR_LIST: GUID = GUID::from_u128(0x56fdf344_fd6d_11d0_958a_006097c9a090);
    const IID_ITASKBAR_LIST: GUID = GUID::from_u128(0x56fdf342_fd6d_11d0_958a_006097c9a090);

    /// Takes the button of the window off the taskbar and puts it back: the taskbar then puts
    /// it on the monitor the window is on.
    pub fn refresh_taskbar_button(window: usize) {
        unsafe {
            // Already done by the window's drag and drop, most of the time: then it says so.
            let _ = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32);
            let mut list: *mut c_void = null_mut();
            if CoCreateInstance(
                &CLSID_TASKBAR_LIST,
                null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_ITASKBAR_LIST,
                &mut list,
            ) < 0
                || list.is_null()
            {
                return;
            }
            let methods = &**(list as *const *const TaskbarList);
            if (methods.hr_init)(list) >= 0 {
                (methods.delete_tab)(list, hwnd(window));
                (methods.add_tab)(list, hwnd(window));
            }
            (methods.release)(list);
        }
    }

    /// The data object of the shell for `paths`, as Explorer gives when its files are
    /// dragged or copied: it holds their list (`CF_HDROP`), their names and their places, so
    /// every program that receives files knows it. To be released with [`release`].
    pub fn data_object(paths: &[std::path::PathBuf]) -> Option<*mut c_void> {
        use std::os::windows::ffi::OsStrExt;
        let items: Vec<*mut ITEMIDLIST> = paths
            .iter()
            .filter_map(|path| {
                // Without the `\\?\` of canonical paths, which the shell does not read.
                let path = crate::project::simplify(path);
                let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
                let item = unsafe { ILCreateFromPathW(wide.as_ptr()) };
                (!item.is_null()).then_some(item)
            })
            .collect();
        let mut object = null_mut();
        if items.len() == paths.len() && !items.is_empty() {
            // Full lists of items are relative to the desktop, whose own list is empty: that
            // way the files may be in different folders.
            let desktop = [0u16];
            let list: Vec<*const ITEMIDLIST> = items.iter().map(|&i| i.cast_const()).collect();
            let made = unsafe {
                SHCreateDataObject(
                    desktop.as_ptr().cast(),
                    list.len() as u32,
                    list.as_ptr(),
                    null_mut(),
                    &IID_IDATA_OBJECT,
                    &mut object,
                )
            };
            if made < 0 {
                object = null_mut();
            }
        }
        for item in items {
            unsafe { ILFree(item) };
        }
        (!object.is_null()).then_some(object)
    }

    /// Releases a COM object: `Release` is the third method of every interface.
    pub unsafe fn release(object: *mut c_void) {
        type Release = unsafe extern "system" fn(*mut c_void) -> u32;
        unsafe {
            let methods = *(object as *const *const usize);
            let release: Release = std::mem::transmute(*methods.add(2));
            release(object);
        }
    }

    const IID_IDATA_OBJECT: GUID = GUID::from_u128(0x0000010e_0000_0000_c000_000000000046);

    pub fn drag_files(window: usize, paths: &[std::path::PathBuf]) -> bool {
        unsafe {
            // Done by winit for the files dropped on the window, most of the time.
            let _ = OleInitialize(null_mut());
            let Some(object) = data_object(paths) else {
                return false;
            };
            // The drag of the shell: its own pictures of the files under the pointer, and the
            // buttons and keys that drop or give up. Copies only: a project file dropped on
            // the desktop stays in the project.
            let mut effect: DROPEFFECT = 0;
            let done = SHDoDragDrop(
                hwnd(window),
                object,
                null_mut(),
                DROPEFFECT_COPY,
                &mut effect,
            );
            release(object);
            // The system took the release of the button: the window still thinks it held. The
            // pointer first, out of the window by now: egui drops a button without a pointer.
            let mut at = POINT { x: 0, y: 0 };
            GetCursorPos(&mut at);
            ScreenToClient(hwnd(window), &mut at);
            let position = (at.x as u16 as u32 | ((at.y as u16 as u32) << 16)) as isize;
            PostMessageW(hwnd(window), WM_MOUSEMOVE, 0, position);
            PostMessageW(hwnd(window), WM_LBUTTONUP, 0, position);
            done >= 0
        }
    }
}

#[cfg(test)]
#[path = "../test/window/tests.rs"]
mod tests;
