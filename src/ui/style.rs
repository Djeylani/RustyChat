pub const CSS: &str = r#"
:root {
    --bg-obsidian: #050505;
    --bg-panel: #0a0a0b;
    --bg-glass: rgba(20, 20, 22, 0.85);
    --border-glass: rgba(255, 255, 255, 0.08);
    --accent-blue: #3b82f6;
    --accent-glow: rgba(59, 130, 246, 0.2);
    --text-pure: #ffffff;
    --text-dim: rgba(255, 255, 255, 0.5);
    --text-soft: rgba(255, 255, 255, 0.85);
    --danger: #ef4444;
    --font-main: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    --font-mono: 'JetBrains Mono', monospace;
}

body, html {
    margin: 0;
    padding: 0;
    background-color: var(--bg-obsidian);
    color: var(--text-pure);
    font-family: var(--font-main);
    width: 100%;
    height: 100%;
    overflow: hidden;
    -webkit-font-smoothing: antialiased;
}

* { box-sizing: border-box; }

#main,
main {
    width: 100%;
    height: 100%;
}

/* Compositional Layout */
.outer-wrapper {
    display: flex;
    width: 100vw;
    height: 100vh;
    background: var(--bg-obsidian);
    overflow: hidden;
}

.app-container {
    display: flex;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
}

.sidebar {
    width: 280px;
    min-width: 280px;
    background: var(--bg-panel);
    border-right: 1px solid var(--border-glass);
    display: flex;
    flex-direction: column;
    padding: 24px 16px;
    height: 100%;
    min-height: 0;
    overflow: hidden;
}

.logo {
    font-size: 1.1rem;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    margin-bottom: 32px;
    padding-left: 8px;
    background: linear-gradient(to right, #fff, #666);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
}

/* Sidebar Search & Buttons */
.new-chat-btn.big {
    background: var(--accent-blue);
    color: white;
    border: none;
    border-radius: 12px;
    padding: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    margin-bottom: 24px;
    box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
}

.new-chat-btn.big:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(59, 130, 246, 0.4);
}

.search-input {
    width: 100%;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-glass);
    border-radius: 10px;
    padding: 10px 14px;
    color: white;
    font-size: 0.85rem;
    margin-bottom: 20px;
    outline: none;
}

.chat-list {
    flex: 1;
    overflow-y: auto;
    padding-right: 4px;
    min-height: 0;
    min-width: 0;
}

.chat-item-row {
    margin-bottom: 8px;
}

.chat-item {
    padding: 12px 12px 12px 14px;
    border-radius: 14px;
    cursor: pointer;
    transition: all 0.2s ease;
    color: var(--text-dim);
    font-size: 0.9rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    border: 1px solid transparent;
    min-width: 0;
}

.chat-item:hover {
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-soft);
    border-color: rgba(255, 255, 255, 0.05);
}

.chat-item.active {
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.06), rgba(255, 255, 255, 0.03));
    color: var(--text-pure);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.chat-title {
    flex: 1;
    min-width: 0;
    font-size: 0.92rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: inherit;
}

.chat-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    opacity: 0;
    transform: translateX(4px);
    transition: opacity 0.16s ease, transform 0.16s ease;
}

.chat-item:hover .chat-actions,
.chat-item.active .chat-actions {
    opacity: 1;
    transform: translateX(0);
}

.chat-action-btn {
    width: 28px;
    height: 28px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-dim);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    font-size: 0.85rem;
    line-height: 1;
    transition: all 0.16s ease;
}

.chat-action-btn:hover {
    color: var(--text-pure);
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.14);
}

.delete-chat-btn:hover {
    color: #fff;
    background: rgba(239, 68, 68, 0.18);
    border-color: rgba(239, 68, 68, 0.28);
}

.rename-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex-wrap: wrap;
}

.rename-input {
    flex: 1 1 120px;
    min-width: 0;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    color: var(--text-pure);
    padding: 10px 12px;
    font-size: 0.88rem;
    outline: none;
}

.rename-input:focus {
    border-color: rgba(59, 130, 246, 0.42);
    box-shadow: 0 0 0 4px rgba(59, 130, 246, 0.12);
}

.rename-save,
.rename-cancel {
    height: 34px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.78rem;
    font-weight: 600;
    transition: all 0.16s ease;
    flex: 0 0 auto;
}

.rename-save {
    background: rgba(59, 130, 246, 0.18);
    border-color: rgba(59, 130, 246, 0.3);
    color: #fff;
}

.rename-save:hover,
.rename-cancel:hover {
    transform: translateY(-1px);
}

.sidebar-footer {
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    flex-shrink: 0;
}

.sidebar-icon-btn {
    width: 42px;
    height: 42px;
    border-radius: 14px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-soft);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    text-decoration: none;
    cursor: pointer;
    position: relative;
    transition: all 0.18s ease;
}

.sidebar-icon-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.14);
    color: var(--text-pure);
    transform: translateY(-1px);
}

.settings-btn {
    font-size: 1.1rem;
}

.repo-icon {
    font-size: 1rem;
}

.settings-tooltip {
    position: absolute;
    left: calc(100% + 10px);
    top: 50%;
    transform: translateY(-50%);
    padding: 6px 10px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-pure);
    font-size: 0.72rem;
    letter-spacing: 0.03em;
    opacity: 0;
    pointer-events: none;
    white-space: nowrap;
    transition: opacity 0.16s ease, transform 0.16s ease;
}

.settings-btn:hover .settings-tooltip {
    opacity: 1;
    transform: translateY(-50%) translateX(2px);
}

/* Immersive Chat Window */
.chat-window {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--bg-obsidian);
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
}

.chat-header {
    padding: 20px 32px;
    background: var(--bg-obsidian);
    border-bottom: 1px solid var(--border-glass);
    z-index: 20;
}

.chat-header h2 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-soft);
}

.model-indicator {
    font-size: 0.75rem;
    color: var(--accent-blue);
    margin-top: 4px;
    font-family: var(--font-mono);
    text-transform: uppercase;
}

.model-indicator.secondary {
    color: var(--text-dim);
}

.chat-messages {
    flex: 1;
    overflow-y: auto;
    padding: 60px 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    min-height: 0;
    min-width: 0;
    overscroll-behavior: contain;
}

/* Centered Message Column */
.message {
    width: 100%;
    max-width: 800px;
    padding: 0 40px;
    margin-bottom: 48px;
    display: flex;
    flex-direction: column;
    animation: slideUp 0.4s cubic-bezier(0, 0, 0.2, 1);
    min-width: 0;
}

@keyframes slideUp {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: translateY(0); }
}

.user-message {
    align-items: flex-end;
}

.user-message .markdown-content {
    background: rgba(255, 255, 255, 0.03);
    padding: 16px 24px;
    border-radius: 20px 20px 4px 20px;
    border: 1px solid var(--border-glass);
}

.assistant-message {
    align-items: flex-start;
}

.empty-state-message {
    min-height: 100%;
    justify-content: center;
}

.empty-state-copy {
    width: 100%;
    max-width: 800px;
    padding: 0 40px;
    color: var(--text-dim);
}

.empty-state-copy h3 {
    margin: 0 0 8px 0;
    font-size: 1.25rem;
    color: var(--text-pure);
}

.empty-state-copy p {
    margin: 0;
    max-width: 560px;
}

/* Markdown Polish */
.markdown-content {
    font-size: 1rem;
    line-height: 1.7;
    color: var(--text-soft);
}

.markdown-content h1, .markdown-content h2 {
    color: var(--text-pure);
    margin: 1.5em 0 0.5em 0;
}

.think-bubble {
    width: 100%;
    background: rgba(255, 255, 255, 0.02);
    border-left: 2px solid var(--accent-blue);
    padding: 16px 20px;
    border-radius: 4px;
    margin: 20px 0;
    color: var(--text-dim);
    font-family: var(--font-mono);
    font-size: 0.9rem;
}

/* Floating Input Pill */
.chat-input-area {
    width: 100%;
    max-width: 850px;
    align-self: center;
    margin: 0 auto 40px auto;
    background: var(--bg-panel);
    border: 1px solid var(--border-glass);
    border-radius: 24px;
    padding: 8px 12px;
    display: flex;
    align-items: flex-end;
    gap: 8px;
    box-shadow: 0 20px 50px rgba(0,0,0,0.5);
    z-index: 30;
    flex-shrink: 0;
}

.composer-hint {
    width: 100%;
    max-width: 850px;
    margin: -28px auto 24px auto;
    padding: 0 18px;
    color: var(--text-dim);
    font-size: 0.8rem;
}

.composer-hint.rag-status {
    margin-top: -14px;
    color: var(--text-soft);
}

.chat-input {
    flex: 1;
    background: transparent;
    border: none;
    color: white;
    font-size: 1rem;
    padding: 12px 16px;
    outline: none;
    resize: none;
    min-height: 44px;
}

.send-button.big {
    background: var(--accent-blue);
    width: 40px;
    height: 40px;
    border-radius: 50%;
    border: none;
    color: white;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform 0.2s;
    margin-bottom: 2px;
}

.send-button.big:hover { transform: scale(1.1); }

.send-button.big:disabled,
.interrupt-button.big:disabled {
    opacity: 0.45;
    cursor: not-allowed;
    transform: none;
}

.interrupt-button.big {
    background: rgba(255, 255, 255, 0.08);
    width: auto;
    min-width: 96px;
    height: 40px;
    border-radius: 999px;
    border: 1px solid var(--border-glass);
    color: white;
    cursor: pointer;
    padding: 0 16px;
    margin-bottom: 2px;
}

.secondary-action-btn {
    height: 40px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-soft);
    cursor: pointer;
    padding: 0 14px;
    font-size: 0.82rem;
    font-weight: 600;
    white-space: nowrap;
    transition: all 0.16s ease;
    margin-bottom: 2px;
}

.secondary-action-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.14);
    color: var(--text-pure);
}

.clear-index-btn:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.14);
    border-color: rgba(239, 68, 68, 0.24);
}

.loading-message {
    opacity: 0.85;
    animation: pulse 1.8s ease-in-out infinite;
}

.loading-dots::after {
    content: "...";
    margin-left: 6px;
}

@keyframes pulse {
    0% { opacity: 0.45; }
    50% { opacity: 1; }
    100% { opacity: 0.45; }
}

/* Code Blocks */
.code-block-container {
    background: #000;
    border: 1px solid var(--border-glass);
    border-radius: 12px;
    margin: 24px 0;
    overflow: hidden;
}

.code-block-header {
    background: rgba(255, 255, 255, 0.03);
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-glass);
    display: flex;
    justify-content: space-between;
}

.code-block-lang {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--accent-blue);
    font-weight: 700;
}

/* Settings Modal Overhaul */
.settings-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.8);
    backdrop-filter: blur(16px);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 100;
    padding: 24px;
}

.settings-modal {
    width: 600px;
    max-width: min(600px, 100%);
    max-height: calc(100vh - 48px);
    background: var(--bg-panel);
    border: 1px solid var(--border-glass);
    border-radius: 24px;
    padding: 40px;
    box-shadow: 0 40px 100px rgba(0,0,0,0.8);
    display: flex;
    flex-direction: column;
    gap: 24px;
    overflow-y: auto;
}

.settings-modal h3 {
    margin: 0 0 8px 0;
    font-size: 1.25rem;
    font-weight: 700;
}

.settings-modal label {
    display: block;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
    margin-bottom: 8px;
}

.settings-modal .input, 
.settings-modal .textarea, 
.settings-modal select {
    width: 100%;
    background: rgba(18, 18, 20, 0.96);
    border: 1px solid var(--border-glass);
    border-radius: 12px;
    padding: 12px 16px;
    color: var(--text-pure);
    font-size: 0.95rem;
    outline: none;
    transition: all 0.2s;
}

.settings-modal select {
    appearance: auto;
}

.settings-modal select option {
    background: #121214;
    color: #ffffff;
}

.settings-modal .input:focus, 
.settings-modal .textarea:focus, 
.settings-modal select:focus {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.2);
}

.settings-modal .textarea {
    min-height: 120px;
    resize: vertical;
    max-width: 100%;
}

.zoom-row {
    display: flex;
    align-items: center;
    gap: 16px;
}

.zoom-row button {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--border-glass);
    color: white;
    cursor: pointer;
}

.zoom-row span {
    font-family: var(--font-mono);
    font-weight: 600;
}

.modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 16px;
    position: sticky;
    bottom: -40px;
    padding-top: 16px;
    background: linear-gradient(to bottom, rgba(10, 10, 11, 0), rgba(10, 10, 11, 0.96) 30%);
    flex-wrap: wrap;
}

.modal-actions button {
    padding: 10px 24px;
    border-radius: 12px;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: all 0.2s;
}

.modal-actions button:not(.delete-all) {
    background: rgba(255, 255, 255, 0.05);
    color: white;
}

.modal-actions button:first-child {
    background: var(--accent-blue);
}

.modal-actions .delete-all {
    background: transparent;
    color: var(--danger);
    margin-right: auto;
    font-size: 0.85rem;
}

.modal-actions .delete-all:hover {
    background: rgba(239, 68, 68, 0.1);
}

@media (max-width: 980px) {
    .chat-messages {
        padding: 32px 0 24px 0;
    }

    .message,
    .empty-state-copy {
        padding: 0 24px;
    }

    .chat-input-area,
    .composer-hint {
        max-width: calc(100% - 32px);
    }
}

@media (max-width: 720px) {
    .sidebar {
        width: 240px;
        min-width: 240px;
        padding: 18px 12px;
    }

    .message,
    .empty-state-copy {
        padding: 0 18px;
    }

    .chat-input-area {
        border-radius: 18px;
        margin-bottom: 20px;
    }

    .settings-modal {
        padding: 24px;
        border-radius: 18px;
    }
}

/* Custom Scrollbar */
::-webkit-scrollbar { width: 5px; }
::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.1); border-radius: 10px; }
"#;
