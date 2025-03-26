# Browser Extension Content Script Crate

This crate is responsible for generating the binding that runs in the context of web pages, analyzing DOM elements and extracting page content. It initializes automatically when the page loads, counts elements like divs, and offers two extraction methods: a sophisticated Readability mode for article-focused content and a Basic mode that strips common noisy elements like ads, headers and comments. The script communicates these findings back to the extension's background process through Chrome's messaging system.
