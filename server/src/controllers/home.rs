use loco_rs::prelude::*;

use crate::views;

#[debug_handler]
async fn index(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::home::index(&v)
}

pub fn routes() -> Routes {
    Routes::new().add("/", get(index))
}
