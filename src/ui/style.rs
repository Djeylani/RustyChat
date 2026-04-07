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

.chat-header-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
}

.chat-header-actions {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 10px;
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

.header-workspace-btn {
    min-width: 108px;
    height: 40px;
    border-radius: 999px;
    border: 1px solid rgba(59, 130, 246, 0.22);
    background: linear-gradient(135deg, rgba(59, 130, 246, 0.18), rgba(15, 23, 42, 0.55));
    color: var(--text-pure);
    cursor: pointer;
    padding: 0 18px;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    box-shadow: 0 12px 28px rgba(59, 130, 246, 0.16);
    transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
}

.header-workspace-btn:hover,
.header-workspace-btn.active {
    transform: translateY(-1px);
    border-color: rgba(96, 165, 250, 0.42);
    box-shadow: 0 16px 34px rgba(59, 130, 246, 0.22);
}

.clear-index-header-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, rgba(239, 68, 68, 0.16), rgba(15, 23, 42, 0.55));
    border-color: rgba(239, 68, 68, 0.28);
    box-shadow: 0 16px 34px rgba(239, 68, 68, 0.18);
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

.mcp-workspace-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(1, 4, 9, 0.52);
    backdrop-filter: blur(8px);
    z-index: 35;
}

.mcp-workspace-drawer {
    position: absolute;
    top: 18px;
    right: 18px;
    bottom: 18px;
    width: min(420px, calc(100% - 36px));
    z-index: 40;
    animation: drawerIn 0.24s cubic-bezier(0.2, 0.9, 0.2, 1);
}

@keyframes drawerIn {
    from { opacity: 0; transform: translateX(20px); }
    to { opacity: 1; transform: translateX(0); }
}

.mcp-workspace-shell {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 22px;
    border-radius: 28px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background:
        radial-gradient(circle at top right, rgba(59, 130, 246, 0.18), transparent 34%),
        radial-gradient(circle at top left, rgba(148, 163, 184, 0.08), transparent 26%),
        linear-gradient(180deg, rgba(11, 15, 24, 0.96), rgba(8, 10, 17, 0.98));
    box-shadow: 0 26px 80px rgba(0, 0, 0, 0.55);
    overflow-y: auto;
}

.mcp-workspace-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
}

.mcp-workspace-kicker {
    display: inline-flex;
    align-items: center;
    height: 28px;
    padding: 0 12px;
    border-radius: 999px;
    background: rgba(59, 130, 246, 0.12);
    border: 1px solid rgba(59, 130, 246, 0.2);
    color: #93c5fd;
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
    margin-bottom: 12px;
}

.mcp-workspace-header h3 {
    margin: 0 0 6px 0;
    font-size: 1.4rem;
    line-height: 1.2;
    color: var(--text-pure);
}

.mcp-workspace-header p {
    margin: 0;
    color: rgba(255, 255, 255, 0.66);
    font-size: 0.88rem;
    line-height: 1.55;
}

.mcp-close-btn {
    width: 38px;
    height: 38px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 1.2rem;
    line-height: 1;
    transition: all 0.16s ease;
}

.mcp-close-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-pure);
}

.mcp-status-card,
.mcp-tool-description-card,
.mcp-empty-state-card {
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.03);
    padding: 16px;
}

.mcp-status-pill {
    display: inline-flex;
    align-items: center;
    min-height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    background: rgba(59, 130, 246, 0.14);
    color: #bfdbfe;
    border: 1px solid rgba(59, 130, 246, 0.24);
    font-size: 0.76rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
}

.mcp-status-copy {
    margin: 12px 0 0 0;
    color: rgba(255, 255, 255, 0.72);
    font-size: 0.84rem;
    line-height: 1.55;
    white-space: pre-wrap;
}

.mcp-status-copy.error {
    color: #fca5a5;
}

.mcp-toolbar {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.mcp-primary-btn {
    min-height: 44px;
    border-radius: 16px;
    border: 1px solid rgba(59, 130, 246, 0.22);
    background: linear-gradient(135deg, rgba(59, 130, 246, 0.22), rgba(15, 23, 42, 0.72));
    color: var(--text-pure);
    cursor: pointer;
    padding: 0 16px;
    font-size: 0.86rem;
    font-weight: 700;
    transition: all 0.18s ease;
}

.mcp-primary-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 16px 30px rgba(59, 130, 246, 0.18);
}

.mcp-primary-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
}

.mcp-primary-btn.run {
    margin-top: 4px;
}

.mcp-quick-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.mcp-chip-btn {
    height: 34px;
    padding: 0 14px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.78rem;
    font-weight: 600;
    transition: all 0.16s ease;
}

.mcp-chip-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.14);
    color: var(--text-pure);
}

.mcp-chip-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
}

.mcp-workspace-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px;
    border-radius: 22px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
}

.mcp-field-label {
    color: rgba(255, 255, 255, 0.56);
    font-size: 0.74rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
}

.mcp-tool-select {
    margin: 0;
}

.mcp-tool-args {
    min-height: 84px;
    margin: 0;
}

.mcp-tool-args.fancy {
    min-height: 136px;
    padding: 14px 18px;
    border-radius: 18px;
    background: rgba(7, 12, 20, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: var(--text-pure);
    box-sizing: border-box;
    line-height: 1.55;
    text-indent: 0;
    background-clip: padding-box;
}

.mcp-tool-help {
    margin: 0;
    color: var(--text-dim);
    font-size: 0.8rem;
    line-height: 1.5;
}

.mcp-settings-shell {
    display: grid;
    grid-template-columns: minmax(220px, 260px) minmax(0, 1fr);
    gap: 18px;
    align-items: start;
}

.mcp-settings-toolbar {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.mcp-server-list,
.mcp-server-editor {
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.03);
    padding: 16px;
}

.mcp-server-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.mcp-server-empty {
    color: var(--text-dim);
    font-size: 0.86rem;
}

.mcp-server-empty strong {
    color: var(--text-pure);
    display: block;
    margin-bottom: 6px;
}

.mcp-server-card {
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 18px;
    padding: 14px;
    background: rgba(8, 12, 20, 0.62);
    cursor: pointer;
    transition: all 0.16s ease;
}

.mcp-server-card:hover,
.mcp-server-card.selected {
    border-color: rgba(59, 130, 246, 0.24);
    background: rgba(12, 18, 28, 0.9);
}

.mcp-server-card-top,
.mcp-server-card-bottom {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
}

.mcp-server-card-top strong {
    color: var(--text-pure);
    font-size: 0.92rem;
}

.mcp-server-transport,
.mcp-server-badge {
    display: inline-flex;
    align-items: center;
    min-height: 26px;
    padding: 0 10px;
    border-radius: 999px;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
}

.mcp-server-transport {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-soft);
}

.mcp-server-badge.active {
    background: rgba(34, 197, 94, 0.18);
    color: #bbf7d0;
}

.mcp-server-badge.warning {
    background: rgba(245, 158, 11, 0.18);
    color: #fde68a;
}

.mcp-server-target {
    margin: 10px 0;
    color: rgba(255, 255, 255, 0.64);
    font-size: 0.8rem;
    line-height: 1.5;
    word-break: break-word;
}

.mcp-server-remove {
    border: none;
    background: transparent;
    color: #fca5a5;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0;
}

.mcp-server-editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.mcp-server-editor.empty {
    color: var(--text-dim);
    min-height: 180px;
    justify-content: center;
}

.mcp-editor-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
}

.mcp-server-editor-footer {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.mcp-active-switcher {
    margin-top: 14px;
}

.history-load-more {
    width: min(280px, calc(100% - 48px));
    margin-bottom: 24px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-soft);
    min-height: 40px;
    cursor: pointer;
}

.history-load-more:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-pure);
}

.mcp-result-card {
    width: 100%;
    max-width: 800px;
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 20px;
    background: rgba(10, 14, 22, 0.76);
    padding: 16px 18px;
}

.mcp-result-card.error {
    border-color: rgba(239, 68, 68, 0.24);
}

.mcp-result-kicker {
    display: inline-block;
    margin-bottom: 12px;
    color: #93c5fd;
    font-size: 0.74rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
}

.mcp-result-copy {
    margin: 0;
    color: rgba(255, 255, 255, 0.76);
    line-height: 1.6;
    white-space: pre-wrap;
}

.mcp-tool-grid,
.mcp-file-list,
.mcp-info-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.mcp-tool-chip-card,
.mcp-file-row,
.mcp-info-row {
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
    padding: 12px 14px;
}

.mcp-tool-chip-card strong,
.mcp-file-name,
.mcp-info-value {
    color: var(--text-pure);
}

.mcp-tool-chip-card p {
    margin: 6px 0 0 0;
    color: rgba(255, 255, 255, 0.68);
    line-height: 1.55;
}

.mcp-file-row,
.mcp-info-row {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: flex-start;
}

.mcp-file-kind,
.mcp-info-key {
    color: var(--text-dim);
    font-size: 0.76rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 700;
    flex: 0 0 auto;
}

.mcp-file-name,
.mcp-info-value {
    min-width: 0;
    text-align: right;
    word-break: break-word;
}

.mcp-tool-description-card p,
.mcp-empty-state-card p {
    margin: 8px 0 0 0;
    color: rgba(255, 255, 255, 0.72);
    line-height: 1.6;
}

.mcp-example-block {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.mcp-example-block pre {
    margin: 8px 0 0 0;
    padding: 12px 14px;
    border-radius: 14px;
    background: rgba(7, 12, 20, 0.86);
    border: 1px solid rgba(255, 255, 255, 0.06);
    color: #dbeafe;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
}

.mcp-empty-state-card h4 {
    margin: 0;
    color: var(--text-pure);
    font-size: 1rem;
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

.message-attachments {
    width: 100%;
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 16px;
}

.message-attachment-card {
    width: min(280px, 100%);
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    overflow: hidden;
}

.message-attachment-image {
    width: 100%;
    height: 160px;
    object-fit: cover;
    display: block;
    background: rgba(255, 255, 255, 0.02);
}

.message-attachment-file-badge {
    height: 80px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #bfdbfe;
    letter-spacing: 0.08em;
    font-size: 0.76rem;
    font-weight: 800;
    background: linear-gradient(135deg, rgba(59, 130, 246, 0.16), rgba(255, 255, 255, 0.02));
}

.message-attachment-copy {
    padding: 12px 14px 14px 14px;
}

.message-attachment-name {
    color: var(--text-pure);
    font-size: 0.86rem;
    font-weight: 700;
    margin-bottom: 4px;
    word-break: break-word;
}

.message-attachment-path {
    color: var(--text-dim);
    font-size: 0.74rem;
    line-height: 1.5;
    word-break: break-word;
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
    position: relative;
    flex-wrap: wrap;
}

.composer-tools-anchor {
    position: relative;
    flex: 0 0 auto;
}

.composer-plus-btn {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-pure);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.25rem;
    line-height: 1;
    transition: all 0.18s ease;
    margin-bottom: 2px;
}

.composer-plus-btn:hover:not(:disabled),
.composer-plus-btn.active {
    background: rgba(59, 130, 246, 0.18);
    border-color: rgba(59, 130, 246, 0.26);
    box-shadow: 0 12px 26px rgba(59, 130, 246, 0.18);
}

.composer-plus-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
}

.composer-tools-popover {
    position: absolute;
    left: 0;
    bottom: calc(100% + 12px);
    width: 300px;
    padding: 10px;
    border-radius: 22px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background:
        radial-gradient(circle at top left, rgba(59, 130, 246, 0.12), transparent 34%),
        linear-gradient(180deg, rgba(13, 16, 24, 0.98), rgba(9, 11, 18, 0.98));
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.48);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 45;
}

.composer-tool-item {
    width: 100%;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px;
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text-soft);
    cursor: pointer;
    text-align: left;
    transition: all 0.16s ease;
}

.composer-tool-item:hover:not(:disabled),
.composer-tool-item.active {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.12);
    color: var(--text-pure);
}

.composer-tool-item:disabled {
    opacity: 0.45;
    cursor: not-allowed;
}

.composer-tool-icon {
    width: 34px;
    height: 34px;
    border-radius: 12px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    background: rgba(59, 130, 246, 0.12);
    color: #bfdbfe;
    font-size: 0.95rem;
    font-weight: 700;
}

.composer-tool-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
}

.composer-tool-copy strong {
    font-size: 0.83rem;
    color: inherit;
}

.composer-tool-copy small {
    font-size: 0.73rem;
    color: var(--text-dim);
    line-height: 1.5;
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
    flex: 1 1 240px;
    background: transparent;
    border: none;
    color: white;
    font-size: 1rem;
    padding: 12px 16px;
    outline: none;
    resize: none;
    min-height: 44px;
}

.input-attachments-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 100%;
    overflow-x: auto;
    padding: 4px 4px 2px 0;
    min-width: 0;
}

.input-attachment-chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    max-width: 220px;
    padding: 6px 8px 6px 6px;
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    flex: 0 0 auto;
}

.input-attachment-preview,
.input-attachment-file {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    flex: 0 0 auto;
}

.input-attachment-preview {
    object-fit: cover;
    display: block;
    background: rgba(255, 255, 255, 0.04);
}

.input-attachment-file {
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(59, 130, 246, 0.14);
    color: #bfdbfe;
    font-size: 0.62rem;
    font-weight: 800;
    letter-spacing: 0.08em;
}

.input-attachment-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-soft);
    font-size: 0.78rem;
    font-weight: 600;
}

.input-attachment-remove {
    width: 24px;
    height: 24px;
    border-radius: 999px;
    border: none;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-soft);
    cursor: pointer;
    flex: 0 0 auto;
    line-height: 1;
}

.input-attachment-remove:hover {
    background: rgba(239, 68, 68, 0.16);
    color: #fff;
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

.send-button.big.stop-mode {
    background: rgba(239, 68, 68, 0.18);
    border: 1px solid rgba(239, 68, 68, 0.28);
}

.send-button.big.stop-mode:hover {
    background: rgba(239, 68, 68, 0.26);
}

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

.mcp-toggle-btn.active {
    background: rgba(59, 130, 246, 0.18);
    border-color: rgba(59, 130, 246, 0.28);
    color: #fff;
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
    gap: 12px;
}

.code-block-lang {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--accent-blue);
    font-weight: 700;
}

.code-block-actions {
    display: flex;
    gap: 8px;
}

.code-action-btn {
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-pure);
    border-radius: 999px;
    padding: 6px 12px;
    cursor: pointer;
    transition: all 0.16s ease;
}

.code-action-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
}

.code-action-btn:disabled {
    opacity: 0.58;
    cursor: not-allowed;
}

.code-run-note {
    margin: 0;
    padding: 10px 16px 12px 16px;
    color: var(--text-dim);
    font-size: 0.74rem;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
}

.code-run-note.warning {
    color: #fbbf24;
}

/* Execution Console */
.execution-console {
    background: #080808;
    border-top: 1px solid var(--border-glass);
    padding: 16px;
    font-family: var(--font-mono);
    font-size: 0.85rem;
}

.console-header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 12px;
    color: var(--text-dim);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}

.console-close {
    background: transparent;
    border: none;
    color: var(--text-dim);
    cursor: pointer;
}

.console-stdout {
    color: #d1d5db;
    white-space: pre-wrap;
    margin: 0 0 8px 0;
}

.console-stderr {
    color: var(--danger);
    white-space: pre-wrap;
    margin: 0 0 8px 0;
}

.console-footer {
    border-top: 1px solid rgba(255, 255, 255, 0.03);
    padding-top: 8px;
    font-size: 0.7rem;
    color: var(--text-dim);
    display: flex;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
}

/* Custom Scrollbar */
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

.settings-toggle-row {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.settings-toggle {
    width: fit-content;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-soft);
    padding: 10px 16px;
    cursor: pointer;
    font-weight: 600;
}

.settings-toggle.active {
    background: rgba(245, 158, 11, 0.16);
    border-color: rgba(245, 158, 11, 0.26);
    color: #fff;
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

.toast-host {
    position: fixed;
    top: 18px;
    right: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    z-index: 120;
    pointer-events: none;
}

.toast-card {
    width: min(360px, calc(100vw - 36px));
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 14px 14px 16px;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(10, 12, 20, 0.92);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.35);
    pointer-events: auto;
    backdrop-filter: blur(10px);
}

.toast-info {
    border-color: rgba(59, 130, 246, 0.28);
}

.toast-success {
    border-color: rgba(34, 197, 94, 0.28);
}

.toast-warning {
    border-color: rgba(245, 158, 11, 0.28);
}

.toast-error {
    border-color: rgba(239, 68, 68, 0.3);
}

.toast-copy {
    min-width: 0;
}

.toast-copy strong {
    display: block;
    margin-bottom: 4px;
    color: var(--text-pure);
    font-size: 0.86rem;
}

.toast-copy p {
    margin: 0;
    color: rgba(255, 255, 255, 0.72);
    font-size: 0.78rem;
    line-height: 1.55;
}

.toast-dismiss {
    width: 28px;
    height: 28px;
    border-radius: 999px;
    border: none;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-soft);
    cursor: pointer;
    flex: 0 0 auto;
}

.toast-dismiss:hover {
    background: rgba(255, 255, 255, 0.12);
    color: var(--text-pure);
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

    .mcp-workspace-drawer {
        top: 12px;
        right: 12px;
        bottom: 12px;
        width: min(430px, calc(100% - 24px));
    }

    .toast-host {
        top: 12px;
        right: 12px;
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

    .composer-tools-popover {
        left: -2px;
        width: min(300px, calc(100vw - 48px));
    }

    .chat-header-top {
        flex-direction: column;
    }

    .chat-header-actions {
        width: 100%;
        align-items: stretch;
    }

    .header-workspace-btn {
        min-width: 100%;
    }

    .mcp-workspace-drawer {
        inset: 12px;
        width: auto;
    }

    .mcp-workspace-shell {
        padding: 18px;
        border-radius: 22px;
    }

    .mcp-workspace-header {
        flex-direction: column;
    }

    .mcp-close-btn {
        align-self: flex-end;
    }

    .settings-modal {
        padding: 24px;
        border-radius: 18px;
    }

    .mcp-settings-shell,
    .mcp-editor-grid {
        grid-template-columns: 1fr;
    }
}

/* Custom Scrollbar */
::-webkit-scrollbar { width: 5px; }
::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.1); border-radius: 10px; }
"#;
