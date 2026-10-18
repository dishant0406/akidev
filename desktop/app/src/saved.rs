//! Playing a saved file: mpv reads it through an `akc://` address, and each read is decrypted as it
//! goes, so the plain video never exists on disk.

use akidev_library::container::Sealed;
use libmpv2_sys::*;
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const PROTOCOL: &CStr = c"akc";

fn open_files() -> &'static Mutex<HashMap<String, Arc<Sealed>>> {
    static OPEN: OnceLock<Mutex<HashMap<String, Arc<Sealed>>>> = OnceLock::new();
    OPEN.get_or_init(Default::default)
}

/// A saved file made available to mpv until this is dropped.
pub struct Opened {
    address: String,
}

impl Opened {
    pub fn new(sealed: Sealed) -> Opened {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let address = format!("akc://{}", NEXT.fetch_add(1, Ordering::SeqCst));
        open_files()
            .lock()
            .unwrap()
            .insert(address.clone(), Arc::new(sealed));
        Opened { address }
    }

    /// What to give mpv to play.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        open_files().lock().unwrap().remove(&self.address);
    }
}

/// Teaches an mpv handle the `akc://` addresses. Called once for each handle, after it starts.
///
/// # Safety
/// `handle` must be a valid, initialised mpv handle.
pub unsafe fn register(handle: *mut mpv_handle) {
    let result = unsafe {
        mpv_stream_cb_add_ro(handle, PROTOCOL.as_ptr(), std::ptr::null_mut(), Some(open))
    };
    if result < 0 {
        tracing::warn!("[saved] mpv would not take the akc protocol ({result})");
    }
}

/// One open stream: the file and where the reader is in it.
struct Reader {
    file: Arc<Sealed>,
    at: u64,
}

unsafe extern "C" fn open(
    _user: *mut c_void,
    uri: *mut c_char,
    info: *mut mpv_stream_cb_info,
) -> c_int {
    let address = unsafe { CStr::from_ptr(uri) }
        .to_string_lossy()
        .into_owned();
    let Some(file) = open_files().lock().unwrap().get(&address).cloned() else {
        return mpv_error_MPV_ERROR_LOADING_FAILED;
    };
    let reader = Box::new(Reader { file, at: 0 });
    unsafe {
        (*info).cookie = Box::into_raw(reader).cast();
        (*info).read_fn = Some(read);
        (*info).seek_fn = Some(seek);
        (*info).size_fn = Some(size);
        (*info).close_fn = Some(close);
        (*info).cancel_fn = None;
    }
    0
}

unsafe extern "C" fn read(cookie: *mut c_void, buf: *mut c_char, bytes: u64) -> i64 {
    let reader = unsafe { &mut *cookie.cast::<Reader>() };
    let out = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), bytes as usize) };
    // A panic must not unwind into mpv.
    match catch_unwind(AssertUnwindSafe(|| reader.file.read_at(reader.at, out))) {
        Ok(Ok(read)) => {
            reader.at += read as u64;
            read as i64
        }
        _ => -1,
    }
}

unsafe extern "C" fn seek(cookie: *mut c_void, offset: i64) -> i64 {
    let reader = unsafe { &mut *cookie.cast::<Reader>() };
    if offset < 0 {
        return -1;
    }
    reader.at = (offset as u64).min(reader.file.len());
    reader.at as i64
}

unsafe extern "C" fn size(cookie: *mut c_void) -> i64 {
    unsafe { &*cookie.cast::<Reader>() }.file.len() as i64
}

unsafe extern "C" fn close(cookie: *mut c_void) {
    drop(unsafe { Box::from_raw(cookie.cast::<Reader>()) });
}

#[cfg(test)]
mod tests {
    use super::*;
    use akidev_library::container::{MasterKey, seal};
    use std::ffi::CString;
    use std::process::Command;

    fn option(handle: *mut mpv_handle, name: &str, value: &str) {
        let (name, value) = (CString::new(name).unwrap(), CString::new(value).unwrap());
        assert!(unsafe { mpv_set_option_string(handle, name.as_ptr(), value.as_ptr()) } >= 0);
    }

    fn number(handle: *mut mpv_handle, name: &str) -> Option<f64> {
        let name = CString::new(name).unwrap();
        let mut value = 0f64;
        let status = unsafe {
            mpv_get_property(
                handle,
                name.as_ptr(),
                mpv_format_MPV_FORMAT_DOUBLE,
                (&mut value as *mut f64).cast(),
            )
        };
        (status >= 0).then_some(value)
    }

    /// mpv, given nothing but an `akc://` address, plays a sealed film from the start to the end,
    /// and finds its length, through the decrypting reader.
    #[test]
    fn mpv_plays_a_sealed_file_through_the_akc_address() {
        let dir = std::env::temp_dir().join(format!("akidev-akc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("film.mp4");
        let made = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=3:size=160x120:rate=10",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=duration=3",
                "-c:v",
                "mpeg4",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&plain)
            .status();
        if !made.is_ok_and(|status| status.success()) {
            eprintln!("ffmpeg is not here, so there is no film to play");
            return;
        }
        let master = MasterKey([9; 32]);
        let sealed_path = dir.join("film.akc");
        seal(&plain, &sealed_path, &master).unwrap();
        // Nothing the player can open without the library's key.
        std::fs::remove_file(&plain).unwrap();
        let opened = Opened::new(Sealed::open(&sealed_path, &master).unwrap());

        unsafe {
            let handle = mpv_create();
            for (name, value) in [
                ("vo", "null"),
                ("ao", "null"),
                ("config", "no"),
                ("load-scripts", "no"),
            ] {
                option(handle, name, value);
            }
            assert!(mpv_initialize(handle) >= 0);
            register(handle);
            let address = CString::new(opened.address()).unwrap();
            let mut command = [c"loadfile".as_ptr(), address.as_ptr(), std::ptr::null()];
            assert!(mpv_command(handle, command.as_mut_ptr()) >= 0);

            let (mut loaded, mut ended) = (false, None);
            let mut length = None;
            for _ in 0..80 {
                let event = &*mpv_wait_event(handle, 0.25);
                if event.event_id == mpv_event_id_MPV_EVENT_FILE_LOADED {
                    loaded = true;
                    length = number(handle, "duration");
                }
                if event.event_id == mpv_event_id_MPV_EVENT_END_FILE {
                    ended = Some((*(event.data as *const mpv_event_end_file)).reason);
                    break;
                }
            }
            mpv_terminate_destroy(handle);
            assert!(loaded, "mpv did not load the sealed file");
            let length = length.expect("mpv did not report a length");
            assert!((2.5..3.5).contains(&length), "the length was {length}");
            assert_eq!(
                ended,
                Some(mpv_end_file_reason_MPV_END_FILE_REASON_EOF),
                "it did not play to the end"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
