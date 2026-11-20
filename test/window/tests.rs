#[cfg(windows)]
#[test]
fn files_dragged_out_reach_other_programs_as_a_list_of_files() {
    use std::{ffi::c_void, fs, os::windows::ffi::OsStringExt, ptr::null_mut};
    use windows_sys::Win32::{
        System::{
            Com::{
                COINIT_APARTMENTTHREADED, CoInitializeEx, DVASPECT_CONTENT, FORMATETC, STGMEDIUM,
                TYMED_HGLOBAL,
            },
            Ole::{CF_HDROP, ReleaseStgMedium},
        },
        UI::Shell::{DragQueryFileW, HDROP},
    };
    // Two files of different folders, one of them with the `\?\` of canonical paths.
    let base = std::env::temp_dir().join(format!("atelier-drag-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join("a/b")).unwrap();
    fs::write(base.join("a/one.txt"), "1").unwrap();
    fs::write(base.join("a/b/two.txt"), "2").unwrap();
    let one = base.join("a/one.txt").canonicalize().unwrap();
    let two = crate::project::simplify(&base.join("a/b/two.txt").canonicalize().unwrap());
    unsafe {
        let _ = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32);
        let object = super::imp::data_object(&[one.clone(), two.clone()]).expect("data object");
        // What a program that receives the drop asks: the list of files.
        let format = FORMATETC {
            cfFormat: CF_HDROP,
            ptd: null_mut(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL as u32,
        };
        let mut medium: STGMEDIUM = std::mem::zeroed();
        type GetData =
            unsafe extern "system" fn(*mut c_void, *const FORMATETC, *mut STGMEDIUM) -> i32;
        let methods = *(object as *const *const usize);
        let get_data: GetData = std::mem::transmute(*methods.add(3));
        assert!(get_data(object, &format, &mut medium) >= 0);
        let drop = medium.u.hGlobal as HDROP;
        let count = DragQueryFileW(drop, u32::MAX, null_mut(), 0);
        let files: Vec<std::path::PathBuf> = (0..count)
            .map(|i| {
                let mut name = vec![0u16; 1024];
                let len = DragQueryFileW(drop, i, name.as_mut_ptr(), name.len() as u32);
                std::ffi::OsString::from_wide(&name[..len as usize]).into()
            })
            .collect();
        ReleaseStgMedium(&mut medium);
        super::imp::release(object);
        assert_eq!(files, [crate::project::simplify(&one), two]);
    }
    let _ = fs::remove_dir_all(&base);
}
