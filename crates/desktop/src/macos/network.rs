use opentrickster_engine::{Mode, SessionKey};
use opentrickster_net::{NativeSession, NetCredentials, NetError, VerifiedProfile};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

pub enum Prepared {
    Embedded(VerifiedProfile),
    Native(NativeSession),
}
pub enum Completion {
    Prepared {
        session: SessionKey,
        result: Result<Prepared, NetError>,
    },
    Closed {
        session: SessionKey,
        result: Result<(), NetError>,
    },
}
pub struct Background {
    sender: mpsc::SyncSender<Completion>,
    receiver: mpsc::Receiver<Completion>,
    overflow: Arc<AtomicBool>,
}
impl Background {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(4);
        Self {
            sender,
            receiver,
            overflow: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn prepare(
        &self,
        session: SessionKey,
        mode: Mode,
        credentials: Option<NetCredentials>,
        cancel: Arc<AtomicBool>,
        cache: PathBuf,
    ) {
        let sender = self.sender.clone();
        let overflow = self.overflow.clone();
        thread::spawn(move || {
            let result = (|| {
                let profile = VerifiedProfile::fetch_public(&cache)?;
                if cancel.load(Ordering::Acquire) {
                    return Err(NetError::Cancelled);
                }
                match mode {
                    Mode::Assisted => Ok(Prepared::Embedded(profile)),
                    Mode::Clientless => NativeSession::open(
                        session,
                        profile,
                        credentials.ok_or(NetError::InvalidCredentials)?,
                    )
                    .map(Prepared::Native),
                }
            })();
            if cancel.load(Ordering::Acquire) {
                let result = close_prepared(result.ok());
                post(&sender, Completion::Closed { session, result }, &overflow);
            } else {
                post(&sender, Completion::Prepared { session, result }, &overflow);
            }
        });
    }
    pub fn close(&self, session: SessionKey, native: NativeSession) {
        let sender = self.sender.clone();
        let overflow = self.overflow.clone();
        thread::spawn(move || {
            let mut native = native;
            let result = native.close();
            post(&sender, Completion::Closed { session, result }, &overflow);
        });
    }
    pub fn next(&self) -> Option<Completion> {
        self.receiver.try_recv().ok()
    }
    pub fn overflowed(&self) -> bool {
        self.overflow.swap(false, Ordering::AcqRel)
    }
}
fn post(sender: &mpsc::SyncSender<Completion>, event: Completion, overflow: &AtomicBool) {
    if let Err(error) = sender.try_send(event) {
        overflow.store(true, Ordering::Release);
        let event = match error {
            mpsc::TrySendError::Full(event) | mpsc::TrySendError::Disconnected(event) => event,
        };
        if let Completion::Prepared {
            result: Ok(prepared),
            ..
        } = event
        {
            let _ = close_prepared(Some(prepared));
        }
    }
}
fn close_prepared(prepared: Option<Prepared>) -> Result<(), NetError> {
    match prepared {
        Some(Prepared::Native(mut native)) => native.close(),
        _ => Ok(()),
    }
}
pub fn public_cache() -> Result<PathBuf, NetError> {
    let directory = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(NetError::Io)?;
    Ok(directory
        .join("Library/Caches/org.opentrickster.desktop/public")
        .join(opentrickster_net::BUILD))
}
