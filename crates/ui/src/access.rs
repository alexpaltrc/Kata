//! What assistive technologies such as Narrator know of a window's controls, through
//! UI Automation. The window describes its controls after each frame; Windows asks
//! about them, from any thread, and the actions it asks for come back to the window
//! as `ACT` messages.

#![allow(
    non_upper_case_globals,
    reason = "UI Automation's constants keep Windows' names where they're matched"
)]

use std::{
    mem,
    ptr::{from_ref, null_mut},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use windows::{
    Win32::{
        Foundation::{E_OUTOFMEMORY, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::{ClientToScreen, ScreenToClient},
        System::{
            Com::SAFEARRAY,
            Ole::{SafeArrayCreateVector, SafeArrayPutElement},
            Variant::{VARIANT, VT_I4},
        },
        UI::{
            Accessibility::*,
            WindowsAndMessaging::{PostMessageW, WM_APP},
        },
    },
    core::{BOOL, BSTR, Error, HRESULT, IUnknown, IUnknownImpl, PCWSTR, Result, implement},
};

use crate::{
    controls::Action,
    paint::{Area, Point},
};

/// Carries an action from an assistive technology to the window: `wparam` is the
/// control's number, and `action` reads `lparam`.
pub const ACT: u32 = WM_APP + 0x100;

/// What an `ACT` message asks of its control.
pub fn action(lparam: LPARAM) -> Action {
    usize::try_from(lparam.0).map_or(Action::Activate, Action::To)
}

/// A control, or one of its items, as assistive technologies know it.
#[derive(Clone, Debug)]
pub struct Element {
    /// The window's own number for the control.
    pub control: usize,
    pub name: String,
    pub role: Role,
    /// In DIPs, from the client area's top-left corner.
    pub area: Area,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Button {
        enabled: bool,
    },
    /// An on/off switch.
    Toggle {
        on: bool,
    },
    /// At one of `count` steps, the current one called `text`.
    Slider {
        value: usize,
        count: usize,
        text: String,
    },
    /// One of a control's items, of which one is selected.
    Choice {
        index: usize,
        selected: bool,
    },
}

/// An element's identity: its control and, for an item, its place plus one.
type Id = (usize, usize);

impl Element {
    fn id(&self) -> Id {
        match self.role {
            Role::Choice { index, .. } => (self.control, index + 1),
            _ => (self.control, 0),
        }
    }
}

/// A window's link to assistive technologies.
pub struct Access {
    tree: Shared,
    /// The window's element, once an assistive technology asked for it.
    root: Option<IRawElementProviderSimple>,
}

impl Access {
    pub fn new(hwnd: HWND) -> Self {
        let tree = Tree {
            hwnd: hwnd.0 as isize,
            scale: 1.0,
            elements: Vec::new(),
            focus: None,
        };
        Self {
            tree: Arc::new(Mutex::new(tree)),
            root: None,
        }
    }

    /// Answers `WM_GETOBJECT` when it asks for UI Automation.
    pub fn answer(&mut self, hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if lparam.0 as i32 != UiaRootObjectId {
            return None;
        }
        let root = self
            .root
            .get_or_insert_with(|| Root(self.tree.clone()).into());
        Some(unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &*root) })
    }

    /// Whether an assistive technology follows the window, and so its controls.
    pub fn is_wanted(&self) -> bool {
        self.root.is_some()
    }

    /// Takes the window's controls as they are now, the one numbered `focus` with
    /// the keyboard focus, and tells assistive technologies what changed. `scale`
    /// is the window's pixels per DIP.
    pub fn update(&mut self, elements: Vec<Element>, focus: Option<usize>, scale: f32) {
        let Some(root) = &self.root else {
            return;
        };
        // A control with items has the focus on its selected one.
        let focus = focus.and_then(|control| {
            let focused = |element: &&Element| {
                element.control == control
                    && !matches!(
                        element.role,
                        Role::Choice {
                            selected: false,
                            ..
                        }
                    )
            };
            elements.iter().find(focused).map(Element::id)
        });
        let mut events = Vec::new();
        {
            let mut tree = lock(&self.tree);
            tree.scale = scale;
            let old = mem::replace(&mut tree.elements, elements);
            let ids = |elements: &[Element]| elements.iter().map(Element::id).collect::<Vec<_>>();
            if ids(&old) == ids(&tree.elements) {
                for (old, new) in old.iter().zip(&tree.elements) {
                    changes(old, new, &mut events);
                }
            } else {
                events.push(Event::Reshaped);
            }
            if mem::replace(&mut tree.focus, focus) != focus
                && let Some(id) = focus
            {
                events.push(Event::Focus(id));
            }
        }
        // Raised without the lock: Windows may ask about the elements meanwhile.
        for event in events {
            let _ = unsafe {
                match event {
                    Event::Reshaped => UiaRaiseStructureChangedEvent(
                        root,
                        StructureChangeType_ChildrenInvalidated,
                        null_mut(),
                        0,
                    ),
                    Event::Property(id, property, old, new) => {
                        let node: IRawElementProviderSimple = node(&self.tree, id);
                        UiaRaiseAutomationPropertyChangedEvent(&node, property, &old, &new)
                    }
                    Event::Selected(id) => UiaRaiseAutomationEvent(
                        &node::<IRawElementProviderSimple>(&self.tree, id),
                        UIA_SelectionItem_ElementSelectedEventId,
                    ),
                    Event::Focus(id) => UiaRaiseAutomationEvent(
                        &node::<IRawElementProviderSimple>(&self.tree, id),
                        UIA_AutomationFocusChangedEventId,
                    ),
                }
            };
        }
    }

    /// Lets assistive technologies go, as the window is destroyed.
    pub fn close(&mut self, hwnd: HWND) {
        lock(&self.tree).elements.clear();
        if let Some(root) = self.root.take() {
            unsafe {
                UiaReturnRawElementProvider(
                    hwnd,
                    WPARAM(0),
                    LPARAM(0),
                    None::<&IRawElementProviderSimple>,
                );
                let _ = UiaDisconnectProvider(&root);
            }
        }
    }
}

/// What changed for assistive technologies.
enum Event {
    /// Elements came or went.
    Reshaped,
    /// A property of an element went from one value to another.
    Property(Id, UIA_PROPERTY_ID, VARIANT, VARIANT),
    Selected(Id),
    Focus(Id),
}

/// Adds what changed from `old` to `new`, the same element, to `events`.
fn changes(old: &Element, new: &Element, events: &mut Vec<Event>) {
    let id = new.id();
    let mut property = |property, old: VARIANT, new: VARIANT| {
        events.push(Event::Property(id, property, old, new));
    };
    if old.name != new.name {
        property(
            UIA_NamePropertyId,
            old.name.as_str().into(),
            new.name.as_str().into(),
        );
    }
    match (&old.role, &new.role) {
        (Role::Button { enabled: was }, Role::Button { enabled }) if was != enabled => {
            property(UIA_IsEnabledPropertyId, (*was).into(), (*enabled).into());
        }
        (Role::Toggle { on: was }, Role::Toggle { on }) if was != on => {
            let state = |on| VARIANT::from(toggle_state(on).0);
            property(UIA_ToggleToggleStatePropertyId, state(*was), state(*on));
        }
        (
            Role::Slider {
                value: was,
                text: said,
                ..
            },
            Role::Slider { value, text, .. },
        ) if was != value => {
            let (was, value) = (*was as f64, *value as f64);
            property(UIA_RangeValueValuePropertyId, was.into(), value.into());
            property(
                UIA_ValueValuePropertyId,
                said.as_str().into(),
                text.as_str().into(),
            );
        }
        (
            Role::Choice {
                selected: false, ..
            },
            Role::Choice { selected: true, .. },
        ) => {
            events.push(Event::Selected(id));
        }
        _ => {}
    }
}

fn toggle_state(on: bool) -> ToggleState {
    if on { ToggleState_On } else { ToggleState_Off }
}

/// What the window shares with its providers, which Windows may call from any thread.
type Shared = Arc<Mutex<Tree>>;

struct Tree {
    /// The window, as a number that can go between threads.
    hwnd: isize,
    /// Pixels per DIP.
    scale: f32,
    /// In the order Tab takes.
    elements: Vec<Element>,
    focus: Option<Id>,
}

impl Tree {
    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    /// Where the element `id` is among the elements, while it's there.
    fn position(&self, id: Id) -> Result<usize> {
        self.elements
            .iter()
            .position(|element| element.id() == id)
            .ok_or_else(|| HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32).into())
    }

    fn get(&self, id: Id) -> Result<&Element> {
        Ok(&self.elements[self.position(id)?])
    }

    /// A point on the screen, in pixels, as DIPs in the client area.
    fn client(&self, (x, y): (f64, f64)) -> Point {
        let mut point = POINT {
            x: x as i32,
            y: y as i32,
        };
        let _ = unsafe { ScreenToClient(self.hwnd(), &mut point) };
        (point.x as f32 / self.scale, point.y as f32 / self.scale)
    }

    /// `area`, in DIPs in the client area, on the screen in pixels.
    fn screen(&self, area: Area) -> UiaRect {
        let mut origin = POINT::default();
        let _ = unsafe { ClientToScreen(self.hwnd(), &mut origin) };
        let scale = f64::from(self.scale);
        UiaRect {
            left: f64::from(origin.x) + f64::from(area.left) * scale,
            top: f64::from(origin.y) + f64::from(area.top) * scale,
            width: f64::from(area.width) * scale,
            height: f64::from(area.height) * scale,
        }
    }
}

fn lock(tree: &Shared) -> MutexGuard<'_, Tree> {
    tree.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A new provider for the element `id`.
fn node<I>(tree: &Shared, id: Id) -> I
where
    Node: Into<I>,
{
    Node {
        tree: tree.clone(),
        id,
    }
    .into()
}

/// Windows calls the providers on the window's thread, through COM.
const OPTIONS: ProviderOptions =
    ProviderOptions(ProviderOptions_ServerSideProvider.0 | ProviderOptions_UseComThreading.0);

/// "Nothing", where UI Automation asks for an element or a pattern.
fn none<T>() -> Result<T> {
    Err(Error::empty())
}

/// The window itself, whose children are the elements. Windows gives it the window's
/// name and place.
#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct Root(Shared);

impl IRawElementProviderSimple_Impl for Root_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(OPTIONS)
    }

    fn GetPatternProvider(&self, _pattern: UIA_PATTERN_ID) -> Result<IUnknown> {
        none()
    }

    fn GetPropertyValue(&self, _property: UIA_PROPERTY_ID) -> Result<VARIANT> {
        Ok(VARIANT::default())
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        let hwnd = lock(&self.0).hwnd();
        unsafe { UiaHostProviderFromHwnd(hwnd) }
    }
}

impl IRawElementProviderFragment_Impl for Root_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let tree = lock(&self.0);
        let child = match direction {
            NavigateDirection_FirstChild => tree.elements.first(),
            NavigateDirection_LastChild => tree.elements.last(),
            _ => None,
        };
        child.map_or_else(none, |child| Ok(node(&self.0, child.id())))
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Ok(null_mut())
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        Ok(UiaRect::default())
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(null_mut())
    }

    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        Ok(self.to_interface())
    }
}

impl IRawElementProviderFragmentRoot_Impl for Root_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let tree = lock(&self.0);
        let point = tree.client((x, y));
        let hit = tree
            .elements
            .iter()
            .find(|element| element.area.contains(point));
        hit.map_or_else(none, |hit| Ok(node(&self.0, hit.id())))
    }

    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        let focus = lock(&self.0).focus;
        focus.map_or_else(none, |id| Ok(node(&self.0, id)))
    }
}

/// One element, which may be gone by the time Windows asks about it.
#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IInvokeProvider,
    IToggleProvider,
    IRangeValueProvider,
    IValueProvider,
    ISelectionItemProvider
)]
struct Node {
    tree: Shared,
    id: Id,
}

impl Node {
    fn with<T>(&self, read: impl FnOnce(&Tree, &Element) -> T) -> Result<T> {
        let tree = lock(&self.tree);
        let element = tree.get(self.id)?;
        Ok(read(&tree, element))
    }

    fn role(&self) -> Result<Role> {
        self.with(|_, element| element.role.clone())
    }

    /// Has the window carry out `action` on the element's control.
    fn act(&self, action: Action) -> Result<()> {
        let lparam = match action {
            Action::To(step) => LPARAM(step as isize),
            _ => LPARAM(-1),
        };
        let (hwnd, control) = self.with(|tree, element| (tree.hwnd(), element.control))?;
        unsafe { PostMessageW(Some(hwnd), ACT, WPARAM(control), lparam) }
    }
}

impl IRawElementProviderSimple_Impl for Node_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(OPTIONS)
    }

    fn GetPatternProvider(&self, pattern: UIA_PATTERN_ID) -> Result<IUnknown> {
        Ok(match (self.role()?, pattern) {
            (Role::Button { .. }, UIA_InvokePatternId) => {
                self.to_interface::<IInvokeProvider>().into()
            }
            (Role::Toggle { .. }, UIA_TogglePatternId) => {
                self.to_interface::<IToggleProvider>().into()
            }
            (Role::Slider { .. }, UIA_RangeValuePatternId) => {
                self.to_interface::<IRangeValueProvider>().into()
            }
            // Says the step's name, where the range only has its number.
            (Role::Slider { .. }, UIA_ValuePatternId) => {
                self.to_interface::<IValueProvider>().into()
            }
            (Role::Choice { .. }, UIA_SelectionItemPatternId) => {
                self.to_interface::<ISelectionItemProvider>().into()
            }
            _ => return none(),
        })
    }

    fn GetPropertyValue(&self, property: UIA_PROPERTY_ID) -> Result<VARIANT> {
        self.with(|tree, element| {
            let enabled = element.role != Role::Button { enabled: false };
            match property {
                UIA_NamePropertyId => element.name.as_str().into(),
                UIA_ControlTypePropertyId => {
                    let control_type = match element.role {
                        // Toggle switches are buttons that toggle, as in WinUI.
                        Role::Button { .. } | Role::Toggle { .. } => UIA_ButtonControlTypeId,
                        Role::Slider { .. } => UIA_SliderControlTypeId,
                        Role::Choice { .. } => UIA_TabItemControlTypeId,
                    };
                    control_type.0.into()
                }
                UIA_IsEnabledPropertyId | UIA_IsKeyboardFocusablePropertyId => enabled.into(),
                UIA_HasKeyboardFocusPropertyId => (tree.focus == Some(self.id)).into(),
                _ => VARIANT::default(),
            }
        })
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        none()
    }
}

impl IRawElementProviderFragment_Impl for Node_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        if direction == NavigateDirection_Parent {
            return Ok(Root(self.tree.clone()).into());
        }
        let tree = lock(&self.tree);
        let index = tree.position(self.id)?;
        let sibling = match direction {
            NavigateDirection_NextSibling => tree.elements.get(index + 1),
            NavigateDirection_PreviousSibling => index
                .checked_sub(1)
                .and_then(|index| tree.elements.get(index)),
            _ => None,
        };
        sibling.map_or_else(none, |sibling| Ok(node(&self.tree, sibling.id())))
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        lock(&self.tree).position(self.id)?;
        let id = [
            UiaAppendRuntimeId as i32,
            self.id.0 as i32,
            self.id.1 as i32,
        ];
        unsafe {
            let array = SafeArrayCreateVector(VT_I4, 0, id.len() as u32);
            if array.is_null() {
                return Err(E_OUTOFMEMORY.into());
            }
            for (index, part) in (0..).zip(&id) {
                SafeArrayPutElement(array, &index, from_ref(part).cast())?;
            }
            Ok(array)
        }
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        self.with(|tree, element| tree.screen(element.area))
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(null_mut())
    }

    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        Ok(Root(self.tree.clone()).into())
    }
}

impl IInvokeProvider_Impl for Node_Impl {
    fn Invoke(&self) -> Result<()> {
        self.act(Action::Activate)
    }
}

impl IToggleProvider_Impl for Node_Impl {
    fn Toggle(&self) -> Result<()> {
        self.act(Action::Activate)
    }

    fn ToggleState(&self) -> Result<ToggleState> {
        Ok(toggle_state(self.role()? == Role::Toggle { on: true }))
    }
}

impl IRangeValueProvider_Impl for Node_Impl {
    fn SetValue(&self, value: f64) -> Result<()> {
        self.act(Action::To(value.round() as usize))
    }

    fn Value(&self) -> Result<f64> {
        match self.role()? {
            Role::Slider { value, .. } => Ok(value as f64),
            _ => Ok(0.0),
        }
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        Ok(false.into())
    }

    fn Maximum(&self) -> Result<f64> {
        match self.role()? {
            Role::Slider { count, .. } => Ok(count.saturating_sub(1) as f64),
            _ => Ok(0.0),
        }
    }

    fn Minimum(&self) -> Result<f64> {
        Ok(0.0)
    }

    fn LargeChange(&self) -> Result<f64> {
        Ok(1.0)
    }

    fn SmallChange(&self) -> Result<f64> {
        Ok(1.0)
    }
}

/// The slider's step by name, which only the range can change.
impl IValueProvider_Impl for Node_Impl {
    fn SetValue(&self, _value: &PCWSTR) -> Result<()> {
        Err(HRESULT(UIA_E_INVALIDOPERATION as i32).into())
    }

    fn Value(&self) -> Result<BSTR> {
        match self.role()? {
            Role::Slider { text, .. } => Ok(BSTR::from(text)),
            _ => Ok(BSTR::new()),
        }
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        Ok(true.into())
    }
}

impl ISelectionItemProvider_Impl for Node_Impl {
    fn Select(&self) -> Result<()> {
        // An item's id holds its place plus one.
        self.act(Action::To(self.id.1.saturating_sub(1)))
    }

    /// One item is always selected, alone.
    fn AddToSelection(&self) -> Result<()> {
        Err(HRESULT(UIA_E_INVALIDOPERATION as i32).into())
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Err(HRESULT(UIA_E_INVALIDOPERATION as i32).into())
    }

    fn IsSelected(&self) -> Result<BOOL> {
        let selected = matches!(self.role()?, Role::Choice { selected: true, .. });
        Ok(selected.into())
    }

    fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> {
        none()
    }
}
