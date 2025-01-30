use std::any::TypeId;

use gpui::{
    AnyElement, AnyView, App, Context, Element as _, Entity, EntityId, EventEmitter, FocusHandle,
    Focusable, Pixels, Point, Render, SharedString, WeakEntity,
};
use ui::prelude::Window;

use super::{
    pane::{self, Pane},
    workspace::{Workspace, WorkspaceId},
};

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum ItemEvent {
    CloseItem,
    UpdateTab,
    Edit,
}

#[derive(Debug, Clone, Copy)]
pub struct TabContentParams {
    pub detail: Option<usize>,
    pub selected: bool,
}

pub trait Item: Focusable + EventEmitter<Self::Event> + Render + Sized {
    type Event;

    /// Returns the content of the tab for this item.
    fn tab_content(&self, _params: TabContentParams, _window: &Window, _cx: &App) -> AnyElement {
        gpui::Empty.into_any()
    }

    /// Returns the tooltip for the tab.
    fn tab_tooltip(&self, _: &App) -> Option<SharedString> {
        None
    }

    /// Returns the description for the tab.
    fn tab_description(&self, _: usize, _: &App) -> Option<SharedString> {
        None
    }

    fn to_item_events(_event: &Self::Event, _f: impl FnMut(ItemEvent)) {}

    /// Invoked when the item is deactivated.
    fn deactivated(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    /// Invoked when the workspace is deactivated.
    fn workspace_deactivated(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn is_singleton(&self, _cx: &App) -> bool {
        false
    }

    fn clone_on_split(
        &self,
        _workspace_id: Option<WorkspaceId>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Entity<Self>>
    where
        Self: Sized,
    {
        None
    }

    fn act_as_type<'a>(
        &'a self,
        type_id: TypeId,
        self_handle: &'a Entity<Self>,
        _: &'a App,
    ) -> Option<AnyView> {
        if TypeId::of::<Self>() == type_id {
            Some(self_handle.clone().into())
        } else {
            None
        }
    }

    fn added_to_workspace(
        &mut self,
        _workspace: &mut Workspace,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }
    fn pixel_position_of_cursor(&self, _: &App) -> Option<Point<Pixels>> {
        None
    }
}

pub trait ItemHandle: 'static + Send {
    fn item_id(&self) -> EntityId;
    #[allow(clippy::type_complexity)]
    fn subscribe_to_item_events(
        &self,
        window: &mut Window,
        cx: &mut App,

        handler: Box<dyn Fn(ItemEvent, &mut Window, &mut App)>,
    ) -> gpui::Subscription;
    fn item_focus_handle(&self, window: &Window, cx: &App) -> FocusHandle;
    fn tab_tooltip(&self, cx: &App) -> Option<SharedString>;
    fn tab_description(&self, detail: usize, cx: &App) -> Option<SharedString>;
    fn tab_content(&self, params: TabContentParams, window: &Window, cx: &App) -> AnyElement;
    fn dragged_tab_content(
        &self,
        params: TabContentParams,
        window: &Window,
        cx: &App,
    ) -> AnyElement;
    fn clone_on_split(
        &self,
        workspace_id: Option<WorkspaceId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn ItemHandle>>;
    fn added_to_pane(
        &self,
        workspace: &mut Workspace,
        pane: &Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    );
    fn deactivated(&self, window: &mut Window, cx: &mut App);
    fn workspace_deactivated(&self, window: &mut Window, cx: &mut App);
    fn to_any(&self) -> AnyView;
    fn on_release(
        &self,
        cx: &mut App,
        callback: Box<dyn FnOnce(&mut App) + Send>,
    ) -> gpui::Subscription;
    fn pixel_position_of_cursor(&self, cx: &App) -> Option<Point<Pixels>>;
    fn downgrade_item(&self) -> Box<dyn WeakItemHandle>;
    fn boxed_clone(&self) -> Box<dyn ItemHandle>;
    fn act_as_type<'a>(&'a self, type_id: TypeId, cx: &'a App) -> Option<AnyView>;
}

pub trait WeakItemHandle: Send + Sync {
    fn id(&self) -> EntityId;
    fn upgrade(&self) -> Option<Box<dyn ItemHandle>>;
}

impl dyn ItemHandle {
    pub fn downcast<V: 'static>(&self) -> Option<Entity<V>> {
        self.to_any().downcast().ok()
    }

    pub fn act_as<V: 'static>(&self, cx: &App) -> Option<Entity<V>> {
        self.act_as_type(TypeId::of::<V>(), cx)
            .and_then(|t| t.downcast().ok())
    }
}

impl<T: Item> ItemHandle for Entity<T> {
    fn subscribe_to_item_events(
        &self,
        window: &mut Window,
        cx: &mut App,
        handler: Box<dyn Fn(ItemEvent, &mut Window, &mut App)>,
    ) -> gpui::Subscription {
        window.subscribe(self, cx, move |_, event, window, cx| {
            T::to_item_events(event, |item_event| handler(item_event, window, cx));
        })
    }

    fn item_focus_handle(&self, _window: &Window, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx)
    }

    fn tab_tooltip(&self, cx: &App) -> Option<SharedString> {
        self.read(cx).tab_tooltip(cx)
    }

    fn tab_description(&self, detail: usize, cx: &App) -> Option<SharedString> {
        self.read(cx).tab_description(detail, cx)
    }

    fn tab_content(&self, params: TabContentParams, window: &Window, cx: &App) -> AnyElement {
        self.read(cx).tab_content(params, window, cx)
    }

    fn dragged_tab_content(
        &self,
        params: TabContentParams,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        self.read(cx).tab_content(
            TabContentParams {
                selected: true,
                ..params
            },
            window,
            cx,
        )
    }

    fn boxed_clone(&self) -> Box<dyn ItemHandle> {
        Box::new(self.clone())
    }

    fn act_as_type<'a>(&'a self, type_id: TypeId, cx: &'a App) -> Option<AnyView> {
        self.read(cx).act_as_type(type_id, self, cx)
    }

    fn clone_on_split(
        &self,
        workspace_id: Option<WorkspaceId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn ItemHandle>> {
        self.update(cx, |item, cx| item.clone_on_split(workspace_id, window, cx))
            .map(|handle| Box::new(handle) as Box<dyn ItemHandle>)
    }

    fn added_to_pane(
        &self,
        workspace: &mut Workspace,
        pane: &Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let _weak_item = self.downgrade();

        if workspace
            .panes_by_item
            .insert(self.item_id(), pane.downgrade())
            .is_none()
        {
            let mut event_subscription = Some(cx.subscribe_in(
                self,
                window,
                move |workspace, item: &Entity<T>, event, window, cx| {
                    let pane = if let Some(pane) = workspace
                        .panes_by_item
                        .get(&item.item_id())
                        .and_then(|pane| pane.upgrade())
                    {
                        pane
                    } else {
                        return;
                    };

                    T::to_item_events(event, |event| match event {
                        ItemEvent::CloseItem => {
                            pane.update(cx, |pane, cx| {
                                pane.close_item_by_id(item.item_id(), window, cx)
                            })
                            .detach_and_log_err(cx);
                        }

                        ItemEvent::UpdateTab => {
                            pane.update(cx, |_, cx| {
                                cx.emit(pane::Event::ChangeItemTitle);
                                cx.notify();
                            });
                        }

                        _ => {}
                    });
                },
            ));

            let item_id = self.item_id();
            cx.observe_release(self, move |workspace, _, _| {
                workspace.panes_by_item.remove(&item_id);
                event_subscription.take();
            })
            .detach();
        }

        // cx.defer(|workspace, cx| {
        //     workspace.serialize_workspace(cx);
        // });
    }

    fn deactivated(&self, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.deactivated(window, cx));
    }

    fn workspace_deactivated(&self, window: &mut Window, cx: &mut App) {
        self.update(cx, |this, cx| this.workspace_deactivated(window, cx));
    }

    fn item_id(&self) -> EntityId {
        self.entity_id()
    }

    fn to_any(&self) -> AnyView {
        self.clone().into()
    }

    fn on_release(
        &self,
        cx: &mut App,
        callback: Box<dyn FnOnce(&mut App) + Send>,
    ) -> gpui::Subscription {
        cx.observe_release(self, move |_, cx| callback(cx))
    }

    fn pixel_position_of_cursor(&self, cx: &App) -> Option<Point<Pixels>> {
        self.read(cx).pixel_position_of_cursor(cx)
    }

    fn downgrade_item(&self) -> Box<dyn WeakItemHandle> {
        Box::new(self.downgrade())
    }
}

impl From<Box<dyn ItemHandle>> for AnyView {
    fn from(val: Box<dyn ItemHandle>) -> Self {
        val.to_any()
    }
}

impl From<&Box<dyn ItemHandle>> for AnyView {
    fn from(val: &Box<dyn ItemHandle>) -> Self {
        val.to_any()
    }
}

impl Clone for Box<dyn ItemHandle> {
    fn clone(&self) -> Box<dyn ItemHandle> {
        self.boxed_clone()
    }
}

impl<T: Item> WeakItemHandle for WeakEntity<T> {
    fn id(&self) -> EntityId {
        self.entity_id()
    }

    fn upgrade(&self) -> Option<Box<dyn ItemHandle>> {
        self.upgrade().map(|v| Box::new(v) as Box<dyn ItemHandle>)
    }
}
