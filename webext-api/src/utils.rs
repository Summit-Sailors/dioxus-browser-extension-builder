use crate::error::ExtensionError;
use js_sys::{Function, Object, Promise, Reflect};
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

pub fn get_api_namespace(root: &JsValue, name: &str) -> Result<Object, ExtensionError> {
	Reflect::get(root, &name.into())
		.map_err(|_| ExtensionError::ApiNotFound(name.to_string()))?
		.dyn_into()
		.map_err(|_| ExtensionError::ApiNotFound(name.to_string()))
}

pub async fn call_async_fn(api: &Object, method: &str, args: &[JsValue]) -> Result<JsValue, ExtensionError> {
	let func: Function = Reflect::get(api, &method.into())?.dyn_into()?;
	let js_args = args.iter().cloned().collect::<js_sys::Array>();
	let promise: Promise = func.apply(&api.into(), &js_args)?.dyn_into()?;
	JsFuture::from(promise).await.map_err(Into::into)
}

pub async fn call_async_fn_and_de<T: DeserializeOwned>(api: &Object, method: &str, args: &[JsValue]) -> Result<T, ExtensionError> {
	let result = call_async_fn(api, method, args).await?;
	serde_wasm_bindgen::from_value(result).map_err(Into::into)
}
