const el = document.getElementById('penumbra-editor');
if (!el) {
  dioxus.send('__ERROR__:editor-element-not-found');
} else {
  try {
    // Inject toolbar styles once per page lifetime
    if (!document.getElementById('penumbra-tb-style')) {
      const s = document.createElement('style');
      s.id = 'penumbra-tb-style';
      s.textContent = `
        .pnb-toolbar {
          display: flex; flex-wrap: wrap; gap: 2px;
          padding: 0 0 10px 0; margin-bottom: 4px;
          border-bottom: 1px solid var(--border, rgba(99,148,220,0.12));
        }
        .pnb-toolbar button {
          display: flex; align-items: center; justify-content: center;
          width: 26px; height: 26px; border: none; border-radius: 5px;
          background: transparent; color: var(--text-dim, rgba(200,220,255,0.55));
          cursor: pointer; font-size: 12px; font-family: inherit;
          transition: background 120ms, color 120ms;
        }
        .pnb-toolbar button:hover { background: var(--accent-soft, rgba(99,148,220,0.15)); color: var(--text, #c8e3ff); }
        .pnb-toolbar button.is-active { background: var(--accent-soft, rgba(99,148,220,0.22)); color: var(--accent-bright, #7abaff); }
        .pnb-toolbar .tb-sep {
          width: 1px; height: 18px;
          background: var(--border, rgba(99,148,220,0.15)); margin: 0 3px; align-self: center;
        }
      `;
      document.head.appendChild(s);
    }

    // Destroy any stale editor from a previous eval run on this element
    if (el._penumbra_editor) {
      try { el._penumbra_editor.destroy(); } catch(_) {}
      delete el._penumbra_editor;
    }
    const oldTb = el.parentNode && el.parentNode.querySelector('.pnb-toolbar');
    if (oldTb) oldTb.remove();

    const [tiptap, starter] = await Promise.all([
      import('https://esm.sh/@tiptap/core@3.26.0'),
      import('https://esm.sh/@tiptap/starter-kit@3.26.0'),
    ]);
    const Editor = tiptap.Editor;
    const StarterKit = starter.default;

    const initialContent = await dioxus.recv();

    // Build toolbar
    const toolbar = document.createElement('div');
    toolbar.className = 'pnb-toolbar';

    const TOOLS = [
      { cmd: 'bold',        html: '<b>B</b>',           title: 'Bold (Ctrl+B)' },
      { cmd: 'italic',      html: '<i>I</i>',            title: 'Italic (Ctrl+I)' },
      { cmd: 'strike',      html: '<s>S</s>',            title: 'Strikethrough' },
      { cmd: 'code',        html: '<code style="font-size:10px">&lt;/&gt;</code>', title: 'Inline code' },
      { sep: true },
      { cmd: 'h1',          html: 'H1',                  title: 'Heading 1' },
      { cmd: 'h2',          html: 'H2',                  title: 'Heading 2' },
      { cmd: 'h3',          html: 'H3',                  title: 'Heading 3' },
      { sep: true },
      { cmd: 'bulletList',  html: '&#9679;&#9552;',      title: 'Bullet list' },
      { cmd: 'orderedList', html: '1&#9552;',            title: 'Ordered list' },
      { sep: true },
      { cmd: 'blockquote',  html: '&#10077;',            title: 'Blockquote' },
      { cmd: 'codeBlock',   html: '{ }',                 title: 'Code block' },
      { sep: true },
      { cmd: 'hr',          html: '&#8212;',             title: 'Horizontal rule' },
    ];

    for (const t of TOOLS) {
      if (t.sep) {
        const sep = document.createElement('div');
        sep.className = 'tb-sep';
        toolbar.appendChild(sep);
      } else {
        const btn = document.createElement('button');
        btn.dataset.cmd = t.cmd;
        btn.title = t.title;
        btn.innerHTML = t.html;
        toolbar.appendChild(btn);
      }
    }

    el.parentNode.insertBefore(toolbar, el);

    function updateToolbar(editor) {
      toolbar.querySelectorAll('button[data-cmd]').forEach(btn => {
        const c = btn.dataset.cmd;
        let active = false;
        if (c === 'h1') active = editor.isActive('heading', { level: 1 });
        else if (c === 'h2') active = editor.isActive('heading', { level: 2 });
        else if (c === 'h3') active = editor.isActive('heading', { level: 3 });
        else if (c === 'bulletList') active = editor.isActive('bulletList');
        else if (c === 'orderedList') active = editor.isActive('orderedList');
        else if (c === 'codeBlock') active = editor.isActive('codeBlock');
        else if (c === 'blockquote') active = editor.isActive('blockquote');
        else active = editor.isActive(c);
        btn.classList.toggle('is-active', active);
      });
    }

    const editor = new Editor({
      element: el,
      extensions: [StarterKit.configure({ heading: { levels: [1, 2, 3] } })],
      content: initialContent || '<p></p>',
      onUpdate({ editor }) { dioxus.send(editor.getHTML()); },
      onSelectionUpdate({ editor }) { updateToolbar(editor); },
      onCreate({ editor }) { updateToolbar(editor); },
    });

    el._penumbra_editor = editor;

    toolbar.addEventListener('mousedown', e => {
      const btn = e.target.closest('button[data-cmd]');
      if (!btn) return;
      e.preventDefault(); // Keep editor focus
      const c = btn.dataset.cmd;
      const chain = editor.chain().focus();
      switch (c) {
        case 'bold':         chain.toggleBold().run(); break;
        case 'italic':       chain.toggleItalic().run(); break;
        case 'strike':       chain.toggleStrike().run(); break;
        case 'code':         chain.toggleCode().run(); break;
        case 'h1':           chain.toggleHeading({ level: 1 }).run(); break;
        case 'h2':           chain.toggleHeading({ level: 2 }).run(); break;
        case 'h3':           chain.toggleHeading({ level: 3 }).run(); break;
        case 'bulletList':   chain.toggleBulletList().run(); break;
        case 'orderedList':  chain.toggleOrderedList().run(); break;
        case 'blockquote':   chain.toggleBlockquote().run(); break;
        case 'codeBlock':    chain.toggleCodeBlock().run(); break;
        case 'hr':           chain.setHorizontalRule().run(); break;
      }
    });

    while (true) {
      const msg = await dioxus.recv();
      if (msg === '__DESTROY__') {
        editor.destroy();
        toolbar.remove();
        break;
      }
    }
  } catch (err) {
    dioxus.send('__ERROR__:tiptap-init-failed:' + err.message);
  }
}
