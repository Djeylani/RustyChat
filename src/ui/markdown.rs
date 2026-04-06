use dioxus::prelude::*;
use pulldown_cmark::{Event, Parser, TagEnd, CodeBlockKind};
use arboard::Clipboard;

#[component]
pub fn Markdown(content: String) -> Element {
    render_markdown(&content)
}

#[component]
fn CodeBlock(language: String, content: String) -> Element {
    let mut copied = use_signal(|| false);

    let copy_to_clipboard = {
        to_owned![content];
        move |_| {
            if let Ok(mut clipboard) = Clipboard::new() {
                if clipboard.set_text(content.clone()).is_ok() {
                    copied.set(true);
                    // Reset after 2 seconds
                    spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        copied.set(false);
                    });
                }
            }
        }
    };

    let run_code = {
        to_owned![content, language];
        move |_| {
            // Placeholder for Phase 3: Inline Script Runner
            println!("Run clicked for {}: \n{}", language, content);
        }
    };

    rsx! {
        div { class: "code-block-container",
            div { class: "code-block-header",
                span { class: "code-block-lang", "{language}" }
                div { class: "code-block-actions",
                    button { 
                        class: "code-action-btn copy-btn", 
                        onclick: copy_to_clipboard,
                        if copied() { "✓ Copied" } else { "📋 Copy" }
                    }
                    button { 
                        class: "code-action-btn run-btn", 
                        onclick: run_code,
                        "▶ Run"
                    }
                }
            }
            pre { class: "code-block",
                code { "{content}" }
            }
        }
    }
}

fn render_markdown(content: &str) -> Element {
    let parser = Parser::new(content);
    let mut stack: Vec<Vec<Element>> = vec![Vec::new()];
    
    // To extract raw code content, we need to track if we are inside a code block
    let mut current_code_lang = String::new();
    let mut current_code_content = String::new();
    let mut in_code_block = false;

    for event in parser {
        match event {
            Event::Start(pulldown_cmark::Tag::CodeBlock(kind)) => {
                in_code_block = true;
                current_code_content.clear();
                current_code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => "text".to_string(),
                };
                stack.push(Vec::new());
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                stack.pop(); // discard the children since we want raw text
                let parent_buffer = stack.last_mut().expect("Stack underflow");
                
                parent_buffer.push(rsx!(
                    CodeBlock { 
                        language: current_code_lang.clone(), 
                        content: current_code_content.clone() 
                    }
                ));
            }
            Event::Start(_tag) => {
                stack.push(Vec::new());
            }
            Event::End(tag_end) => {
                let children = stack.pop().unwrap_or_default();
                let parent_buffer = stack.last_mut().expect("Stack underflow");
                
                match tag_end {
                    TagEnd::Paragraph => parent_buffer.push(rsx!(p { {children.into_iter()} })),
                    TagEnd::Heading(level) => {
                        match level {
                            pulldown_cmark::HeadingLevel::H1 => parent_buffer.push(rsx!(h1 { {children.into_iter()} })),
                            pulldown_cmark::HeadingLevel::H2 => parent_buffer.push(rsx!(h2 { {children.into_iter()} })),
                            pulldown_cmark::HeadingLevel::H3 => parent_buffer.push(rsx!(h3 { {children.into_iter()} })),
                            _ => parent_buffer.push(rsx!(h4 { {children.into_iter()} })),
                        }
                    }
                    TagEnd::List(ordered) => {
                        if ordered {
                            parent_buffer.push(rsx!(ol { {children.into_iter()} }));
                        } else {
                            parent_buffer.push(rsx!(ul { {children.into_iter()} }));
                        }
                    }
                    TagEnd::Item => parent_buffer.push(rsx!(li { {children.into_iter()} })),
                    TagEnd::Strong => parent_buffer.push(rsx!(strong { {children.into_iter()} })),
                    TagEnd::Emphasis => parent_buffer.push(rsx!(em { {children.into_iter()} })),
                    TagEnd::Link => parent_buffer.push(rsx!(a { {children.into_iter()} })),
                    TagEnd::Table => parent_buffer.push(rsx!(table { {children.into_iter()} })),
                    TagEnd::TableHead => parent_buffer.push(rsx!(thead { {children.into_iter()} })),
                    TagEnd::TableRow => parent_buffer.push(rsx!(tr { {children.into_iter()} })),
                    TagEnd::TableCell => parent_buffer.push(rsx!(td { {children.into_iter()} })),
                    _ => parent_buffer.extend(children),
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    current_code_content.push_str(&text);
                } else if let Some(buffer) = stack.last_mut() {
                    buffer.push(rsx!("{text}"));
                }
            }
            Event::Code(code) => {
                if let Some(buffer) = stack.last_mut() {
                    buffer.push(rsx!(code { class: "inline-code", "{code}" }));
                }
            }
            Event::SoftBreak => {
                if in_code_block {
                    current_code_content.push('\n');
                } else if let Some(buffer) = stack.last_mut() {
                    buffer.push(rsx!(" "));
                }
            }
            Event::HardBreak => {
                if in_code_block {
                    current_code_content.push('\n');
                } else if let Some(buffer) = stack.last_mut() {
                    buffer.push(rsx!(br {}));
                }
            }
            _ => {}
        }
    }

    let final_elements = stack.pop().unwrap_or_default();

    rsx! {
        div { class: "markdown-content",
            {final_elements.into_iter()}
        }
    }
}
