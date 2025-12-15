//! Tests for PointRole mirroring

use icy_engine_edit::brushes::PointRole;

#[test]
fn test_point_role_mirror_horizontal() {
    assert_eq!(PointRole::NWCorner.mirror_horizontal(), PointRole::NECorner);
    assert_eq!(PointRole::NECorner.mirror_horizontal(), PointRole::NWCorner);
    assert_eq!(PointRole::SWCorner.mirror_horizontal(), PointRole::SECorner);
    assert_eq!(PointRole::SECorner.mirror_horizontal(), PointRole::SWCorner);
    assert_eq!(PointRole::LeftSide.mirror_horizontal(), PointRole::RightSide);
    assert_eq!(PointRole::RightSide.mirror_horizontal(), PointRole::LeftSide);
    assert_eq!(PointRole::TopSide.mirror_horizontal(), PointRole::TopSide);
    assert_eq!(PointRole::BottomSide.mirror_horizontal(), PointRole::BottomSide);
    assert_eq!(PointRole::Fill.mirror_horizontal(), PointRole::Fill);
}

#[test]
fn test_point_role_mirror_vertical() {
    assert_eq!(PointRole::NWCorner.mirror_vertical(), PointRole::SWCorner);
    assert_eq!(PointRole::NECorner.mirror_vertical(), PointRole::SECorner);
    assert_eq!(PointRole::SWCorner.mirror_vertical(), PointRole::NWCorner);
    assert_eq!(PointRole::SECorner.mirror_vertical(), PointRole::NECorner);
    assert_eq!(PointRole::TopSide.mirror_vertical(), PointRole::BottomSide);
    assert_eq!(PointRole::BottomSide.mirror_vertical(), PointRole::TopSide);
    assert_eq!(PointRole::LeftSide.mirror_vertical(), PointRole::LeftSide);
    assert_eq!(PointRole::RightSide.mirror_vertical(), PointRole::RightSide);
    assert_eq!(PointRole::Fill.mirror_vertical(), PointRole::Fill);
}

#[test]
fn test_point_role_mirror_both() {
    assert_eq!(PointRole::NWCorner.mirror_both(), PointRole::SECorner);
    assert_eq!(PointRole::NECorner.mirror_both(), PointRole::SWCorner);
    assert_eq!(PointRole::SWCorner.mirror_both(), PointRole::NECorner);
    assert_eq!(PointRole::SECorner.mirror_both(), PointRole::NWCorner);
}
