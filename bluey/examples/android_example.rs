// mod ble;
use android_activity::AndroidApp;
use bluey::Event;
use std::pin::Pin;
use tokio_stream::{Stream, StreamExt, StreamMap};
const DEX_DATA: &[u8] = include_bytes!("./classes.dex");
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) -> anyhow::Result<()> {
    let jvm = unsafe { bluey::jni::JavaVM::from_raw(app.vm_as_ptr() as _)? };
    let activity =
        unsafe { bluey::jni::objects::JObject::from_raw(app.activity_as_ptr() as _) };
    let dex_loader = bluey::android::helper::DexLoader::new(&app, "bluey", DEX_DATA)?;
    let session = bluey::session::SessionConfig::android_new(jvm, activity, dex_loader, None);
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async{
        let session = session.start().await.unwrap();
        let events = session.events().unwrap();
        let mut mainloop = StreamMap::new();
        let bt_event_stream: Pin<Box<dyn Stream<Item = bluey::Event>>> =
            Box::pin(events.map(|bt_event| bt_event));
        mainloop.insert("BLE", bt_event_stream);
        tokio::pin!(mainloop);
        session
            .start_scanning(bluey::session::Filter::new())
            .await.unwrap();
        while let Some((_, event)) = mainloop.next().await {
            match event {
                Event::PeripheralFound { address, name, .. } => {
                    println!("found: {}, {}", address, name)
                }
                _ => {}
            }
        }
        let _ = session.stop_scanning().await;
    });
    Ok(())
}
