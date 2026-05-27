use anyhow::Result;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

// ── impl functions (no JNI types — easy to test) ─────────────────────────────

/// Creates a new backlogged Action with the given title and serialises it to JSON.
/// The JSON matches the shape expected by `PUT /api/actions/:id` on the server.
fn create_action_impl(title: &str) -> Result<String> {
    let action = simple_core::Action::new(title);
    Ok(serde_json::to_string(&action)?)
}

// ── JNI helpers ───────────────────────────────────────────────────────────────

fn get_string(env: &mut JNIEnv, arg: &JString) -> Option<String> {
    env.get_string(arg).ok().map(|s| s.into())
}

fn return_json_or_throw(env: &mut JNIEnv, result: Result<String>, fallback: &str) -> jstring {
    match result {
        Ok(json) => env
            .new_string(json)
            .expect("Failed to create JNI string")
            .into_raw(),
        Err(err) => {
            let _ = env.throw_new("java/lang/RuntimeException", err.to_string());
            env.new_string(fallback)
                .expect("Failed to create fallback JNI string")
                .into_raw()
        }
    }
}

// ── JNI exports ───────────────────────────────────────────────────────────────

/// Creates a new backlogged Action from a title string.
/// Returns the Action as a JSON string ready to POST to the server.
///
/// Package: com.example.subroutine_simple (underscore → _1 in JNI name)
/// Class:   RustBridge
/// Method:  createAction
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_1simple_RustBridge_createAction(
    mut env: JNIEnv,
    _class: JClass,
    title: JString,
) -> jstring {
    let Some(title) = get_string(&mut env, &title) else {
        return env.new_string("{}").expect("alloc").into_raw();
    };
    return_json_or_throw(&mut env, create_action_impl(&title), "{}")
}
