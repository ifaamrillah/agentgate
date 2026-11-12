//! View engine Tera 2 beserta fungsi template `t()` (i18n Fluent) dan `icon()`,
//! dan layer yang mengubah respons error menjadi halaman HTML lengkap.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{Extension, Router as AxumRouter, middleware::from_fn_with_state};
use fluent_templates::FluentLoader;
use loco_rs::{
    Error, Result,
    app::{AppContext, Initializer},
    controller::views::{ViewEngine, engines::TeraView},
};
use tera::{Kwargs, State, TeraResult, Value};

use crate::{
    common::{i18n, icons::IconSet},
    controllers::middleware::html_errors,
};

const ICONS_DIR: &str = "assets/icons";

pub struct ViewEngineInitializer;

/// Fungsi Tera `icon(name, class="")`. Output-nya SVG yang sudah aman, jadi `is_safe`.
pub struct IconFn(pub Arc<IconSet>);

impl tera::Function<TeraResult<Value>> for IconFn {
    fn call(&self, kwargs: Kwargs, _state: &State) -> TeraResult<Value> {
        let name: String = kwargs.must_get("name")?;
        let class: String = kwargs.get("class")?.unwrap_or_default();
        self.0
            .render(&name, &class)
            .map(Value::from)
            .ok_or_else(|| tera::Error::message(format!("unknown icon `{name}`")))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Fungsi Tera `palette()`: 18 warna pemilih warna (`views::palette::COLORS`).
pub struct PaletteFn;

impl tera::Function<TeraResult<Value>> for PaletteFn {
    fn call(&self, _kwargs: Kwargs, _state: &State) -> TeraResult<Value> {
        Ok(Value::from(
            crate::views::palette::COLORS
                .iter()
                .map(|c| Value::from(*c))
                .collect::<Vec<_>>(),
        ))
    }
}

/// Bangun view engine aplikasi. Dipakai initializer dan tes (satu implementasi).
pub fn build_view_engine() -> Result<TeraView> {
    let locales = i18n::loader()?;
    let icons = Arc::new(
        IconSet::load(std::path::Path::new(ICONS_DIR))
            .map_err(|e| Error::string(&e.to_string()))?,
    );
    TeraView::build_with_post_process(move |tera| {
        tera.register_function("t", FluentLoader::new(locales.clone()));
        tera.register_function("icon", IconFn(icons.clone()));
        tera.register_function("palette", PaletteFn);
        Ok(())
    })
}

#[async_trait]
impl Initializer for ViewEngineInitializer {
    fn name(&self) -> String {
        "view-engine".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        let view = build_view_engine()?;
        Ok(router
            .layer(from_fn_with_state(view.clone(), html_errors::layer))
            .layer(Extension(ViewEngine::from(view))))
    }
}
