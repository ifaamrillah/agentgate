use loco_rs::prelude::*;

use crate::{
    controllers::{extractors::CurrentUser, shell::app_shell},
    views,
};

#[debug_handler(state = AppContext)]
async fn index(
    CurrentUser(user): CurrentUser,
    ViewEngine(v): ViewEngine<TeraView>,
) -> Result<Response> {
    views::home::index(&v, &app_shell(&user))
}

pub fn routes() -> Routes {
    Routes::new().add("/", get(index))
}
