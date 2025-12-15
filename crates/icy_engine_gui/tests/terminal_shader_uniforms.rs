// Intentionally left empty.
//
// The CRT uniform struct (`CRTUniforms`) is private to `terminal_shader.rs`.
// Integration tests in `tests/` can't access private items, so the actual
// layout/size assertion lives in a `#[cfg(test)]` module next to the struct.
