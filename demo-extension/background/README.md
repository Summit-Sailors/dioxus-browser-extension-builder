# Browser Extension Background Script Crate

This crate is responsible for generating a binding responsible for creating the central communication hub for the browser extension, handling messages between the content script and the popup(UI) running on web pages. It initializes when the extension loads, setting up event listeners that process button clicks and input changes from the popup. This crate is responsible for maintaining persistent connection throughout the browser session, enabling real-time communication between components through Chrome's messaging API.
