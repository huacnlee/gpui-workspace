use std::sync::Arc;

use gpui::{
    deferred, div, prelude::FluentBuilder as _, px, AnyView, App, AppContext as _, Axis, Context,
    Entity, EntityId, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement as _, Pixels, Render, StatefulInteractiveElement,
    StyleRefinement, Styled as _, Subscription, WeakEntity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ui::{prelude::Window, theme::ActiveTheme, IconName, StyledExt as _};

const RESIZE_HANDLE_SIZE: Pixels = Pixels(6.);

use crate::{DraggedDock, Event};

use super::workspace::Workspace;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DockPosition {
    Left,
    Bottom,
    Right,
}

impl DockPosition {
    pub fn axis(&self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Bottom => Axis::Vertical,
        }
    }
}

pub enum PanelEvent {
    ZoomIn,
    ZoomOut,
    Activate,
    Close,
}

#[allow(unused)]
pub trait Panel: Focusable + EventEmitter<PanelEvent> + Render + Sized {
    fn persistent_name() -> &'static str;
    /// Return the position of the panel.
    fn position(&self, window: &Window, cx: &App) -> DockPosition;
    /// Return true if the panel can be positioned at the given position.
    fn can_position(&self, position: DockPosition) -> bool {
        true
    }
    /// Set the position of the panel.
    fn set_position(
        &mut self,
        position: DockPosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
    }
    /// Return the size of the panel.
    fn size(&self, window: &Window, cx: &App) -> Pixels;
    /// Set the size of the panel.
    fn set_size(&mut self, size: Option<Pixels>, window: &mut Window, cx: &mut Context<Self>) {}
    /// Set the active state of the panel.
    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {}
    fn icon(&self, _window: &Window, cx: &App) -> Option<IconName> {
        None
    }
    fn is_zoomed(&self, _window: &Window, cx: &App) -> bool {
        false
    }
    fn set_zoomed(&mut self, _zoomed: bool, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn starts_open(&self, _window: &Window, cx: &App) -> bool {
        true
    }
}

pub trait PanelHandle: Send + Sync {
    fn id(&self) -> EntityId;
    fn persistent_name(&self) -> &'static str;
    fn position(&self, window: &Window, cx: &App) -> DockPosition;
    fn can_position(&self, position: DockPosition, window: &Window, cx: &App) -> bool;
    fn set_position(&self, position: DockPosition, window: &mut Window, cx: &mut App);
    fn size(&self, window: &Window, cx: &App) -> Pixels;
    fn set_size(&self, size: Option<Pixels>, window: &mut Window, cx: &mut App);
    fn icon(&self, window: &Window, cx: &App) -> Option<IconName>;
    fn panel_focus_handle(&self, cx: &App) -> FocusHandle;
    fn set_active(&self, active: bool, window: &mut Window, cx: &mut App);
    fn is_zoomed(&self, window: &Window, cx: &App) -> bool;
    fn set_zoomed(&self, zoomed: bool, window: &mut Window, cx: &mut App);
    fn to_any(&self) -> AnyView;
}

impl<T> PanelHandle for Entity<T>
where
    T: Panel,
{
    fn id(&self) -> EntityId {
        Entity::entity_id(self)
    }

    fn persistent_name(&self) -> &'static str {
        T::persistent_name()
    }

    fn position(&self, window: &Window, cx: &App) -> DockPosition {
        self.read(cx).position(window, cx)
    }

    fn can_position(&self, position: DockPosition, _window: &Window, cx: &App) -> bool {
        self.read(cx).can_position(position)
    }

    fn set_position(&self, position: DockPosition, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.set_position(position, window, cx));
    }

    fn size(&self, window: &Window, cx: &App) -> Pixels {
        self.read(cx).size(window, cx)
    }

    fn set_size(&self, size: Option<Pixels>, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.set_size(size, window, cx));
    }

    fn icon(&self, window: &Window, cx: &App) -> Option<IconName> {
        self.read(cx).icon(window, cx)
    }

    fn panel_focus_handle(&self, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx).clone()
    }

    fn set_active(&self, active: bool, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.set_active(active, window, cx));
    }

    fn is_zoomed(&self, window: &Window, cx: &App) -> bool {
        self.read(cx).is_zoomed(window, cx)
    }

    fn set_zoomed(&self, zoomed: bool, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.set_zoomed(zoomed, window, cx));
    }

    fn to_any(&self) -> AnyView {
        self.clone().into()
    }
}

impl From<&dyn PanelHandle> for AnyView {
    fn from(handle: &dyn PanelHandle) -> Self {
        handle.to_any()
    }
}
struct PanelEntry {
    panel: Arc<dyn PanelHandle>,
    _subscriptions: [Subscription; 2],
}

pub struct Dock {
    position: DockPosition,
    panel_entries: Vec<PanelEntry>,
    is_open: bool,
    active_panel_index: usize,
    focus_handle: FocusHandle,
    resizeable: bool,
    _subscriptions: [Subscription; 1],
}

impl Focusable for Dock {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Dock {
    pub fn new(
        position: DockPosition,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<Self> {
        let focus_handle = cx.focus_handle();
        // let workspace = cx.view().clone();

        let dock = cx.new(|cx: &mut Context<Self>| {
            let focus_subscription = cx.on_focus(&focus_handle, window, |dock, window, cx| {
                if let Some(active_entry) = dock.panel_entries.get(dock.active_panel_index) {
                    active_entry.panel.panel_focus_handle(cx).focus(window)
                }
            });

            Self {
                position,
                panel_entries: Vec::new(),
                active_panel_index: 0,
                is_open: false,
                focus_handle: focus_handle.clone(),
                resizeable: true,
                _subscriptions: [focus_subscription],
            }
        });

        cx.on_focus_in(&focus_handle, window, {
            let dock = dock.downgrade();
            move |_workspace, _, cx| {
                let Some(dock) = dock.upgrade() else {
                    return;
                };
                let Some(_panel) = dock.read(cx).active_panel() else {
                    return;
                };
            }
        })
        .detach();

        dock
    }

    pub fn position(&self) -> DockPosition {
        self.position
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn set_panel_zoomed(
        &mut self,
        panel: &AnyView,
        zoomed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for entry in &mut self.panel_entries {
            if entry.panel.id() == panel.entity_id() {
                if zoomed != entry.panel.is_zoomed(window, cx) {
                    entry.panel.set_zoomed(zoomed, window, cx);
                }
            } else if entry.panel.is_zoomed(window, cx) {
                entry.panel.set_zoomed(false, window, cx);
            }
        }

        cx.notify();
    }

    pub fn zoom_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for entry in &mut self.panel_entries {
            if entry.panel.is_zoomed(window, cx) {
                entry.panel.set_zoomed(false, window, cx);
            }
        }
    }

    pub(crate) fn add_panel<T: Panel>(
        &mut self,
        panel: Entity<T>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscriptions = [
            cx.observe(&panel, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &panel,
                window,
                move |this, panel, event, window, cx| match event {
                    PanelEvent::ZoomIn => {
                        this.set_panel_zoomed(&panel.to_any(), true, window, cx);
                        if !panel.focus_handle(cx).contains_focused(window, cx) {
                            cx.focus_view(panel, window);
                        }
                        workspace
                            .update(cx, |workspace, cx| {
                                workspace.zoomed = Some(panel.downgrade().into());
                                workspace.zoomed_position =
                                    Some(panel.read(cx).position(window, cx));
                                cx.emit(Event::ZoomChanged);
                            })
                            .ok();
                    }
                    PanelEvent::ZoomOut => {
                        this.set_panel_zoomed(&panel.to_any(), false, window, cx);
                        workspace
                            .update(cx, |workspace, cx| {
                                if workspace.zoomed_position == Some(this.position) {
                                    workspace.zoomed = None;
                                    workspace.zoomed_position = None;
                                    cx.emit(Event::ZoomChanged);
                                }
                                cx.notify();
                            })
                            .ok();
                    }
                    PanelEvent::Activate => {
                        if let Some(ix) = this
                            .panel_entries
                            .iter()
                            .position(|entry| entry.panel.id() == Entity::entity_id(panel))
                        {
                            this.set_open(true, window, cx);
                            this.activate_panel(ix, window, cx);
                            cx.focus_view(panel, window);
                        }
                    }
                    PanelEvent::Close => {
                        if this
                            .visible_panel()
                            .map_or(false, |p| p.id() == Entity::entity_id(panel))
                        {
                            this.set_open(false, window, cx);
                        }
                    }
                },
            ),
        ];

        let _name = panel.persistent_name().to_string();

        self.panel_entries.push(PanelEntry {
            panel: Arc::new(panel.clone()),
            _subscriptions: subscriptions,
        });

        if panel.read(cx).starts_open(window, cx) {
            self.activate_panel(self.panel_entries.len() - 1, window, cx);
            self.set_open(true, window, cx);
        }

        cx.notify()
    }

    pub fn remove_panel<T: Panel>(
        &mut self,
        panel: &Entity<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel_ix) = self
            .panel_entries
            .iter()
            .position(|entry| entry.panel.id() == Entity::entity_id(panel))
        {
            #[allow(clippy::comparison_chain)]
            if panel_ix == self.active_panel_index {
                self.active_panel_index = 0;
                self.set_open(false, window, cx);
            } else if panel_ix < self.active_panel_index {
                self.active_panel_index -= 1;
            }
            self.panel_entries.remove(panel_ix);
            cx.notify();
        }
    }

    pub fn panels_len(&self) -> usize {
        self.panel_entries.len()
    }

    pub fn activate_panel(&mut self, panel_ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if panel_ix != self.active_panel_index {
            if let Some(active_panel) = self.panel_entries.get(self.active_panel_index) {
                active_panel.panel.set_active(false, window, cx);
            }

            self.active_panel_index = panel_ix;
            if let Some(active_panel) = self.panel_entries.get(self.active_panel_index) {
                active_panel.panel.set_active(true, window, cx);
            }

            cx.notify();
        }
    }

    pub fn visible_panel(&self) -> Option<&Arc<dyn PanelHandle>> {
        let entry = self.visible_entry()?;
        Some(&entry.panel)
    }

    pub fn active_panel(&self) -> Option<&Arc<dyn PanelHandle>> {
        Some(&self.panel_entries.get(self.active_panel_index)?.panel)
    }

    pub fn active_panel_index(&self) -> usize {
        self.active_panel_index
    }

    fn visible_entry(&self) -> Option<&PanelEntry> {
        if self.is_open {
            self.panel_entries.get(self.active_panel_index)
        } else {
            None
        }
    }

    pub(crate) fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open != self.is_open {
            self.is_open = open;
            if let Some(active_panel) = self.panel_entries.get(self.active_panel_index) {
                active_panel.panel.set_active(open, window, cx);
            }

            cx.notify();
        }
    }

    pub fn panel<T: Panel>(&self) -> Option<Entity<T>> {
        self.panel_entries
            .iter()
            .find_map(|entry| entry.panel.to_any().clone().downcast().ok())
    }

    pub fn resize_active_panel(
        &mut self,
        size: Option<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.panel_entries.get_mut(self.active_panel_index) {
            let size = size.map(|size| size.max(RESIZE_HANDLE_SIZE).round());
            entry.panel.set_size(size, window, cx);
            cx.notify();
        }
    }
}

impl Render for Dock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        if self.visible_entry().is_none() {
            return div().key_context("Dock").track_focus(&self.focus_handle);
        }

        let entry = self.visible_entry().unwrap();
        let size = entry.panel.size(window, cx);
        let position = self.position;
        let create_resize_handle = || {
            let handle = div()
                .id("resize-handle")
                .on_drag(DraggedDock(position), |dock, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| dock.clone())
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|v, e: &MouseUpEvent, window, cx| {
                        if e.click_count == 2 {
                            v.resize_active_panel(None, window, cx);
                            cx.stop_propagation();
                        }
                    }),
                )
                .occlude();

            match self.position() {
                DockPosition::Left => deferred(
                    handle
                        .absolute()
                        .right(-RESIZE_HANDLE_SIZE / 2.)
                        .top(px(0.))
                        .h_full()
                        .w(RESIZE_HANDLE_SIZE)
                        .cursor_col_resize(),
                ),
                DockPosition::Bottom => deferred(
                    handle
                        .absolute()
                        .top(-RESIZE_HANDLE_SIZE / 2.)
                        .left(px(0.))
                        .w_full()
                        .h(RESIZE_HANDLE_SIZE)
                        .cursor_row_resize(),
                ),
                DockPosition::Right => deferred(
                    handle
                        .absolute()
                        .top(px(0.))
                        .left(-RESIZE_HANDLE_SIZE / 2.)
                        .h_full()
                        .w(RESIZE_HANDLE_SIZE)
                        .cursor_col_resize(),
                ),
            }
        };

        div()
            .key_context("Dock")
            .track_focus(&self.focus_handle)
            .flex()
            .bg(cx.theme().panel)
            .border_color(cx.theme().border)
            .overflow_hidden()
            .map(|this| match self.position().axis() {
                Axis::Horizontal => this.w(size).h_full().flex_row(),
                Axis::Vertical => this.h(size).w_full().flex_col(),
            })
            .map(|this| match self.position() {
                DockPosition::Left => this.border_r_1(),
                DockPosition::Right => this.border_l_1(),
                DockPosition::Bottom => this.border_t_1(),
            })
            .child(
                div()
                    .map(|this| match self.position().axis() {
                        Axis::Horizontal => this.min_w(size).h_full(),
                        Axis::Vertical => this.min_h(size).w_full(),
                    })
                    .child(
                        entry
                            .panel
                            .to_any()
                            .cached(StyleRefinement::default().v_flex().size_full()),
                    ),
            )
            .when(self.resizeable, |this| this.child(create_resize_handle()))
    }
}
