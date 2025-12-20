//! Tests for editor operations
//!
//! These tests verify that all edit actions:
//! 1. Work correctly
//! 2. Push exactly one undo operation onto the stack when they change something
//! 3. Can be undone

mod area_operations_tests;
mod edit_operations_tests;
mod layer_operations_tests;
