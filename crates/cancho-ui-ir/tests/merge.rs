use cancho_ui_ir::{merge_ir_files, schema_version};

#[test]
fn merging_nothing_is_an_empty_module_not_a_panic() {
    let m = merge_ir_files(&[]);
    assert!(m.components.is_empty());
    assert_eq!(m.version, schema_version());
}
