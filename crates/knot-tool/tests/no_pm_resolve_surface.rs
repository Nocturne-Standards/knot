//! After public-launch peel, knot must not expose PM-resolve as a product surface.
#[test]
fn tool_crate_has_no_pm_resolve_modules() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    for rel in [
        "src/pm_resolve_types.rs",
        "src/pm_read_types.rs",
        "static/pm-resolve.html",
        "static/pm-resolve-app.js",
    ] {
        let p = std::path::Path::new(manifest_dir).join(rel);
        assert!(
            !p.exists(),
            "PM-resolve peel incomplete: {} still exists",
            p.display()
        );
    }
}

#[test]
fn main_help_text_has_no_pm_resolve_subcommand_docs() {
    let main = include_str!("../src/main.rs");
    assert!(
        !main.contains("PmResolve") && !main.contains("pm-resolve"),
        "main.rs still references PmResolve / pm-resolve"
    );
}
