use {
    std::{env, io},
    winres::WindowsResource,
};

fn main() -> io::Result<()> {
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("build/windows/app.ico")
            .compile()?;
    }
    println!("cargo:rustc-env=ICY_BUILD_DATE={}", chrono::Utc::now().to_rfc3339());

    Ok(())
}
