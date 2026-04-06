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
- [ ] **Error Handling:** Centralized error logging and user notifications.
- [ ] **State Management:** Optimize Dioxus signals for large conversation histories.
- [ ] **File & Folder Sharing:** Let users attach files and folders directly into chat flows, not only RAG indexing.
- [ ] **Image Sharing:** Support image attachment/upload in chat and render image messages cleanly.
- [ ] **Multimodal Pipeline:** Pass attached images/files to models or tool flows that can actually consume them.
- [ ] **MCP Integrations UX:** Support multiple MCP integrations with structured config, auth/env fields, and validation.
- [ ] **Tool-Friendly Results:** Add richer result views for common MCP/file operations instead of raw text-first outputs.
- [ ] **Execution Safety:** Add sandboxing or a safer execution boundary for the inline code runner.
- [ ] **Long-History Performance:** Add virtualization or equivalent rendering strategy for very large chats.

---
*Status: ⚙️ Phases 1-3 complete. Phase 4 focuses on product hardening, richer attachments, safer execution, and scalability.*
