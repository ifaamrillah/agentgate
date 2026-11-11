use loco_rs::prelude::*;

use crate::{controllers::extractors::CurrentUser, views};

#[debug_handler(state = AppContext)]
async fn index(
    CurrentUser(_user): CurrentUser,
    ViewEngine(v): ViewEngine<TeraView>,
) -> Result<Response> {
    views::home::index(&v)
}

pub fn routes() -> Routes {
    Routes::new().add("/", get(index))
}
