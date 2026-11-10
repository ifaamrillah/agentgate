//! View engine aplikasi: fungsi template custom terdaftar dan ketat.

use agentgate_server::initializers::view_engine::{IconFn, build_view_engine};
use loco_rs::controller::views::ViewRenderer;

#[test]
fn view_engine_loads_every_template() {
    build_view_engine().unwrap();
}

#[test]
fn unknown_icon_name_is_a_render_error() {
    let dir = std::env::temp_dir().join("agentgate-icon-test/views");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("x.html"), r#"{{ icon(name="tidak-ada") }}"#).unwrap();
    let view = loco_rs::controller::views::engines::TeraView::from_custom_dir(&dir, |tera| {
        let icons =
            agentgate_server::common::icons::IconSet::load(std::path::Path::new("assets/icons"))
                .unwrap();
        tera.register_function("icon", IconFn(std::sync::Arc::new(icons)));
        Ok(())
    })
    .unwrap();
    assert!(view.render("x.html", serde_json::json!({})).is_err());
}
