use super::normal_input::{Context, Control, Target};
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSResponder, NSView,
    NSWindowDidResignKeyNotification,
};
use objc2_foundation::{NSNotificationCenter, NSPoint, NSString};
use objc2_web_kit::WKWebView;
use opentrickster_net::{InputKey, NormalInput};
use std::{cell::Cell, ptr::NonNull, rc::Rc, time::Instant};
use wry::{WebView, WebViewExtMacOS};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
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
        let physical = Rc::new(Cell::new(0u64));
        let changed = physical.clone();
        let monitored = web.webview();
        let shortcut_keys = Cell::new(0u128);
        let handler = block2::RcBlock::new(move |event: NonNull<NSEvent>| {
            let at = Instant::now();
            changed.set(changed.get().saturating_add(1));
            // SAFETY: AppKit lends this event for the duration of the monitor callback.
            let incoming = unsafe { event.as_ref() };
            let shortcut = if matches!(incoming.r#type(), NSEventType::KeyDown | NSEventType::KeyUp)
            {
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
            let in_game = if matches!(
                incoming.r#type(),
                NSEventType::KeyDown | NSEventType::KeyUp | NSEventType::FlagsChanged
            ) {
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
            if in_game && !shortcut && incoming.r#type() != NSEventType::FlagsChanged {
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
            && NSEvent::pressedMouseButtons() == 0
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
                target.mouseDown(&event);
            }
            NormalInput::LeftUp => {
                target.mouseUp(&event);
                self.left = None;
            }
            NormalInput::RightDown => {
                self.right = Some(target.clone());
                target.rightMouseDown(&event);
            }
            NormalInput::RightUp => {
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
            Control::Left => NSEvent::pressedMouseButtons() & 1 != 0,
            Control::Right => NSEvent::pressedMouseButtons() & 2 != 0,
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
