#[cfg(target_os = "android")]
use crate::platform::{file::emit_selected_file, SelectedMedia};
use jni::{
    objects::{JByteArray, JClass, JObject, JObjectArray, JString},
    JNIEnv,
};
use std::sync::atomic::{AtomicI32, Ordering};
use tracing::{debug, error, info};
use std::fs::OpenOptions;
use std::io::Write;

// A no_mangle static byte string so the marker appears in the compiled .so strings.
// This helps verify the native library includes our instrumentation.
#[no_mangle]
#[used]
pub static NATIVE_ECHO_MARKER: [u8; 12] = *b"native-echo\0";

#[no_mangle]
pub extern "C" fn get_native_echo_marker() -> *const u8 {
    NATIVE_ECHO_MARKER.as_ptr() as *const u8
}


#[link(name = "log")]
extern "C" {
    fn __android_log_write(prio: ::std::os::raw::c_int, tag: *const ::std::os::raw::c_char, text: *const ::std::os::raw::c_char) -> ::std::os::raw::c_int;
}


pub fn get_jvm() -> jni::JavaVM {
    unsafe { jni::JavaVM::from_raw(ndk_context::android_context().vm().cast()) }.unwrap()
}

/// Log a message to Android logcat at DEBUG level using android.util.Log.d
pub fn android_log_d(tag: &str, msg: &str) {
    use std::ffi::CString;

    let fallback_write = |t: &str, m: &str| {
        let c_tag = CString::new(t).unwrap_or_else(|_| CString::new("tag").unwrap());
        let c_msg = CString::new(m).unwrap_or_else(|_| CString::new("msg").unwrap());
        unsafe {
            let _ = __android_log_write(3, c_tag.as_ptr(), c_msg.as_ptr());
        }
        // Also try to persist logs to the app external files directory so they can be
        // retrieved with `adb pull` if logcat entries are missing.
        let _ = (|| -> std::io::Result<()> {
            let dir = "/sdcard/Android/data/com.damus.notedeck/files";
            std::fs::create_dir_all(dir)?;
            let path = format!("{}/notedeck_native.log", dir);
            let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
            writeln!(file, "[{}] {}", t, m)?;
            Ok(())
        })().map_err(|e| eprintln!("android_log_d: failed to write fallback log file: {:?}", e));
    };

    // Try to attach to the JVM and call android.util.Log.d(TAG, MSG)
    let vm = get_jvm();
    let attach_res = vm.attach_current_thread();
    match attach_res {
        Ok(mut env) => {
            use jni::objects::JValue;
            match (env.new_string(tag), env.new_string(msg), env.find_class("android/util/Log")) {
                (Ok(tag_j), Ok(msg_j), Ok(class)) => {
                    let tag_obj = JObject::from(tag_j);
                    let msg_obj = JObject::from(msg_j);
                    let args = [JValue::Object(&tag_obj), JValue::Object(&msg_obj)];
                    if let Err(e) = env.call_static_method(
                        class,
                        "d",
                        "(Ljava/lang/String;Ljava/lang/String;)I",
                        &args,
                    ) {
                        // Fallback to native log writer so logs still appear in logcat
                        eprintln!("android_log_d: call_static_method failed: {:?}", e);
                        fallback_write(tag, msg);
                    }
                }
                (t_res, m_res, c_res) => {
                    eprintln!("android_log_d: failed to prepare JNI args: tag_err={:?} msg_err={:?} class_err={:?}", t_res.err(), m_res.err(), c_res.err());
                    fallback_write(tag, msg);
                }
            }
        }
        Err(e) => {
            eprintln!("android_log_d: attach_current_thread failed: {:?}", e);
            fallback_write(tag, msg);
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_damus_notedeck_MainActivity_nativeEcho(mut env: JNIEnv, _class: JClass, jmsg: JString) {
    // Safely convert incoming Java string and forward to android_log_d
    match env.get_string(&jmsg) {
        Ok(jstr) => {
            let s: String = jstr.into();
            android_log_d("native-echo", &s);
        }
        Err(e) => {
            android_log_d("native-echo", &format!("failed to read jstring: {:?}", e));
        }
    }
}

// Thread-safe static global
static KEYBOARD_HEIGHT: AtomicI32 = AtomicI32::new(0);

/// This function is called by our main notedeck android activity when the
/// keyboard height changes. You can use [`virtual_keyboard_height`] to access
/// this
#[no_mangle]
pub extern "C" fn Java_com_damus_notedeck_KeyboardHeightHelper_nativeKeyboardHeightChanged(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    height: jni::sys::jint,
) {
    debug!("updating virtual keyboard height {}", height);

    // Convert and store atomically
    KEYBOARD_HEIGHT.store(height.max(0), Ordering::SeqCst);
}

/// Gets the current Android virtual keyboard height. Useful for transforming
/// the view
pub fn virtual_keyboard_height() -> i32 {
    KEYBOARD_HEIGHT.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn Java_com_damus_notedeck_MainActivity_nativeOnFilePickedFailed(
    mut env: JNIEnv,
    _class: JClass,
    juri: JString,
    je: JString,
) {
    let _uri: String = env.get_string(&juri).unwrap().into();
    let _error: String = env.get_string(&je).unwrap().into();
}

#[no_mangle]
pub extern "C" fn Java_com_damus_notedeck_MainActivity_nativeOnFilePickedWithContent(
    mut env: JNIEnv,
    _class: JClass,
    // [display_name, size, mime_type]
    juri_info: JObjectArray,
    jcontent: JByteArray,
) {
    debug!("File picked with content");

    let display_name: Option<String> = {
        let obj = env.get_object_array_element(&juri_info, 0).unwrap();
        if obj.is_null() {
            None
        } else {
            Some(env.get_string(&JString::from(obj)).unwrap().into())
        }
    };

    if let Some(display_name) = display_name {
        let length = env.get_array_length(&jcontent).unwrap() as usize;
        let mut content: Vec<i8> = vec![0; length];
        env.get_byte_array_region(&jcontent, 0, &mut content)
            .unwrap();

        debug!("selected file: {display_name:?} ({length:?} bytes)",);

        emit_selected_file(SelectedMedia::from_bytes(
            display_name,
            content.into_iter().map(|b| b as u8).collect(),
        ));
    } else {
        error!("Received null file name");
    }
}

pub fn vibrate(duration_ms: i64) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let vm = get_jvm();
    let mut env = vm.attach_current_thread()?;
    let context = unsafe { JObject::from_raw(ndk_context::android_context().context().cast()) };
    env.call_method(
        context,
        "vibrate",
        "(J)V",
        &[jni::objects::JValue::Long(duration_ms)],
    )?;
    Ok(())
}

pub fn try_vibrate() {
    match vibrate(200) {
        Ok(()) => {
            info!("Vibration triggered");
        }
        Err(e) => {
            error!("Failed to vibrate: {}", e);
        }
    }
}

pub fn try_open_file_picker() {
    match open_file_picker() {
        Ok(()) => {
            info!("File picker opened successfully");
        }
        Err(e) => {
            error!("Failed to open file picker: {}", e);
        }
    }
}

pub fn open_file_picker() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Get the Java VM from AndroidApp
    let vm = get_jvm();

    // Attach current thread to get JNI environment
    let mut env = vm.attach_current_thread()?;

    let context = unsafe { JObject::from_raw(ndk_context::android_context().context().cast()) };
    // Call the openFilePicker method on the MainActivity
    env.call_method(
        context,
        "openFilePicker",
        "()V", // Method signature: no parameters, void return
        &[],   // No arguments
    )?;

    Ok(())
}

#[no_mangle]
pub extern "C" fn JNI_OnLoad(_vm: *mut jni::sys::JavaVM, _reserved: *mut std::os::raw::c_void) -> jni::sys::jint {
    use std::ffi::CString;
    // Write a small native log entry so we can verify the .so was loaded even if
    // JNI attach or android.util.Log.d calls fail. __android_log_write should be
    // available on Android platforms via liblog.
    unsafe {
        let c_tag = CString::new("JNI").unwrap_or_else(|_| CString::new("JNI").unwrap());
        let c_msg = CString::new("JNI_OnLoad called").unwrap_or_else(|_| CString::new("loaded").unwrap());
        let _ = __android_log_write(3, c_tag.as_ptr(), c_msg.as_ptr());
    }

    // Also attempt to write a short record into the app external files area for
    // offline inspection via adb if logcat isn't showing the message.
    let _ = (|| -> std::io::Result<()> {
        let dir = "/sdcard/Android/data/com.damus.notedeck/files";
        std::fs::create_dir_all(dir)?;
        let path = format!("{}/notedeck_native.log", dir);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(file, "[JNI] JNI_OnLoad called")?;
        Ok(())
    })().map_err(|e| eprintln!("JNI_OnLoad: failed to write fallback log file: {:?}", e));

    jni::sys::JNI_VERSION_1_6
}

