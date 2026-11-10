use loco_rs::prelude::*;

pub fn ui(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "dev/ui.html", data!({}))
}
