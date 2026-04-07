# RustyChat Evolution Roadmap 🚀

This document tracks the transformation of RustyChat from a primitive Ollama wrapper into a high-performance, local-first AI Agent platform inspired by Askimo.

## Phase 1: Visual & UX Leap (The "Look") ✅
- [x] **Markdown Rendering:** Implement `pulldown-cmark` for tables, bold text, and math.
- [x] **Code Block Actions:** Add "Copy" and "Run" buttons to AI-generated code.
- [x] **Fluid UI:** 
    - [x] Auto-scroll to bottom with "manual scroll" detection.
    - [x] Sidebar search/filter for past conversations.
    - [x] Thinking/Loading animations.
- [x] **Modern Aesthetic:** "Obsidian Agent" design with glassmorphism and cinematic spacing.

## Phase 2: Hybrid RAG (The "Brain") ✅
- [x] **Ollama Embeddings:** Integrate `/api/embeddings` support.
- [x] **Local Indexing:**
    - [x] "Add Folder" button for context.
    - [x] Recursive file walking (`walkdir`).
    - [x] Vector storage in SQLite.
- [x] **Context Injection:** Automatic retrieval of relevant chunks before prompting.

## Phase 3: Agentic Execution (The "Power") ✅
- [x] **Inline Script Runner:** Execute Python/Bash/JS blocks directly.
- [x] **Output Console:** Capture and display `stdout`/`stderr` in a drawer.
- [x] **MCP Client:** Basic Model Context Protocol support for external tools.

## Phase 4: Architectural Integrity (The "Foundation") ✅
- [x] **Modularization:** Split `main.rs` into `db`, `ui`, `ollama`, and `rag`.
- [x] **Shared Notifications:** Add centralized in-app toast notifications for major success/error states.
- [x] **Error Handling:** Centralized app error logging and cleaned-up notification handling across core chat/MCP/RAG flows.
- [x] **State Management:** Load chat history windows from SQLite on demand instead of keeping the full conversation in one UI signal.
- [x] **File Sharing:** Let users attach files directly into chat flows with previews and prompt injection for text files.
- [x] **Folder Sharing:** Let users attach folders directly into chat flows, not only RAG indexing.
- [x] **Image Sharing:** Support image attachment/upload in chat and render image messages cleanly.
- [x] **Multimodal Pipeline:** Pass attached images to Ollama chat requests for vision-capable models.
- [x] **MCP Integrations UX:** Support multiple MCP integrations with structured config, auth/env fields, active switching, and validation.
- [x] **Tool-Friendly Results:** Add richer result views for common MCP/file operations instead of raw text-first outputs.
- [x] **Execution Safety:** Add an opt-in inline runner with temp-folder isolation, timeouts, and output caps.
- [x] **Long-History Performance:** Limit the initial render window for large chats and let users load older messages on demand.

---
*Status: ✅ Phases 1-4 complete. RustyChat now includes file and folder attachments, vision support, shared notifications, centralized app error logging, safer code execution, multi-server MCP UX, richer MCP result views, and DB-backed on-demand loading for long chat histories.*
