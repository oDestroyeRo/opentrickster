use super::normal_input::{Context, Control, Target};
use objc2::{
    ClassType, MainThreadMarker,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Imp, Sel},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSResponder, NSView,
    NSWindowDidResignKeyNotification,
};
use objc2_foundation::{NSNotificationCenter, NSPoint, NSString};
use objc2_web_kit::WKWebView;
use opentrickster_net::{InputKey, NormalInput};
use std::{
    cell::Cell,
    ptr::NonNull,
    rc::Rc,
    sync::{
        Once, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};
use wry::{WebView, WebViewExtMacOS};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
}

type PressedButtons = unsafe extern "C-unwind" fn(&AnyClass, Sel) -> usize;

/// Mouse buttons that this app holds down through native events, as `pressedMouseButtons` bits.
static OWNED_BUTTONS: AtomicUsize = AtomicUsize::new(0);
static PHYSICAL_BUTTONS: OnceLock<PressedButtons> = OnceLock::new();

unsafe extern "C-unwind" fn pressed_buttons(class: &AnyClass, sel: Sel) -> usize {
    physical_buttons_from(class, sel) | OWNED_BUTTONS.load(Ordering::Relaxed)
}

fn physical_buttons_from(class: &AnyClass, sel: Sel) -> usize {
    // SAFETY: the saved implementation is AppKit's own, called with its class and selector.
    PHYSICAL_BUTTONS
        .get()
        .map_or(0, |physical| unsafe { physical(class, sel) })
}

/// The buttons held on the physical mouse, without any that this app holds.
fn physical_buttons() -> usize {
    physical_buttons_from(NSEvent::class(), sel!(pressedMouseButtons))
}

/// WebKit fills DOM `buttons` from `+[NSEvent pressedMouseButtons]`, which reports only the
/// physical mouse. Without this, the game sees an owned press with no button held and ignores it.
fn report_owned_buttons() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let Some(method) = NSEvent::class().class_method(sel!(pressedMouseButtons)) else {
            return;
        };
        // SAFETY: both functions take the receiver class and selector and return NSUInteger.
        // The original is saved before the replacement can run.
        unsafe {
            let _ = PHYSICAL_BUTTONS.set(std::mem::transmute::<Imp, PressedButtons>(
                method.implementation(),
            ));
            method.set_implementation(std::mem::transmute::<PressedButtons, Imp>(pressed_buttons));
        }
    });
}

pub(super) struct NativeInput {
    host: u64,
    pub layout: Rc<Cell<u64>>,
    physical: Rc<Cell<u64>>,
    monitor: Retained<AnyObject>,
    focus_monitor: Retained<AnyObject>,
    view: Retained<WKWebView>,
    pointer: NSPoint,
    left: Option<Retained<NSView>>,
    right: Option<Retained<NSView>>,
    keys: Vec<(u16, Retained<NSResponder>)>,
}
impl NativeInput {
    pub fn new(
        host: u64,
        web: &WebView,
        manual: impl Fn(Instant) + 'static,
    ) -> Result<Self, &'static str> {
        let window = web
            .webview()
            .window()
            .ok_or("The input view has no window.")?;
        report_owned_buttons();
        let physical = Rc::new(Cell::new(0u64));
        let changed = physical.clone();
        let monitored = web.webview();
        let shortcut_keys = Cell::new(0u128);
        let handler = block2::RcBlock::new(move |event: NonNull<NSEvent>| {
            let at = Instant::now();
            // SAFETY: AppKit lends this event for the duration of the monitor callback.
            let incoming = unsafe { event.as_ref() };
            let kind = match incoming.r#type() {
                NSEventType::KeyDown | NSEventType::KeyUp => Physical::Key,
                NSEventType::FlagsChanged => Physical::Modifier,
                NSEventType::MouseMoved
                | NSEventType::LeftMouseDragged
                | NSEventType::RightMouseDragged
                | NSEventType::OtherMouseDragged
                | NSEventType::ScrollWheel => Physical::Pointer,
                _ => Physical::Button,
            };
            let shortcut = if kind == Physical::Key {
                let bit = 1u128
                    .checked_shl(u32::from(incoming.keyCode()))
                    .unwrap_or(0);
                let modified = incoming
                    .modifierFlags()
                    .intersects(NSEventModifierFlags::Command | NSEventModifierFlags::Control);
                let shortcut = modified || shortcut_keys.get() & bit != 0;
                if incoming.r#type() == NSEventType::KeyDown && modified {
                    shortcut_keys.set(shortcut_keys.get() | bit);
                }
                if incoming.r#type() == NSEventType::KeyUp {
                    shortcut_keys.set(shortcut_keys.get() & !bit);
                }
                shortcut
            } else {
                false
            };
            let in_game = if matches!(kind, Physical::Key | Physical::Modifier) {
                monitored
                    .window()
                    .and_then(|window| window.firstResponder())
                    .and_then(|responder| responder.downcast::<NSView>().ok())
                    .is_some_and(|view| view.isDescendantOf(&monitored))
            } else {
                let point = monitored.convertPoint_fromView(incoming.locationInWindow(), None);
                let bounds = monitored.bounds();
                point.x >= bounds.origin.x
                    && point.y >= bounds.origin.y
                    && point.x < bounds.origin.x + bounds.size.width
                    && point.y < bounds.origin.y + bounds.size.height
            };
            let (fence, gameplay) = classify(kind, in_game, shortcut);
            if fence {
                changed.set(changed.get().saturating_add(1));
            }
            if gameplay {
                manual(at);
            }
            event.as_ptr()
        });
        // SAFETY: AppKit copies this main-thread block; it returns its borrowed event unchanged.
        // Direct responder dispatch below bypasses NSApplication's physical-event monitors.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::KeyDown
                    | NSEventMask::KeyUp
                    | NSEventMask::FlagsChanged
                    | NSEventMask::LeftMouseDown
                    | NSEventMask::LeftMouseUp
                    | NSEventMask::RightMouseDown
                    | NSEventMask::RightMouseUp
                    | NSEventMask::OtherMouseDown
                    | NSEventMask::OtherMouseUp
                    | NSEventMask::MouseMoved
                    | NSEventMask::LeftMouseDragged
                    | NSEventMask::RightMouseDragged
                    | NSEventMask::OtherMouseDragged
                    | NSEventMask::ScrollWheel,
                &handler,
            )
        }
        .ok_or("The native input fence could not be installed.")?;
        window.setAcceptsMouseMovedEvents(true);
        let focus_changed = physical.clone();
        let focus_handler = block2::RcBlock::new(move |_| {
            focus_changed.set(focus_changed.get().saturating_add(1));
        });
        // SAFETY: the center copies the block and delivers this window notification on
        // the posting main thread. The opaque NSObjectProtocol token is an Objective-C object.
        let focus_monitor = unsafe {
            Retained::cast_unchecked(
                NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                    Some(NSWindowDidResignKeyNotification),
                    Some(window.as_ref()),
                    None,
                    &focus_handler,
                ),
            )
        };
        Ok(Self {
            host,
            layout: Rc::new(Cell::new(1)),
            physical,
            monitor,
            focus_monitor,
            view: web.webview().into_super(),
            pointer: NSPoint::new(0., 0.),
            left: None,
            right: None,
            keys: Vec::new(),
        })
    }
    pub fn physical_revision(&self) -> u64 {
        self.physical.get()
    }

    /// Account-free fixture only: enqueue through AppKit so its monitor sees input.
    pub fn verify_manual_event(&self) -> bool {
        let (Some(window), Some(mtm)) = (self.view.window(), MainThreadMarker::new()) else {
            return false;
        };
        let text = NSString::from_str("x");
        let Some(event) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyUp, NSPoint::new(0.,0.), NSEventModifierFlags::empty(), 0., window.windowNumber(), None, &text, &text, false, 7,
        ) else { return false; };
        NSApplication::sharedApplication(mtm).postEvent_atStart(&event, false);
        true
    }

    fn focused(&self) -> bool {
        let Some(window) = self.view.window() else {
            return false;
        };
        window.isKeyWindow() && window.attachedSheet().is_none()
            // SAFETY: the retained main-thread WKWebView supplies these scalar properties.
            && unsafe { self.view.pageZoom() == 1. && self.view.magnification() == 1. }
            && !self.view.isHiddenOrHasHiddenAncestor()
            && window.firstResponder().and_then(|r| r.downcast::<NSView>().ok())
                .is_some_and(|r| r.isDescendantOf(&self.view))
            && NSEvent::modifierFlags_class().intersection(
                NSEventModifierFlags::Shift | NSEventModifierFlags::Control
                    | NSEventModifierFlags::Option | NSEventModifierFlags::Command
                    | NSEventModifierFlags::CapsLock | NSEventModifierFlags::Function,
            ).is_empty()
            // SAFETY: read-only combined-session hardware state. Never post a global event.
            && !(0..128).any(|code| unsafe { CGEventSourceKeyState(0, code) })
            && physical_buttons() == 0
    }

    fn mouse(&mut self, input: NormalInput, cleanup: bool) -> Result<(), ()> {
        let window = self.view.window().ok_or(())?;
        let (kind, down) = match input {
            NormalInput::PointerMove { x, y } => {
                let bounds = self.view.bounds();
                if !x.is_finite()
                    || !y.is_finite()
                    || x < 0.
                    || y < 0.
                    || x >= bounds.size.width
                    || y >= bounds.size.height
                {
                    return Err(());
                }
                // CSS viewport and AppKit point units agree under the native zoom fence.
                self.pointer = NSPoint::new(
                    x + bounds.origin.x,
                    if self.view.isFlipped() {
                        y + bounds.origin.y
                    } else {
                        bounds.origin.y + bounds.size.height - y
                    },
                );
                (NSEventType::MouseMoved, false)
            }
            NormalInput::LeftDown => (NSEventType::LeftMouseDown, true),
            NormalInput::LeftUp => (NSEventType::LeftMouseUp, false),
            NormalInput::RightDown => (NSEventType::RightMouseDown, true),
            NormalInput::RightUp => (NSEventType::RightMouseUp, false),
            _ => return Err(()),
        };
        let location = self.view.convertPoint_toView(self.pointer, None);
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind, location, NSEventModifierFlags::empty(), 0., window.windowNumber(), None, 0,
            if kind == NSEventType::MouseMoved { 0 } else { 1 }, if down { 1. } else { 0. },
        ).ok_or(())?;
        let target = match input {
            NormalInput::LeftUp => self
                .left
                .clone()
                .or_else(|| cleanup.then(|| self.view.clone().into_super())),
            NormalInput::RightUp => self
                .right
                .clone()
                .or_else(|| cleanup.then(|| self.view.clone().into_super())),
            _ => {
                // SAFETY: the retained view and its parent are borrowed on the main thread.
                let parent = unsafe { self.view.superview() }.ok_or(())?;
                let point = parent.convertPoint_fromView(self.pointer, Some(&self.view));
                self.view
                    .hitTest(point)
                    .filter(|view| view.isDescendantOf(&self.view))
            }
        }
        .ok_or(())?;
        if !cleanup && !self.focused() {
            return Err(());
        }
        match input {
            NormalInput::PointerMove { .. } => window.sendEvent(&event),
            NormalInput::LeftDown => {
                self.left = Some(target.clone());
                OWNED_BUTTONS.fetch_or(1, Ordering::Relaxed);
                target.mouseDown(&event);
            }
            NormalInput::LeftUp => {
                OWNED_BUTTONS.fetch_and(!1, Ordering::Relaxed);
                target.mouseUp(&event);
                self.left = None;
            }
            NormalInput::RightDown => {
                self.right = Some(target.clone());
                OWNED_BUTTONS.fetch_or(2, Ordering::Relaxed);
                target.rightMouseDown(&event);
            }
            NormalInput::RightUp => {
                OWNED_BUTTONS.fetch_and(!2, Ordering::Relaxed);
                target.rightMouseUp(&event);
                self.right = None;
            }
            _ => return Err(()),
        }
        Ok(())
    }

    fn key(&mut self, key: InputKey, down: bool, cleanup: bool) -> Result<(), ()> {
        let code = key_code(key).ok_or(())?;
        let window = self.view.window().ok_or(())?;
        let responder = if down {
            window.firstResponder().ok_or(())?
        } else {
            self.keys
                .iter()
                .find(|(held, _)| *held == code)
                .map(|(_, r)| r.clone())
                .or_else(|| cleanup.then(|| self.view.clone().into_super().into_super()))
                .ok_or(())?
        };
        let text = match key {
            InputKey::Character(c) if c.is_ascii_alphabetic() => c.to_ascii_lowercase().to_string(),
            InputKey::Escape => "\u{1b}".into(),
            InputKey::Space => " ".into(),
            InputKey::Enter => "\r".into(),
            InputKey::Tab => "\t".into(),
            InputKey::ArrowUp => "\u{f700}".into(),
            InputKey::ArrowDown => "\u{f701}".into(),
            InputKey::ArrowLeft => "\u{f702}".into(),
            InputKey::ArrowRight => "\u{f703}".into(),
            _ => return Err(()),
        };
        let characters = NSString::from_str(&text);
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            if down { NSEventType::KeyDown } else { NSEventType::KeyUp }, NSPoint::new(0., 0.),
            NSEventModifierFlags::empty(), 0., window.windowNumber(), None,
            &characters, &characters, false, code,
        ).ok_or(())?;
        if !cleanup && !self.focused() {
            return Err(());
        }
        if down {
            self.keys.push((code, responder.clone()));
            responder.keyDown(&event);
        } else {
            responder.keyUp(&event);
            self.keys.retain(|(held, _)| *held != code);
        }
        Ok(())
    }
}
impl Target for NativeInput {
    fn context(&self) -> Context {
        Context {
            host: self.host,
            layout: self.layout.get(),
            physical: self.physical.get(),
            focused: self.focused(),
        }
    }
    fn physical_held(&self, control: Control) -> bool {
        match control {
            Control::Left => physical_buttons() & 1 != 0,
            Control::Right => physical_buttons() & 2 != 0,
            // SAFETY: Combined-session state is read-only; no event tap, posting, or permission change.
            Control::Key(key) => {
                key_code(key).is_none_or(|code| unsafe { CGEventSourceKeyState(0, code) })
            }
        }
    }
    fn dispatch(&mut self, input: NormalInput, cleanup: bool) -> Result<Instant, ()> {
        match input {
            NormalInput::KeyDown(key) => self.key(key, true, cleanup)?,
            NormalInput::KeyUp(key) => self.key(key, false, cleanup)?,
            other => self.mouse(other, cleanup)?,
        }
        Ok(Instant::now())
    }
}
impl Drop for NativeInput {
    fn drop(&mut self) {
        // SAFETY: this is the token returned by AppKit; removal occurs on its main thread.
        unsafe {
            NSEvent::removeMonitor(&self.monitor);
        }
        // SAFETY: this token belongs to the center above and remains retained until removal.
        unsafe {
            NSNotificationCenter::defaultCenter().removeObserver(&self.focus_monitor);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Physical {
    Key,
    Modifier,
    Button,
    Pointer,
}

/// Returns whether a physical event bumps the input fence and whether it is manual gameplay.
/// Keys, modifiers and buttons fence anywhere. Pointer motion and scrolling fence only over
/// the game, so moving the pointer over the dashboard cannot cancel an action.
fn classify(kind: Physical, in_game: bool, shortcut: bool) -> (bool, bool) {
    let fence = in_game || kind != Physical::Pointer;
    let gameplay = in_game && !shortcut && kind != Physical::Modifier;
    (fence, gameplay)
}

fn key_code(key: InputKey) -> Option<u16> {
    key.dom_code()?;
    Some(match key {
        InputKey::Character(c) => [
            0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7,
            16, 6,
        ][c.to_ascii_uppercase() as usize - 'A' as usize],
        InputKey::Escape => 53,
        InputKey::Space => 49,
        InputKey::Enter => 36,
        InputKey::Tab => 48,
        InputKey::ArrowLeft => 123,
        InputKey::ArrowRight => 124,
        InputKey::ArrowDown => 125,
        InputKey::ArrowUp => 126,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_pointer_motion_neither_fences_nor_reports_gameplay() {
        assert_eq!(classify(Physical::Pointer, false, false), (false, false));
        assert_eq!(classify(Physical::Pointer, true, false), (true, true));
    }

    #[test]
    fn keys_modifiers_and_buttons_fence_anywhere_but_only_game_input_is_gameplay() {
        for kind in [Physical::Key, Physical::Modifier, Physical::Button] {
            assert_eq!(classify(kind, false, false), (true, false));
        }
        assert_eq!(classify(Physical::Key, true, false), (true, true));
        assert_eq!(classify(Physical::Key, true, true), (true, false));
        assert_eq!(classify(Physical::Modifier, true, false), (true, false));
        assert_eq!(classify(Physical::Button, true, false), (true, true));
    }
}
