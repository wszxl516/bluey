use android_activity::AndroidApp;
use ::jni::JavaVM;
use ::jni::objects::{GlobalRef, JObject, JValue};
use ::jni::sys::jint;
use jni_min_helper::*;
use jni_min_helper::JObjectGet;
use log::info;
#[derive(Debug)]
pub struct DexLoader {
    loader: GlobalRef,
    app: AndroidApp,
}
impl DexLoader {
    pub fn new(app: &AndroidApp, dex_name: &str, dex_data: &[u8]) -> anyhow::Result<DexLoader> {
        let dex_obj = Self::load_java_helper(app, dex_name, dex_data)?;
        Ok(Self {
            loader: dex_obj,
            app: app.clone(),
        })
    }
    pub fn vm(&self) -> anyhow::Result<JavaVM> {
        unsafe { JavaVM::from_raw(self.app.vm_as_ptr().cast()) }.map_err(anyhow::Error::from)
    }
    pub fn get_class(&self, class_name: &str) -> anyhow::Result<GlobalRef> {
        let vm = self.vm()?;
        let mut env = vm.attach_current_thread()?;
        let class_name = env.new_string(class_name)?;
        let helper_class = env
            .call_method(
                &self.loader,
                "findClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(class_name.into())],
            )?
            .l()?;

        let class = env.new_global_ref(helper_class)?;
        Ok(class)
    }
    fn load_java_helper(
        app: &AndroidApp,
        dex_name: &str,
        dex_data: &[u8],
    ) -> anyhow::Result<GlobalRef> {
        // Safety: as documented in android-activity to obtain a jni::JavaVM
        let native_activity = unsafe { JObject::from_raw(app.activity_as_ptr() as *mut _) };

        let vm =
            unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(anyhow::Error::from)?;
        let mut env = vm.attach_current_thread()?;
        // Safety: dex_data is 'static and the InMemoryDexClassLoader will not mutate it it
        let dex_buffer = unsafe {
            env.new_direct_byte_buffer(dex_data.as_ptr() as *mut _, dex_data.len())
                .unwrap()
        };

        let parent_class_loader = env
            .call_method(
                native_activity,
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;

        let os_build_class = env.find_class("android/os/Build$VERSION")?;
        let sdk_ver = env.get_static_field(os_build_class, "SDK_INT", "I")?.i()?;

        let dex_loader = if sdk_ver >= 26 {
            env.new_object(
                "dalvik/system/InMemoryDexClassLoader",
                "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
                &[
                    JValue::Object(dex_buffer.into()),
                    JValue::Object(parent_class_loader.into()),
                ],
            )?
        } else {
            // writes the dex data into the application internal storage
            let dex_data_len = dex_data.len() as i32;
            let dex_byte_array = env.byte_array_from_slice(dex_data)?;

            let dex_dir = env.new_string("dex")?;
            let dex_dir_path = env
                .call_method(
                    native_activity,
                    "getDir",
                    "(Ljava/lang/String;I)Ljava/io/File;",
                    &[JValue::Object(dex_dir.into()), JValue::from(0 as jint)],
                )?
                .l()?;
            let dex_name = env.new_string(dex_name.to_owned() + ".dex")?;
            let dex_path =
                env.new_object("java/io/File", "(Ljava/io/File;Ljava/lang/String;)V", &[
                    JValue::Object(dex_dir_path.into()),
                    JValue::Object(dex_name.into()),
                ])?;
            let dex_path = env
                .call_method(dex_path, "getAbsolutePath", "()Ljava/lang/String;", &[])?
                .l()?;

            // prepares the folder for optimized dex generated while creating `DexClassLoader`
            let out_dex_dir = env.new_string("outdex")?;
            let out_dex_dir_path = env
                .call_method(
                    native_activity,
                    "getDir",
                    "(Ljava/lang/String;I)Ljava/io/File;",
                    &[JValue::Object(out_dex_dir.into()), JValue::from(0 as jint)],
                )?
                .l()?;
            let out_dex_dir_path = env
                .call_method(
                    out_dex_dir_path,
                    "getAbsolutePath",
                    "()Ljava/lang/String;",
                    &[],
                )?
                .l()?;

            // writes the dex data
            let write_stream =
                env.new_object("java/io/FileOutputStream", "(Ljava/lang/String;)V", &[
                    JValue::Object(dex_path.into()),
                ])?;
            env.call_method(write_stream, "write", "([BII)V", &[
                JValue::Object(unsafe {JObject::from_raw(dex_byte_array)}),
                JValue::from(0 as jint),
                JValue::from(dex_data_len as jint),
            ])?;
            env.call_method(write_stream, "close", "()V", &[])?;

            // loads the dex file
            env.new_object(
                "dalvik/system/DexClassLoader",
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/ClassLoader;)V",
                &[
                    JValue::Object(dex_path.into()),
                    JValue::Object(out_dex_dir_path.into()),
                    JValue::Object(JObject::null()),
                    JValue::Object(parent_class_loader.into()),
                ],
            )?
        };
        let o = env.new_global_ref(dex_loader)?;
        Ok(o)
    }
}
