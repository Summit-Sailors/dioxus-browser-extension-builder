use crate::{
	error::ExtensionError,
	types::{Alarm, AlarmInfo, ListenerHandle, attach_listener},
	utils::{call_async_fn, call_async_fn_and_de, get_api_namespace},
};
use js_sys::Object;
use serde_wasm_bindgen::to_value;
use wasm_bindgen::{JsValue, prelude::*};

#[derive(Clone)]
pub struct Alarms {
	api: Object,
}

impl Alarms {
	pub(crate) fn new(api_root: &Object) -> Self {
		let api = get_api_namespace(api_root, "alarms").expect("`alarms` API not available");
		Self { api }
	}

	pub async fn create(&self, name: &str, alarm_info: AlarmInfo) -> Result<(), ExtensionError> {
		call_async_fn(&self.api, "create", &[name.into(), to_value(&alarm_info)?][..]).await?;
		Ok(())
	}

	pub async fn clear(&self, name: &str) -> Result<bool, ExtensionError> {
		call_async_fn_and_de(&self.api, "clear", &[name.into()][..]).await
	}

	pub fn on_alarm(&self) -> Result<OnAlarm, ExtensionError> {
		Ok(OnAlarm(get_api_namespace(&self.api, "onAlarm")?))
	}
}

pub struct OnAlarm(Object);

impl OnAlarm {
	pub fn add_listener(&self, mut callback: impl FnMut(Alarm) + 'static) -> Result<ListenerHandle<dyn FnMut(JsValue)>, ExtensionError> {
		attach_listener(
			&self.0,
			Closure::wrap(Box::new(move |val: JsValue| {
				if let Ok(alarm) = serde_wasm_bindgen::from_value(val) {
					callback(alarm);
				}
			}) as Box<dyn FnMut(JsValue)>),
		)
	}
}
