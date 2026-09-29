//! rust-embed resolves the `i18n` folder when the crate compiles, but does not tell cargo to
//! watch it: after the folder moves, or when languages are added, the crate would keep an
//! outdated embedding and every translation shows as "No localization for id". Watching the
//! folder recompiles the crate whenever a translation changes.
fn main() {
    println!("cargo:rerun-if-changed=i18n");
}
