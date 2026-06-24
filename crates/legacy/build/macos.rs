use std::env;

pub fn target_clang_args(target: &str) -> Vec<String> {
    let mut args = Vec::new();
    if target.contains("apple-ios-sim") || target.contains("x86_64-apple-ios") {
        let arch = if target.contains("aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        let version = env::var("IPHONEOS_DEPLOYMENT_TARGET")
            .unwrap_or_else(|_| "17.0".into());
        args.push(format!("--target={}-apple-ios{}-simulator", arch, version));
        if let Ok(sdkroot) = env::var("SDKROOT") {
            args.push(format!("-isysroot{}", sdkroot));
        }
    }
    args
}
