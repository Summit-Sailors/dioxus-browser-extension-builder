use crate::error::ExtensionError;
use js_sys::{Function, Object};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, prelude::*};

pub struct ListenerHandle<T: ?Sized> {
	target: Object,
	closure: Closure<T>,
}

impl<T: ?Sized> Drop for ListenerHandle<T> {
	fn drop(&mut self) {
		if let Ok(remove_listener_fn) = js_sys::Reflect::get(&self.target, &"removeListener".into()).and_then(|v| v.dyn_into::<Function>()) {
			let _ = remove_listener_fn.call1(&self.target, self.closure.as_ref());
		}
	}
}

pub(crate) fn attach_listener<T: ?Sized + 'static>(target: &Object, closure: Closure<T>) -> Result<ListenerHandle<T>, ExtensionError> {
	let add_listener_fn: Function =
		js_sys::Reflect::get(target, &"addListener".into())?.dyn_into().map_err(|_| ExtensionError::ApiNotFound("addListener".to_string()))?;
	add_listener_fn.call1(target, closure.as_ref())?;
	Ok(ListenerHandle { target: target.clone(), closure })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserType {
	Chrome,
	Firefox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
	pub id: Option<u32>,
	pub title: Option<String>,
	pub url: Option<String>,
	pub active: bool,
	pub window_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabChangeInfo {
	pub status: Option<String>,
	pub url: Option<String>,
	pub title: Option<String>,
	pub audible: Option<bool>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeConfig {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub text: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tab_id: Option<u32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub background_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMenuConfig {
	pub id: String,
	pub title: String,
	pub contexts: Vec<String>,
}

impl ContextMenuConfig {
	pub fn build(id: impl Into<String>, title: impl Into<String>) -> ContextMenuConfigBuilder {
		ContextMenuConfigBuilder { id: id.into(), title: title.into(), contexts: vec![] }
	}
}

pub struct ContextMenuConfigBuilder {
	id: String,
	title: String,
	contexts: Vec<String>,
}

impl ContextMenuConfigBuilder {
	pub fn contexts(mut self, contexts: &[&str]) -> Self {
		self.contexts = contexts.iter().map(|s| s.to_string()).collect();
		self
	}

	pub fn build(self) -> ContextMenuConfig {
		ContextMenuConfig { id: self.id, title: self.title, contexts: self.contexts }
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmInfo {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub delay_in_minutes: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub period_in_minutes: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alarm {
	pub name: String,
	pub scheduled_time: f64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub period_in_minutes: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRulesOptions {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub add_rules: Vec<Rule>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub remove_rule_ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
	pub id: u32,
	pub priority: u32,
	pub action: RuleAction,
	pub condition: RuleCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAction {
	#[serde(rename = "type")]
	pub action_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
	pub url_filter: String,
	pub resource_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
	pub name: String,
	pub description: Option<String>,
	pub shortcut: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSender {
	pub id: Option<String>,
	pub url: Option<String>,
	pub tab: Option<TabInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnClickData {
	pub menu_item_id: String,
	pub page_url: Option<String>,
	pub selection_text: Option<String>,
}
