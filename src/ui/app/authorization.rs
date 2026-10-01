use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Sender},
};
use std::time::{Duration, Instant};

type AuthNotifier = dyn Fn(u64, &str) -> bool + Send + Sync;

struct Pending {
    id: u64,
    reply: Sender<bool>,
    deadline: Option<Instant>,
}

pub(crate) struct AuthorizationGate {
    notify: Box<AuthNotifier>,
    next_id: AtomicU64,
    pending: Mutex<Option<Pending>>,
    timeout: Option<Duration>,
    resolved: Box<dyn Fn(u64) + Send + Sync>,
}

impl AuthorizationGate {
    #[cfg(test)]
    pub(crate) fn new(notify: impl Fn(u64, &str) -> bool + Send + Sync + 'static) -> Self {
        Self::with_timeout(notify, |_| {}, None)
    }

    pub(crate) fn with_timeout(
        notify: impl Fn(u64, &str) -> bool + Send + Sync + 'static,
        resolved: impl Fn(u64) + Send + Sync + 'static,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            notify: Box::new(notify),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(None),
            timeout,
            resolved: Box::new(resolved),
        }
    }

    pub(crate) fn request(&self, prompt: &str) -> bool {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        let deadline = self.timeout.map(|timeout| Instant::now() + timeout);
        *self.pending.lock().expect("授权锁损坏") = Some(Pending {
            id,
            reply,
            deadline,
        });
        if !(self.notify)(id, prompt) {
            self.cancel();
            return false;
        }
        let allowed = match deadline {
            Some(deadline) => answer
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or(false),
            None => answer.recv().unwrap_or(false),
        };
        let expired = {
            let mut pending = self.pending.lock().expect("授权锁损坏");
            if pending.as_ref().is_some_and(|pending| pending.id == id) {
                pending.take();
                true
            } else {
                false
            }
        };
        if expired {
            (self.resolved)(id);
        }
        allowed
    }

    pub(crate) fn respond(&self, id: u64, allowed: bool) -> bool {
        let pending = {
            let mut pending = self.pending.lock().expect("授权锁损坏");
            if !pending.as_ref().is_some_and(|pending| pending.id == id) {
                return false;
            }
            pending.take()
        };
        let Some(pending) = pending else {
            return false;
        };
        let valid = pending
            .deadline
            .is_none_or(|deadline| Instant::now() < deadline);
        // 先清除展示缓存再唤醒工作线程，重连无需等待线程调度才能知道授权已解除。
        (self.resolved)(id);
        let sent = pending.reply.send(valid && allowed).is_ok();
        valid && sent
    }

    pub(crate) fn pending_id(&self) -> Option<u64> {
        self.pending
            .lock()
            .expect("授权锁损坏")
            .as_ref()
            .map(|pending| pending.id)
    }

    pub(crate) fn cancel(&self) {
        let pending = self.pending.lock().expect("授权锁损坏").take();
        if let Some(pending) = pending {
            (self.resolved)(pending.id);
            let _ = pending.reply.send(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, sync::mpsc, time::Duration};

    use super::AuthorizationGate;

    #[test]
    fn denied_and_late_replies_never_authorize_next_tool() {
        let (sent, received) = mpsc::channel();
        let gate = Arc::new(AuthorizationGate::new(move |id, prompt| {
            sent.send((id, prompt.to_owned())).is_ok()
        }));
        assert_eq!(gate.pending_id(), None);
        let pending = Arc::clone(&gate);
        let first = std::thread::spawn(move || pending.request("write /tmp/a"));
        let (id, prompt) = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(prompt, "write /tmp/a");
        assert_eq!(gate.pending_id(), Some(id));
        assert!(gate.respond(id, false));
        assert!(!first.join().unwrap());
        assert_eq!(gate.pending_id(), None);
        assert!(!gate.respond(id, true));

        let pending = Arc::clone(&gate);
        let second = std::thread::spawn(move || pending.request("bash dangerous"));
        let (next_id, _) = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(next_id > id);
        assert_eq!(gate.pending_id(), Some(next_id));
        gate.cancel();
        assert!(!second.join().unwrap());
        assert_eq!(gate.pending_id(), None);
        assert!(!gate.respond(next_id, true));
    }

    #[test]
    fn frontend_disconnect_defaults_to_denial() {
        let gate = AuthorizationGate::new(|_, _| false);
        assert!(!gate.request("write /tmp/a"));
    }

    #[test]
    fn unanswered_request_expires_and_resolves_its_own_id() {
        let (resolved, received) = mpsc::channel();
        let gate = AuthorizationGate::with_timeout(
            |_, _| true,
            move |id| {
                resolved.send(id).unwrap();
            },
            Some(Duration::from_millis(10)),
        );
        assert!(!gate.request("write"));
        let id = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(gate.pending_id(), None);
        assert!(!gate.respond(id, true));
    }

    #[test]
    fn first_browser_reply_wins() {
        let (sent, received) = mpsc::channel();
        let gate = Arc::new(AuthorizationGate::new(move |id, _| sent.send(id).is_ok()));
        let worker = Arc::clone(&gate);
        let pending = std::thread::spawn(move || worker.request("tool"));
        let id = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(gate.respond(id, true));
        assert!(!gate.respond(id, false));
        assert!(pending.join().unwrap());
    }

    #[test]
    fn disconnect_resolves_before_the_worker_can_receive_its_denial() {
        let (notified, notification) = mpsc::channel();
        let (release, paused) = mpsc::channel();
        let paused = std::sync::Mutex::new(paused);
        let (resolved, resolution) = mpsc::channel();
        let gate = Arc::new(AuthorizationGate::with_timeout(
            move |id, _| {
                notified.send(id).unwrap();
                paused
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .is_ok()
            },
            move |id| {
                resolved.send(id).unwrap();
            },
            None,
        ));
        let worker = Arc::clone(&gate);
        let request = std::thread::spawn(move || worker.request("tool"));
        let id = notification.recv_timeout(Duration::from_secs(2)).unwrap();
        gate.cancel();
        assert_eq!(
            resolution.recv_timeout(Duration::from_millis(100)).unwrap(),
            id
        );
        release.send(()).unwrap();
        assert!(!request.join().unwrap());
        assert!(resolution.try_recv().is_err());
    }
}
