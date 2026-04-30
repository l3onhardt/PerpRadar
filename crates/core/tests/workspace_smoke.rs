#[test]
fn workspace_exposes_core_build_info() {
    assert_eq!(perp_radar_core::build_info::crate_name(), "perp-radar-core");
}
