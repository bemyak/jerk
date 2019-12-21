#![cfg_attr(feature = "nightly", feature(external_doc)  )] // https://doc.rust-lang.org/unstable-book/language-features/external-doc.html
#![cfg_attr(feature = "nightly", doc(include = "../Readme.md"))]

use jni_sys::*;
use std::convert::*;
use std::fmt::{self, Debug, Display, Formatter};
use std::ptr::null_mut;

pub type Result<T> = std::result::Result<T, JavaTestError>;

#[derive(Clone)]
pub enum JavaTestError {
    Unknown(String),
    #[doc(hidden)] _NonExhaustive,
}

impl Display for JavaTestError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            JavaTestError::Unknown(message) => write!(fmt, "{}", message),
            JavaTestError::_NonExhaustive   => write!(fmt, "NonExhaustive"),
        }
    }
}

impl Debug for JavaTestError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        Display::fmt(self, fmt)
    }
}

impl<'a> From<&'a str> for JavaTestError {
    fn from(value: &'a str) -> Self {
        JavaTestError::Unknown(value.to_string())
    }
}


impl From<String> for JavaTestError {
    fn from(value: String) -> Self {
        JavaTestError::Unknown(value)
    }
}


/// Execute a Java unit test.  The method must be static, return void, and take no arguments.
pub fn run_test(package: &str, class: &str, method: &str) -> Result<()> {
    let env = test_thread_env();
    if env == null_mut() { return Err("Couldn't initialize Java VM".into()); }
    
    let class_id    = format!("{}/{}\0", package.replace(".", "/"), class);
    let method_id   = format!("{}\0", method);
    
    // Safety:
    // * `**env` must be valid (non-null, not dangling, valid fn pointers if present)
    // * string IDs must be `\0` terminated
    unsafe {
        let class_id    = (**env).FindClass.unwrap()(env, class_id.as_ptr() as *const _);
        assert_ne!(class_id, null_mut(), "Failed to FindClass {}.{} - the corresponding .jar may not be loaded", package, class);
        let method_id   = (**env).GetStaticMethodID.unwrap()(env, class_id, method_id.as_ptr() as *const _, "()V\0".as_ptr() as *const _);
        assert_ne!(method_id, null_mut(), "Failed to GetStaticMethodID {}.{}", class, method);
        (**env).CallStaticVoidMethodA.unwrap()(env, class_id, method_id, [].as_ptr());
        if (**env).ExceptionCheck.unwrap()(env) == JNI_TRUE {
            (**env).ExceptionDescribe.unwrap()(env);
            (**env).ExceptionClear.unwrap()(env);
            Err(format!("{}.{}() threw a Java Exception", class, method).into())
        } else {
            Ok(())
        }
    }
}



/// Get a handle to the current Java VM, or create one if it doesn't already exist.
pub fn test_vm() -> *mut JavaVM { **VM }
lazy_static::lazy_static! { static ref VM : ThreadSafe<*mut JavaVM> = ThreadSafe(create_java_vm()); }

/// Get a handle to the Java environment for the current thread, attaching if one doesn't already exist.
pub fn test_thread_env() -> *mut JNIEnv { ENV.with(|e| *e) }
thread_local! { static ENV : *mut JNIEnv = attach_current_thread(); }

fn attach_current_thread() -> *mut JNIEnv {
    let vm = test_vm();
    let mut env = null_mut();
    assert_eq!(JNI_OK, unsafe { (**vm).AttachCurrentThread.unwrap()(vm, &mut env, null_mut()) });
    env as *mut _
}

fn create_java_vm() -> *mut JavaVM {
    let mut vm  = 0 as *mut _;
    let mut env = 0 as *mut _;

    let classpath = format!("-Djava.class.path={}\0", std::env::var("CLASSPATH").unwrap());

    let mut options = [
        //JavaVMOption { optionString: "-verbose:class\0".as_ptr() as *const _ as *mut _, extraInfo: null_mut() },
        //JavaVMOption { optionString: "-verbose:jni\0".as_ptr() as *const _ as *mut _, extraInfo: null_mut() },
        JavaVMOption { optionString: "-ea\0".as_ptr() as *const _ as *mut _, extraInfo: null_mut() }, // Enable Assertions
        JavaVMOption { optionString: "-esa\0".as_ptr() as *const _ as *mut _, extraInfo: null_mut() }, // Enable System Assertions
        JavaVMOption { optionString: classpath.as_ptr() as *const _ as *mut _, extraInfo: null_mut() },
    ];

    let mut args = JavaVMInitArgs {
        version:            JNI_VERSION_1_6,
        nOptions:           options.len() as _,
        options:            options.as_mut_ptr(),
        ignoreUnrecognized: JNI_FALSE,
    };

    assert_eq!(JNI_OK, unsafe { JNI_GetDefaultJavaVMInitArgs(&mut args as *mut _ as *mut _) });
    assert_eq!(JNI_OK, unsafe { JNI_CreateJavaVM(&mut vm, &mut env, &mut args as *mut _ as *mut _) });

    vm
}

struct ThreadSafe<T>(pub T);
impl<T> std::ops::Deref for ThreadSafe<T> { type Target = T; fn deref(&self) -> &Self::Target { &self.0 } }
unsafe impl<T> Send for ThreadSafe<T> {}
unsafe impl<T> Sync for ThreadSafe<T> {}
