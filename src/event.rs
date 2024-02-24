use std::{
    collections::{BTreeMap, BTreeSet},
    sync::RwLock,
};

use serde::{de::DeserializeOwned, Serialize};
use specta::{DataType, NamedType, SpectaID};
use tauri::{EventId, Manager, Runtime, Window};

use crate::PluginName;

#[derive(Clone, Copy)]
pub struct EventRegistryMeta {
    plugin_name: PluginName,
}

impl EventRegistryMeta {
    fn wrap_with_plugin(&self, input: &str) -> String {
        self.plugin_name
            .apply_as_prefix(input, crate::ItemType::Event)
    }
}

#[derive(Default)]
pub struct EventCollection(pub(crate) BTreeSet<SpectaID>, BTreeSet<&'static str>);

impl EventCollection {
    pub fn register<E: Event>(&mut self) {
        if !self.0.insert(E::SID) {
            panic!("Event {} registered twice!", E::NAME)
        }

        if !self.1.insert(E::NAME) {
            panic!("Another event with name {} is already registered!", E::NAME)
        }
    }
}

#[derive(Default)]
pub(crate) struct EventRegistry(pub(crate) RwLock<BTreeMap<SpectaID, EventRegistryMeta>>);

impl EventRegistry {
    pub fn register_collection(&self, collection: EventCollection, plugin_name: PluginName) {
        let mut registry = self.0.write().expect("Failed to write EventRegistry");

        registry.extend(
            collection
                .0
                .into_iter()
                .map(|sid| (sid, EventRegistryMeta { plugin_name })),
        );
    }

    pub fn get_or_manage<R: Runtime>(handle: &impl Manager<R>) -> tauri::State<'_, Self> {
        if handle.try_state::<Self>().is_none() {
            handle.manage(Self::default());
        }

        handle.state::<Self>()
    }
}

pub struct TypedEvent<T: Event> {
    pub id: EventId,
    pub payload: T,
}

fn get_meta_from_registry<R: Runtime>(
    sid: SpectaID,
    name: &str,
    handle: &impl Manager<R>,
) -> EventRegistryMeta {
    handle.try_state::<EventRegistry>().expect(
        "EventRegistry not found in Tauri state - Did you forget to call Exporter::with_events?",
    )
    .0
        .read()
        .expect("Failed to read EventRegistry")
        .get(&sid)
        .copied()
        .unwrap_or_else(|| panic!("Event {name} not found in registry!"))
}

macro_rules! make_handler {
    ($handler:ident) => {
        move |event| {
            let value: serde_json::Value = serde_json::from_str(event.payload())
                .ok() // TODO: Error handling
                .unwrap_or(serde_json::Value::Null);

            $handler(TypedEvent {
                id: event.id(),
                payload: serde_json::from_value(value)
                    .expect("Failed to deserialize event payload"),
            });
        }
    };
}

macro_rules! get_meta {
    ($handle:ident) => {
        get_meta_from_registry(Self::SID, Self::NAME, $handle)
    };
}

pub trait Event: NamedType {
    const NAME: &'static str;

    // Manager functions

    fn emit_all<R: Runtime>(self, handle: &impl Manager<R>) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        let meta = get_meta!(handle);

        handle.emit(&meta.wrap_with_plugin(Self::NAME), self)
    }

    fn emit_to<R: Runtime>(self, handle: &impl Manager<R>, label: &str) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        let meta = get_meta!(handle);

        handle.emit_to(label, &meta.wrap_with_plugin(Self::NAME), self)
    }

    fn listen_any<F, R: Runtime>(handle: &impl Manager<R>, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(handle);

        handle.listen_any(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn once_any<F, R: Runtime>(handle: &impl Manager<R>, handler: F)
    where
        F: FnOnce(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(handle);

        handle.once_any(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    // Window functions

    fn emit(self, window: &Window<impl Runtime>) -> tauri::Result<()>
    where
        Self: Serialize + Clone,
    {
        let meta = get_meta!(window);

        window.emit(&meta.wrap_with_plugin(Self::NAME), self)
    }

    fn listen<F>(window: &Window<impl Runtime>, handler: F) -> EventId
    where
        F: Fn(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(window);

        window.listen(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }

    fn once<F>(window: &Window<impl Runtime>, handler: F)
    where
        F: FnOnce(TypedEvent<Self>) + Send + 'static,
        Self: DeserializeOwned,
    {
        let meta = get_meta!(window);

        window.once(meta.wrap_with_plugin(Self::NAME), make_handler!(handler))
    }
}

pub struct EventDataType {
    pub name: &'static str,
    pub typ: DataType,
}

pub(crate) type CollectEventsTuple = (EventCollection, Vec<EventDataType>, specta::TypeMap);

#[macro_export]
macro_rules! collect_events {
    ($($event:ident),+) => {{
    	let mut collection: $crate::EventCollection = ::core::default::Default::default();

     	$(collection.register::<$event>();)+

      	let mut type_map = Default::default();

      	let event_data_types = [$(
	       $crate::EventDataType {
	       		name: <$event as $crate::Event>::NAME,
	       		typ: <$event as ::specta::Type>::reference(&mut type_map, &[]).inner
	       }
       	),+]
        .into_iter()
        .collect::<Vec<_>>();

      	(collection, event_data_types, type_map)
    }};
}
