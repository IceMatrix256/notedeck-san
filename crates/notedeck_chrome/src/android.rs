//#[cfg(target_os = "android")]
//use egui_android::run_android;

use egui_winit::winit::platform::android::activity::AndroidApp;

use crate::chrome::Chrome;
use notedeck::Notedeck;

#[no_mangle]
#[tokio::main]
pub async fn android_main(android_app: AndroidApp) {
    use tracing_logcat::{LogcatMakeWriter, LogcatTag};
    use tracing_subscriber::{prelude::*, EnvFilter};

    std::env::set_var("RUST_BACKTRACE", "full");
    //std::env::set_var("DAVE_MODEL", "hhao/qwen2.5-coder-tools:latest");
    std::env::set_var(
        "RUST_LOG",
        "egui=debug,egui-winit=debug,winit=debug,notedeck=debug,notedeck_columns=debug,notedeck_chrome=debug,enostr=debug,android_activity=debug,contacts=debug",
    );

    //std::env::set_var(
    //    "RUST_LOG",
    //    "enostr=debug,notedeck_columns=debug,notedeck_chrome=debug",
    //);

    let logcat_layer = tracing_subscriber::fmt::layer()
        .with_level(true)
        .with_target(true)
        .without_time()
        .with_writer(
            LogcatMakeWriter::new(LogcatTag::Target)
                .expect("Failed to initialize logcat writer"),
        );
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_level(true)
        .with_target(true)
        .without_time();

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(logcat_layer)
        .with(stdout_layer)
        .init();

    tracing::info!(target: "contacts", "Tracing initialized");

    let _ = android_keyring::set_android_keyring_credential_builder();

    let path = android_app.internal_data_path().expect("data path");
    let mut options = eframe::NativeOptions {
        depth_buffer: 24,
        ..eframe::NativeOptions::default()
    };

    options.renderer = eframe::Renderer::Wgpu;
    // Clone `app` to use it both in the closure and later in the function
    //let app_clone_for_event_loop = app.clone();
    //options.event_loop_builder = Some(Box::new(move |builder| {
    //    builder.with_android_app(app_clone_for_event_loop);
    //}));

    options.android_app = Some(android_app.clone());

    let app_args = get_app_args(&android_app);

    let _res = eframe::run_native(
        "Damus Notedeck",
        options,
        Box::new(move |cc| {
            let ctx = &cc.egui_ctx;

            let mut notedeck = Notedeck::new(ctx, path, &app_args);
            notedeck.set_android_context(android_app);
            notedeck.setup(ctx);
            let chrome = Chrome::new_with_apps(cc, &app_args, &mut notedeck)?;
            notedeck.set_app(chrome);

            Ok(Box::new(notedeck))
        }),
    );
}
/*
Read args from a config file:
- allows use of more interesting args w/o risk of checking them in by mistake
- allows use of different args w/o rebuilding the app
- uses compiled in defaults if config file missing or broken

Example android-config.json:
```
{
  "args": [
    "argv0-placeholder",
    "--npub",
    "npub1h50pnxqw9jg7dhr906fvy4mze2yzawf895jhnc3p7qmljdugm6gsrurqev",
    "-c",
    "contacts",
    "-c",
    "notifications"
  ]
}
```

Install/update android-config.json with:
```
adb push android-config.json /sdcard/Android/data/com.damus.notedeck/files/android-config.json
```

Using internal storage would be better but it seems hard to get the config file onto
the device ...
*/

fn get_app_args(app: &AndroidApp) -> Vec<String> {
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    let default_args = vec!["argv0-placeholder".to_string()];

    let Some(external_data_path) = app.external_data_path() else {
        tracing::debug!(target: "android-config", "No external data path; using defaults");
        return default_args;
    };

    let config_file: PathBuf = external_data_path.to_path_buf().join("android-config.json");
    let config_contents = match fs::read_to_string(&config_file) {
        Ok(contents) => contents,
        Err(e) => {
            tracing::debug!(
                target: "android-config",
                "No android-config.json at {:?}: {e}",
                config_file
            );
            return default_args;
        }
    };

    let Ok(json) = serde_json::from_str::<Value>(&config_contents) else {
        tracing::warn!(
            target: "android-config",
            "Could not parse android-config.json; using defaults"
        );
        return default_args;
    };

    if let Some(args_array) = json.get("args").and_then(|v| v.as_array()) {
        let config_args: Vec<String> = args_array
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if !config_args.is_empty() {
            tracing::info!(
                target: "android-config",
                "Loaded {} args from android-config.json",
                config_args.len()
            );
            return config_args;
        }
    }

    tracing::warn!(
        target: "android-config",
        "android-config.json present but contained no args; using defaults"
    );
    default_args
}
