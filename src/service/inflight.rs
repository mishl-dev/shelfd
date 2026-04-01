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
    use dashmap::mapref::entry::Entry;

    let map_for_guard = map.clone();
    match map.entry(key.clone()) {
        Entry::Occupied(e) => {
            let notify = e.get().clone();
            InflightRole::Waiter(notify)
        }
        Entry::Vacant(e) => {
            let notify = Arc::new(Notify::new());
            e.insert(notify.clone());
            InflightRole::Leader(InflightGuard {
                key,
                map: map_for_guard,
                notify,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn first_caller_becomes_leader() {
        let map: Arc<DashMap<String, Arc<Notify>>> = Arc::new(DashMap::new());
        let role = begin_inflight(map.clone(), "key1".to_owned()).await;
        assert!(matches!(role, InflightRole::Leader(_)));
    }

    #[tokio::test]
    async fn second_caller_becomes_waiter() {
        let map: Arc<DashMap<String, Arc<Notify>>> = Arc::new(DashMap::new());
        let _role1 = begin_inflight(map.clone(), "key1".to_owned()).await;
        let role2 = begin_inflight(map.clone(), "key1".to_owned()).await;
        assert!(matches!(role2, InflightRole::Waiter(_)));
    }

    #[tokio::test]
    async fn different_keys_do_not_interfere() {
        let map: Arc<DashMap<String, Arc<Notify>>> = Arc::new(DashMap::new());
        let role1 = begin_inflight(map.clone(), "key1".to_owned()).await;
        let role2 = begin_inflight(map.clone(), "key2".to_owned()).await;
        assert!(matches!(role1, InflightRole::Leader(_)));
        assert!(matches!(role2, InflightRole::Leader(_)));
    }

    #[tokio::test]
    async fn guard_drop_removes_entry_and_notifies_waiters() {
        let map: Arc<DashMap<String, Arc<Notify>>> = Arc::new(DashMap::new());
        let role1 = begin_inflight(map.clone(), "key1".to_owned()).await;
        let role2 = begin_inflight(map.clone(), "key1".to_owned()).await;

        let waiter_notify = match role2 {
            InflightRole::Waiter(n) => n,
            _ => panic!("expected waiter"),
        };
        let guard = match role1 {
            InflightRole::Leader(g) => g,
            _ => panic!("expected leader"),
        };

        assert!(map.contains_key("key1"));

        let notified = waiter_notify.notified();
        drop(guard);

        assert!(!map.contains_key("key1"));

        notified.await;
    }

    #[test]
    fn into_leader_on_leader_succeeds() {
        let notify = Arc::new(Notify::new());
        let map: Arc<DashMap<String, Arc<Notify>>> = Arc::new(DashMap::new());
        let guard = InflightGuard {
            key: "k".to_owned(),
            map: map.clone(),
            notify: notify.clone(),
        };
        let role = InflightRole::Leader(guard);
        assert!(role.into_leader().is_ok());
    }

    #[test]
    fn into_leader_on_waiter_errors() {
        let notify = Arc::new(Notify::new());
        let role = InflightRole::Waiter(notify);
        assert!(role.into_leader().is_err());
    }

    #[tokio::test]
    async fn waiter_receives_notification_after_leader_drops() {
        let map: Arc<DashMap<String, Arc<Notify>>> = Arc::new(DashMap::new());
        let leader = begin_inflight(map.clone(), "x".to_owned()).await;
        let waiter = begin_inflight(map.clone(), "x".to_owned()).await;

        let waiter_notify = match waiter {
            InflightRole::Waiter(n) => n,
            _ => panic!("expected waiter"),
        };

        let notified = waiter_notify.notified();
        drop(leader);
        notified.await;
    }
}
