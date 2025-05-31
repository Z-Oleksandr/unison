use tokio::sync::OnceCell;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::UnisonApp;

static APP_INSTANCE: OnceCell<Arc<Mutex<UnisonApp>>> = OnceCell::const_new();

pub async fn init_app() {
    let app = Arc::new(Mutex::new(UnisonApp::new()));
    let _ = APP_INSTANCE.set(app);
}

pub fn get_app() -> Option<Arc<Mutex<UnisonApp>>> {
    APP_INSTANCE.get().cloned()
}
