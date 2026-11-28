// GeoPolit native core: JNI bridge for the Kotlin Android app.
pub mod app;
pub mod batch;
pub mod colors;
pub mod divs;
pub mod front;
pub mod labels;
pub mod peace;
pub mod render;
pub mod sim;
pub mod topo;
pub mod world;
pub mod zones;

use app::App;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jdouble, jfloat, jint, jlong, jstring};
use jni::JNIEnv;
use std::sync::{Arc, Mutex};

pub struct SendApp(pub App);
unsafe impl Send for SendApp {}

pub static APP: Mutex<Option<SendApp>> = Mutex::new(None);

// every JNI entry runs inside catch_unwind: a bug in one frame must never kill the app
pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    let mut g = APP.lock().unwrap_or_else(|e| e.into_inner());
    let a = g.as_mut()?;
    let app: &mut App = &mut a.0;
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(app))) {
        Ok(r) => Some(r),
        Err(_) => None,
    }
}

pub static STAGE: Mutex<String> = Mutex::new(String::new());
fn set_stage(s: &str) {
    *STAGE.lock().unwrap_or_else(|e| e.into_inner()) = s.to_string();
}

pub fn load_app(a: &world::Assets, fonts: Vec<Vec<u8>>) {
    let w = Arc::new(world::load(a, &|s| set_stage(s)));
    set_stage("Подготовка графики…");
    let app = App::new(w, fonts);
    *APP.lock().unwrap_or_else(|e| e.into_inner()) = Some(SendApp(app));
}

#[cfg(target_os = "android")]
mod gles {
    use std::ffi::{c_char, c_void, CString};
    extern "C" {
        fn dlopen(f: *const c_char, m: i32) -> *mut c_void;
        fn dlsym(h: *mut c_void, s: *const c_char) -> *mut c_void;
    }
    pub fn loader() -> impl FnMut(&str) -> *const c_void {
        let gles = unsafe { dlopen(b"libGLESv3.so\0".as_ptr() as *const c_char, 2) };
        let egl = unsafe { dlopen(b"libEGL.so\0".as_ptr() as *const c_char, 2) };
        let gpa: Option<extern "C" fn(*const c_char) -> *mut c_void> = unsafe {
            let p = dlsym(egl, b"eglGetProcAddress\0".as_ptr() as *const c_char);
            if p.is_null() {
                None
            } else {
                Some(std::mem::transmute(p))
            }
        };
        move |s: &str| {
            let c = CString::new(s).unwrap();
            let mut p = unsafe { dlsym(gles, c.as_ptr()) };
            if p.is_null() {
                if let Some(g) = gpa {
                    p = g(c.as_ptr());
                }
            }
            p as *const c_void
        }
    }
}

fn bytes(env: &JNIEnv, a: &JByteArray) -> Vec<u8> {
    env.convert_byte_array(a).unwrap_or_default()
}
fn jstr(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(|s| s.into()).unwrap_or_default()
}
fn ret(env: &mut JNIEnv, s: String) -> jstring {
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_load<'l>(
    env: JNIEnv<'l>,
    _c: JClass<'l>,
    nations: JByteArray<'l>,
    lo: JByteArray<'l>,
    mid: JByteArray<'l>,
    hi: JByteArray<'l>,
    water: JByteArray<'l>,
    marine: JByteArray<'l>,
    cities: JByteArray<'l>,
    f_reg: JByteArray<'l>,
    f_serif: JByteArray<'l>,
) -> jboolean {
    let (n, l, m, h, w, mr, c) = (bytes(&env, &nations), bytes(&env, &lo), bytes(&env, &mid), bytes(&env, &hi), bytes(&env, &water), bytes(&env, &marine), bytes(&env, &cities));
    let fonts = vec![bytes(&env, &f_reg), bytes(&env, &f_serif)];
    let a = world::Assets { nations: &n, world_lo: &l, world_mid: &m, world_hi: &h, water: &w, marine: &mr, cities: &c };
    let r = std::panic::catch_unwind(|| load_app(&a, fonts));
    r.is_ok() as jboolean
}

#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_stage<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    let s = STAGE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    ret(&mut env, s)
}

#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_surfaceCreated<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    #[cfg(target_os = "android")]
    {
        let gl = unsafe { glow::Context::from_loader_function(gles::loader()) };
        let res = with_app(|a| match unsafe { render::Renderer::new(gl, &a.w) } {
            Ok(r) => {
                a.r = Some(r);
                a.own_seen = 0;
                a.zt.full = true;
                a.anx_seen = u64::MAX;
                a.atlas_gen = u64::MAX;
                a.at.full = true;
                String::new()
            }
            Err(e) => e,
        });
        return ret(&mut env, res.unwrap_or_else(|| "no app".into()));
    }
    #[allow(unreachable_code)]
    ret(&mut env, "desktop".into())
}

#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_image<'l>(env: JNIEnv<'l>, _c: JClass<'l>, slot: jint, w: jint, h: jint, px: JByteArray<'l>) {
    let d = bytes(&env, &px);
    with_app(|a| {
        if let Some(r) = a.r.as_mut() {
            unsafe { r.image(slot, w, h, &d) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_resize<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, w: jfloat, h: jfloat, dpr: jfloat) {
    with_app(|a| a.resize(w, h, dpr));
}

#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_frame<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, now: jdouble) -> jlong {
    with_app(|a| {
        a.frame(now);
        a.ui_ver as jlong
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_pan<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, dx: jfloat, dy: jfloat) {
    with_app(|a| a.pan(dx, dy));
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_zoom<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, f: jfloat, x: jfloat, y: jfloat) {
    with_app(|a| a.zoom(f, x, y));
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_tap<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>, x: jfloat, y: jfloat) -> jstring {
    let s = with_app(|a| a.tap(x, y)).unwrap_or_else(|| "{}".into());
    ret(&mut env, s)
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_drawTouch<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, ph: jint, x: jfloat, y: jfloat) -> jboolean {
    with_app(|a| a.draw_touch(ph, x, y)).unwrap_or(false) as jboolean
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_pcTouch<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, ph: jint, x: jfloat, y: jfloat) -> jboolean {
    with_app(|a| a.pc_touch(ph, x, y)).unwrap_or(false) as jboolean
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_pcRect<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, x: jfloat, y: jfloat, w: jfloat, h: jfloat) {
    with_app(|a| a.pc_set_rect(x, y, w, h));
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_ui<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    let s = with_app(|a| a.ui_json()).unwrap_or_else(|| "{}".into());
    ret(&mut env, s)
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_nations<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    let s = with_app(|a| a.nations_json()).unwrap_or_else(|| "[]".into());
    ret(&mut env, s)
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_nationInfo<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>, i: jint) -> jstring {
    let s = with_app(|a| a.nation_info(i as usize)).unwrap_or_else(|| "{}".into());
    ret(&mut env, s)
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_cmd<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>, c: JString<'l>, a: jlong, b: jlong, s: JString<'l>) -> jstring {
    let c = jstr(&mut env, &c);
    let s = jstr(&mut env, &s);
    let r = with_app(|app| app.cmd(&c, a, b, &s)).unwrap_or_default();
    ret(&mut env, r)
}
#[no_mangle]
pub extern "system" fn Java_com_geopolit_game_Native_zoomToNat<'l>(_env: JNIEnv<'l>, _c: JClass<'l>, n: jint) {
    with_app(|a| a.zoom_to_nat(n as u16));
}
