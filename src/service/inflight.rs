use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use tokio::sync::Notify;

pub enum InflightRole {
    Leader(InflightGuard),
    Waiter(Arc<Notify>),
}

impl InflightRole {
    pub fn into_leader(self) -> Result<InflightGuard> {
        match self {
            Self::Leader(guard) => Ok(guard),
            Self::Waiter(_) => anyhow::bail!("expected inflight leader"),
        }
    }
}

pub struct InflightGuard {
    key: String,
    map: Arc<DashMap<String, Arc<Notify>>>,
    notify: Arc<Notify>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.map.remove(&self.key);
        self.notify.notify_waiters();
    }
}

pub async fn begin_inflight(map: Arc<DashMap<String, Arc<Notify>>>, key: String) -> InflightRole {
    if let Some(existing) = map.get(&key) {
        return InflightRole::Waiter(existing.clone());
    }

    let notify = Arc::new(Notify::new());
    map.insert(key.clone(), notify.clone());
    InflightRole::Leader(InflightGuard {
        key,
        map: map.clone(),
        notify,
    })
}
