//! `POST /logout`: hanya POST (preload dan pemindai link tidak boleh me-logout). Sesi yang
//! sudah tidak ada di server tetap berakhir dengan cookie dihapus dan redirect ke login.
use loco_rs::prelude::*;

#[debug_handler(state = AppContext)]
pub async fn submit(session: Session) -> Result<impl IntoResponse> {
    Ok((session.clear(), format::redirect("/login")?))
}
