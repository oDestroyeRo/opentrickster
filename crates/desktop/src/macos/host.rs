use super::{macos_native, native_input::NativeInput};
use crate::layout::{GameRect, game_rect};
use gpui::{Bounds, Pixels, Window};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    time::Instant,
};
use wry::{
    NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder, WebViewBuilderExtDarwin,
    dpi::{LogicalPosition, LogicalSize},
};

pub const OFFICIAL_URL: &str = "https://tr-play.playall.in.th/";
const MAX_IPC: usize = 65_536;
const MAX_INBOX: usize = 256;

#[derive(Clone, Copy)]
pub enum HostSource {
    Official { probe: bool },
    OfflineFixture,
    InputFixture,
}
pub enum HostEvent {
    PageReady,
    /// The WebContent process terminated; its page script and sockets are gone.
    ProcessFailed,
    /// The live page could not be placed in the window.
    PlacementFailed,
    Neutralized,
    NetworkPage {
        origin: String,
        json: String,
    },
    Overflow,
    FullscreenRequested,
    PhysicalInput,
    Message(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HostEpoch(pub u64);
pub struct PageIdentity {
    pub epoch: HostEpoch,
    guard: String,
}
impl PageIdentity {
    pub fn fresh(epoch: HostEpoch) -> Result<Self, &'static str> {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce)
            .map_err(|_| "The game observation guard could not be created.")?;
        Ok(Self {
            epoch,
            guard: nonce.iter().map(|byte| format!("{byte:02x}")).collect(),
        })
    }
    pub fn binding(&self) -> Result<opentrickster_net::PageBinding, opentrickster_net::NetError> {
        opentrickster_net::PageBinding::new(self.epoch.0, self.guard.clone())
    }
}
pub struct HostMessage {
    pub epoch: HostEpoch,
    pub event: HostEvent,
    pub ingress: Instant,
}
pub struct HostQueue {
    active: Cell<Option<HostEpoch>>,
    queue: RefCell<VecDeque<HostMessage>>,
}
pub type HostInbox = Rc<HostQueue>;
impl HostQueue {
    pub fn new() -> Self {
        Self {
            active: Cell::new(None),
            queue: RefCell::new(VecDeque::new()),
        }
    }
    fn activate(&self, epoch: HostEpoch) {
        self.queue.borrow_mut().clear();
        self.active.set(Some(epoch));
    }
    fn deactivate(&self, epoch: HostEpoch) {
        if self.active.get() == Some(epoch) {
            self.active.set(None);
            self.queue.borrow_mut().clear();
        }
    }
    pub fn drain(&self) -> Vec<HostMessage> {
        self.queue.borrow_mut().drain(..).collect()
    }
}
pub type ChildSlot = Rc<RefCell<Option<WebView>>>;

pub(super) fn enqueue(inbox: &HostInbox, epoch: HostEpoch, event: HostEvent) {
    enqueue_at(inbox, epoch, event, Instant::now());
}
fn enqueue_at(inbox: &HostInbox, epoch: HostEpoch, event: HostEvent, ingress: Instant) {
    if inbox.active.get() != Some(epoch) {
        return;
    }
    let mut queue = inbox.queue.borrow_mut();
    if queue.len() >= MAX_INBOX {
        queue.clear();
        queue.push_back(HostMessage {
            epoch,
            event: HostEvent::Overflow,
            ingress,
        });
    } else {
        queue.push_back(HostMessage {
            epoch,
            event,
            ingress,
        });
    }
}

pub struct EmbeddedHost {
    pub epoch: HostEpoch,
    inbox: HostInbox,
    pub child: ChildSlot,
    pub placement: Rc<RefCell<Option<GameRect>>>,
    pub input: Rc<RefCell<NativeInput>>,
    neutralizing: Rc<Cell<bool>>,
}

impl EmbeddedHost {
    pub fn open(
        window: &Window,
        inbox: HostInbox,
        source: HostSource,
        identity: PageIdentity,
        observer_script: Option<&opentrickster_net::ObserverScript>,
    ) -> Result<Self, &'static str> {
        let PageIdentity { epoch, guard } = identity;
        let verification = matches!(
            source,
            HostSource::OfflineFixture | HostSource::InputFixture
        );
        let probe = matches!(source, HostSource::Official { probe: true });
        inbox.activate(epoch);
        let ipc = inbox.clone();
        let failed = inbox.clone();
        let loaded = inbox.clone();
        let policy = include_str!("../../resources/page-policy.js").replace("__OT_GUARD__", &guard);
        let target_probe =
            include_str!("../../verification/target.js").replace("__OT_GUARD__", &guard);
        let navigation_seen = Cell::new(false);
        let neutralizing = Rc::new(Cell::new(false));
        let navigation_close = neutralizing.clone();
        let loaded_close = neutralizing.clone();
        let builder = WebViewBuilder::new()
            .with_bounds(Rect {
                position: LogicalPosition::new(3., 58.).into(),
                size: LogicalSize::new(960., 540.).into(),
            })
            .with_focused(false)
            .with_devtools(false)
            .with_initialization_script_for_main_only(policy, true)
            .with_navigation_handler(move |url| {
                if navigation_close.get() {
                    url == "about:blank"
                } else {
                    (url == OFFICIAL_URL || verification && url == "about:blank")
                        && !navigation_seen.replace(true)
                }
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, PageLoadEvent::Finished) {
                    if loaded_close.get() && url == "about:blank" {
                        enqueue(&loaded, epoch, HostEvent::Neutralized);
                    } else if url == OFFICIAL_URL || verification && url == "about:blank" {
                        enqueue(&loaded, epoch, HostEvent::PageReady);
                    }
                }
            })
            .with_on_web_content_process_terminate_handler(move || {
                enqueue(&failed, epoch, HostEvent::ProcessFailed)
            })
            .with_ipc_handler(move |request| {
                let captured_at = Instant::now();
                let url = request.uri().to_string();
                if !(url == OFFICIAL_URL || verification && url == "about:blank") {
                    return;
                }
                if request.body().len() > MAX_IPC {
                    enqueue(&ipc, epoch, HostEvent::Overflow);
                    return;
                }
                if verification {
                    enqueue(&ipc, epoch, HostEvent::Message(request.into_body()));
                    return;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(request.body()) else {
                    return;
                };
                if value["guard"].as_str() != Some(guard.as_str()) {
                    return;
                }
                if value["channel"].as_str() == Some("opentrickster.net.v1") {
                    enqueue_at(
                        &ipc,
                        epoch,
                        HostEvent::NetworkPage {
                            origin: url.trim_end_matches('/').to_owned(),
                            json: request.into_body(),
                        },
                        captured_at,
                    );
                    return;
                }
                match value["kind"].as_str() {
                    Some("fullscreen") => {
                        enqueue_at(&ipc, epoch, HostEvent::FullscreenRequested, captured_at)
                    }
                    Some("target") if probe => {
                        enqueue(&ipc, epoch, HostEvent::Message(request.into_body()))
                    }
                    _ => {}
                }
            });
        let builder = if let Some(script) = observer_script {
            builder.with_initialization_script_for_main_only(script.as_str(), true)
        } else {
            builder
        };
        let builder = if probe {
            builder.with_initialization_script(target_probe)
        } else {
            builder
        };
        let builder = if verification {
            builder.with_html(if matches!(source, HostSource::InputFixture) {
                include_str!("../../verification/input.html")
            } else {
                include_str!("../../verification/embedding.html")
            })
        } else {
            builder.with_url(OFFICIAL_URL)
        };
        let web = builder
            .build_as_child(window)
            .map_err(|_| "The embedded game could not be opened.")?;
        if !macos_native::is_child(&web, window) {
            return Err("The game view has no application parent.");
        }
        let manual_inbox = inbox.clone();
        let input = NativeInput::new(epoch.0, &web, move |at| {
            enqueue_at(&manual_inbox, epoch, HostEvent::PhysicalInput, at);
        })?;
        Ok(Self {
            epoch,
            inbox,
            child: Rc::new(RefCell::new(Some(web))),
            placement: Rc::new(RefCell::new(None)),
            input: Rc::new(RefCell::new(input)),
            neutralizing,
        })
    }

    pub fn place(
        child: &ChildSlot,
        placement: &Rc<RefCell<Option<GameRect>>>,
        layout: &Cell<u64>,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let rect = game_rect(
            bounds.origin.x.into(),
            bounds.origin.y.into(),
            bounds.size.width.into(),
            bounds.size.height.into(),
        );
        if *placement.borrow() == rect {
            return true;
        }
        layout.set(layout.get().saturating_add(1));
        let borrowed = child.borrow();
        let Some(web) = borrowed.as_ref() else {
            return true;
        };
        if let Some(rect) = rect {
            if web
                .set_bounds(Rect {
                    position: LogicalPosition::new(rect.x as f64, rect.y as f64).into(),
                    size: LogicalSize::new(rect.width as f64, rect.height as f64).into(),
                })
                .is_ok()
            {
                *placement.borrow_mut() = Some(rect);
                return web.set_visible(true).is_ok();
            }
            let _ = web.set_visible(false);
            *placement.borrow_mut() = None;
            return false;
        } else {
            let _ = web.set_visible(false);
            *placement.borrow_mut() = None;
        }
        true
    }

    pub fn neutralize(&self) -> bool {
        self.neutralizing.set(true);
        self.child
            .borrow()
            .as_ref()
            .is_some_and(|web| web.load_url("about:blank").is_ok())
    }

    pub fn close(&self) {
        self.inbox.deactivate(self.epoch);
        self.child.borrow_mut().take();
    }
}

impl Drop for EmbeddedHost {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_callbacks_cannot_fill_or_fail_the_replacement_inbox() {
        let inbox = Rc::new(HostQueue::new());
        inbox.activate(HostEpoch(1));
        enqueue(&inbox, HostEpoch(1), HostEvent::PageReady);
        inbox.activate(HostEpoch(2));
        for _ in 0..MAX_INBOX + 1 {
            enqueue(&inbox, HostEpoch(1), HostEvent::ProcessFailed);
        }
        enqueue(&inbox, HostEpoch(2), HostEvent::PageReady);
        let received = inbox.drain();
        assert_eq!(received.len(), 1);
        assert!(
            received[0].epoch == HostEpoch(2) && matches!(received[0].event, HostEvent::PageReady)
        );
    }
    #[test]
    fn an_active_overflow_is_explicit_and_close_revokes_callbacks() {
        let inbox = Rc::new(HostQueue::new());
        inbox.activate(HostEpoch(9));
        for _ in 0..MAX_INBOX + 1 {
            enqueue(&inbox, HostEpoch(9), HostEvent::PageReady);
        }
        let received = inbox.drain();
        assert_eq!(received.len(), 1);
        assert!(matches!(received[0].event, HostEvent::Overflow));
        inbox.deactivate(HostEpoch(9));
        enqueue(&inbox, HostEpoch(9), HostEvent::PageReady);
        assert!(inbox.drain().is_empty());
    }
}
