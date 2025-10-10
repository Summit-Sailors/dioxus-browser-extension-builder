use crate::{
	error::ExtensionError,
	utils::{call_async_fn, get_api_namespace},
};
use js_sys::{Object, Reflect};
use serde::{Serialize, de::DeserializeOwned};
use serde_wasm_bindgen::to_value;

#[derive(Clone)]
pub struct Storage {
	api: Object,
}

impl Storage {
	pub(crate) fn new(api_root: &Object) -> Self {
		let api = get_api_namespace(api_root, "storage").expect("`storage` API not available");
		Self { api }
	}

	pub fn local(&self) -> StorageArea {
		let local_api = get_api_namespace(&self.api, "local").expect("`storage.local` API not available");
		StorageArea { api: local_api }
	}

	pub fn sync(&self) -> StorageArea {
		let sync_api = get_api_namespace(&self.api, "sync").expect("`storage.sync` API not available");
		StorageArea { api: sync_api }
	}
}

#[derive(Clone)]
pub struct StorageArea {
	api: Object,
}

impl StorageArea {
	pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, ExtensionError> {
		let result = call_async_fn(&self.api, "get", &[key.into()][..]).await?;
		let value = Reflect::get(&result, &key.into())?;
		if value.is_undefined() || value.is_null() { Ok(None) } else { serde_wasm_bindgen::from_value(value).map(Some).map_err(Into::into) }
	}

	pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ExtensionError> {
		let items = Object::new();
		Reflect::set(&items, &key.into(), &to_value(value)?)?;
		call_async_fn(&self.api, "set", &[items.into()][..]).await?;
		Ok(())
	}
}
