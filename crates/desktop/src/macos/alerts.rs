use objc2::runtime::Bool;
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNUserNotificationCenter,
};
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, Ordering},
};

const NOT_REQUESTED: u8 = 0;
const REQUESTING: u8 = 1;
const GRANTED: u8 = 2;
const DENIED: u8 = 3;
const UNAVAILABLE: u8 = 4;

pub struct Alerts {
    permission: Arc<AtomicU8>,
    next: Arc<AtomicU64>,
}
impl Alerts {
    pub fn new() -> Self {
        // UserNotifications requires an application bundle. Command-line verification uses the panel and Dock instead.
        let bundled = NSBundle::mainBundle()
            .bundleIdentifier()
            .is_some_and(|id| id.to_string() == "org.opentrickster.desktop");
        Self {
            permission: Arc::new(AtomicU8::new(if bundled {
                NOT_REQUESTED
            } else {
                UNAVAILABLE
            })),
            next: Arc::new(AtomicU64::new(1)),
        }
    }
    pub fn caption(&self) -> &'static str {
        match self.permission.load(Ordering::Acquire) {
            GRANTED => "macOS notifications allowed",
            DENIED => "Notifications denied. Alerts remain in the panel.",
            UNAVAILABLE => "macOS notifications unavailable. Alerts remain in the panel.",
            REQUESTING => "Waiting for macOS notification permission",
            _ => "macOS asks for notification permission before the first alert.",
        }
    }
    pub fn notify(&self, title: &'static str, body: &'static str) {
        match self.permission.load(Ordering::Acquire) {
            GRANTED => deliver(
                title,
                body,
                self.next.fetch_add(1, Ordering::Relaxed),
                self.permission.clone(),
            ),
            NOT_REQUESTED => {
                self.permission.store(REQUESTING, Ordering::Release);
                let permission = self.permission.clone();
                let next = self.next.clone();
                let completion = block2::RcBlock::new(move |granted: Bool, error: *mut NSError| {
                    if granted.as_bool() && error.is_null() {
                        permission.store(GRANTED, Ordering::Release);
                        deliver(
                            title,
                            body,
                            next.fetch_add(1, Ordering::Relaxed),
                            permission.clone(),
                        );
                    } else {
                        permission.store(DENIED, Ordering::Release);
                    }
                });
                UNUserNotificationCenter::currentNotificationCenter()
                    .requestAuthorizationWithOptions_completionHandler(
                        UNAuthorizationOptions::Alert,
                        &completion,
                    );
            }
            _ => {}
        }
    }
}
fn deliver(title: &str, body: &str, id: u64, permission: Arc<AtomicU8>) {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&format!("opentrickster-{id}")),
        &content,
        None,
    );
    let completion = block2::RcBlock::new(move |error: *mut NSError| {
        if !error.is_null() {
            permission.store(UNAVAILABLE, Ordering::Release);
        }
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&request, Some(&completion));
}
