# icy_engine_gui
UI for icy_engine

The egui frontend uses egui 0.37 and wgpu 30. The optional `legacy-ui`
frontend retains the wgpu version provided by `icy_ui` (currently wgpu 27).
Both compile the same terminal renderer implementation and use the same WGSL
shader and prepared frame data; a thin legacy frame wrapper keeps the two
versions' GPU resources separate.

Check both rendering paths with:

```sh
cargo check -p icy_engine_gui --all-features --all-targets --locked
cargo test -p icy_engine_gui --all-features --locked
# Requires a working GPU adapter:
cargo test -p icy_engine_gui --all-features --test gpu_sampling -- --ignored
```
