//! Browser-side JS page functions shared verbatim with TypeScript & Python `myuri`.

pub const RESOLVE_SELECTOR_JS: &str = r#"(selector) => {
    function resolve(s) {
        if (s.includes('>>')) {
            const tokens = s.split(/\s*(>>>|>>)\s*/).filter(Boolean);
            let root = document;
            let el = null;
            let hop = '';
            for (const t of tokens) {
                if (t === '>>' || t === '>>>') {
                    hop = t;
                    continue;
                }
                if (el) {
                    if (hop === '>>>') {
                        let doc = null;
                        try { doc = el.contentDocument; } catch { doc = null; }
                        if (!doc) return null;
                        root = doc;
                    } else {
                        root = el.shadowRoot || el;
                    }
                }
                el = root.querySelector(t);
                if (!el) return null;
            }
            return el;
        }
        if (s.indexOf('xpath=') === 0 || s[0] === '/' || s[0] === '(') {
            const expr = s.indexOf('xpath=') === 0 ? s.slice(6) : s;
            try { return document.evaluate(expr, document, null, 9, null).singleNodeValue; } catch { return null; }
        }
        const direct = document.querySelector(s);
        if (direct) return direct;
        const stack = [document];
        while (stack.length) {
            const root = stack.pop();
            const found = root.querySelector(s);
            if (found) return found;
            root.querySelectorAll('*').forEach((e) => { if (e.shadowRoot) stack.push(e.shadowRoot); });
        }
        return null;
    }
    return resolve(selector);
}"#;

pub const HIT_TEST_JS: &str = r#"(e, at) => {
    const field = e.closest('label')?.control || e;
    const target = e.matches('input, textarea, select') ? e : e.closest('button, [role=button], a, [role=link]') || e;
    if (at?.pressing && (target.matches(':disabled') || target.closest('[aria-disabled="true"]')))
        return 'element is not enabled';
    let x = 0;
    let y = 0;
    if (at) {
        x = at.x;
        y = at.y;
        for (let win = e.ownerDocument.defaultView; win && win !== window && win.frameElement; win = win.parent) {
            const fe = win.frameElement;
            const fr = fe.getBoundingClientRect();
            x -= fr.x + fe.clientLeft;
            y -= fr.y + fe.clientTop;
        }
    } else {
        const r = e.getBoundingClientRect();
        x = r.left + r.width / 2;
        y = r.top + r.height / 2;
    }
    let hit = e.ownerDocument.elementFromPoint(x, y);
    while (hit?.shadowRoot) {
        const inner = hit.shadowRoot.elementFromPoint(x, y);
        if (!inner || inner === hit) break;
        hit = inner;
    }
    for (let n = hit; n; n = n.parentNode || n.host || null) {
        if (n === e || n === target || n === field || n.control === field)
            return null;
    }
    return hit ? `<${hit.localName}> intercepts pointer events` : 'element is not at the point';
}"#;

pub const BOX_JS: &str = r#"(el) => {
    const r = el.getBoundingClientRect();
    if (r.width === 0 && r.height === 0) return null;
    let x = r.x;
    let y = r.y;
    for (let win = el.ownerDocument.defaultView; win && win !== window && win.frameElement; win = win.parent) {
        const fe = win.frameElement;
        const fr = fe.getBoundingClientRect();
        x += fr.x + fe.clientLeft;
        y += fr.y + fe.clientTop;
    }
    return { x, y, width: r.width, height: r.height };
}"#;

pub const FOCUSED_FIELD_JS: &str = r#"() => {
    let el = document.activeElement;
    let ox = 0;
    let oy = 0;
    for (;;) {
        if (!el) return null;
        if (el.shadowRoot?.activeElement) {
            el = el.shadowRoot.activeElement;
        } else if (el.tagName === 'IFRAME' || el.tagName === 'FRAME') {
            let doc = null;
            try { doc = el.contentDocument; } catch { doc = null; }
            if (!doc) return null;
            const fr = el.getBoundingClientRect();
            ox += fr.x + el.clientLeft;
            oy += fr.y + el.clientTop;
            el = doc.activeElement;
        } else {
            break;
        }
    }
    const doc = el.ownerDocument;
    if (el === doc.body && !el.isContentEditable) return null;
    const r = el.getBoundingClientRect();
    const cs = (doc.defaultView ?? window).getComputedStyle(el);
    const isTextControl = el.tagName === 'INPUT' || el.tagName === 'TEXTAREA';
    const text = isTextControl ? el.value : el.innerText.replace(/\n+$/, '');
    const lineHeight = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.2;
    let firstLine = {
        x: r.left + parseFloat(cs.borderLeftWidth) + parseFloat(cs.paddingLeft) + 6,
        y: r.top + parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop) - el.scrollTop + lineHeight / 2,
    };
    const contentLeft = r.left + parseFloat(cs.borderLeftWidth) + parseFloat(cs.paddingLeft);
    const lineTop = r.top + parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop) - el.scrollTop;
    let span = { left: contentLeft, right: contentLeft, top: lineTop, bottom: lineTop + lineHeight };
    if (el.tagName === 'TEXTAREA') {
        const ctx = doc.createElement('canvas').getContext('2d');
        if (ctx) {
            ctx.font = cs.font;
            span.right = contentLeft + ctx.measureText(text.split('\n')[0]).width;
        }
    }
    let selection;
    if (isTextControl) {
        selection = el.selectionStart == null || (el.selectionStart === 0 && el.selectionEnd === text.length)
            ? 'all'
            : 'partial';
    } else {
        selection = String(doc.getSelection()).trim() === el.innerText.trim() ? 'all' : 'partial';
        const walker = doc.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
            const i = (n.textContent ?? '').search(/\S/);
            if (i < 0) continue;
            const range = doc.createRange();
            range.setStart(n, i);
            range.setEnd(n, i + 1);
            const c = range.getBoundingClientRect();
            firstLine = { x: c.left + c.width / 2 + 4, y: c.top + c.height / 2 };
            range.setEnd(n, (n.textContent ?? '').length);
            const line = range.getClientRects()[0] ?? c;
            span = { left: c.left, right: line.right, top: line.top, bottom: line.bottom };
            break;
        }
    }
    return {
        box: { x: r.x + ox, y: r.y + oy, width: r.width, height: r.height },
        empty: text.trim() === '',
        selection,
        lines: text === '' ? 1 : text.split('\n').length,
        singleLine: el.tagName === 'INPUT',
        firstLineSpan: { left: span.left + ox, right: span.right + ox, top: span.top + oy, bottom: span.bottom + oy },
        visualLines: Math.ceil(el.scrollHeight / lineHeight),
        scrollTop: el.scrollTop,
        firstLine: { x: firstLine.x + ox, y: firstLine.y + oy },
    };
}"#;

/// Composes a page function `(el, arg) => ...` on a resolved selector without using `eval` under strict CSP.
pub fn on_selector_script(fn_source: &str) -> String {
    format!("(a) => {{ const el = ({RESOLVE_SELECTOR_JS})(a.selector); return el ? ({fn_source})(el, a.arg) : null; }}")
}
