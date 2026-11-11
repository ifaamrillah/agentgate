use loco_rs::prelude::*;

use super::layout::{AppShell, app_page};

pub fn index(v: &impl ViewRenderer, shell: &AppShell) -> Result<Response> {
    format::render().view(v, "home/index.html", app_page(shell, data!({})))
}
