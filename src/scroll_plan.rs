use serde::{Deserialize, Serialize};

use crate::geometry::VIEWPORT_PADDING;
use crate::math::BoundingBox;

pub const SCROLL_PAD: f64 = if VIEWPORT_PADDING + 5.0 > 20.0 {
    VIEWPORT_PADDING + 5.0
} else {
    20.0
};
pub const SCROLL_EDGE: f64 = VIEWPORT_PADDING;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollPlanArgs {
    pub finish_position: f64,
    pub pad: f64,
    pub edge: f64,
    pub from_x: f64,
    pub from_y: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollPlan {
    pub in_view: bool,
    pub delta_y: f64,
    pub hover_x: f64,
    pub hover_y: f64,
    pub is_page: bool,
    pub region: Option<BoundingBox>,
    pub box_rect: Option<BoundingBox>,
    pub borders: [f64; 2],
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawScrollPlan {
    pub in_view: bool,
    pub delta_y: f64,
    pub hover_x: f64,
    pub hover_y: f64,
    pub is_page: bool,
    pub region: Option<BoundingBox>,
    #[serde(rename = "box")]
    pub box_rect: BoundingBox,
    pub borders: [f64; 2],
}

pub const SCROLL_PLAN_JS: &str = r#"(el, arg) => {
    const frameOffset = (node) => {
        let x = 0;
        let y = 0;
        let win = node.ownerDocument ? node.ownerDocument.defaultView : null;
        while (win && win !== window && win.frameElement) {
            const fe = win.frameElement;
            const fr = fe.getBoundingClientRect();
            x += fr.x + fe.clientLeft;
            y += fr.y + fe.clientTop;
            win = win.parent;
        }
        return { x, y };
    };
    const rectOf = (node) => {
        const r = node.getBoundingClientRect();
        const o = frameOffset(node);
        return {
            x: r.x + o.x, y: r.y + o.y, width: r.width, height: r.height,
            top: r.top + o.y, bottom: r.bottom + o.y, left: r.left + o.x, right: r.right + o.x,
        };
    };
    const styleOf = (node) => ((node.ownerDocument && node.ownerDocument.defaultView) || window).getComputedStyle(node);
    const isFrameDocRoot = (node) => !!node.ownerDocument &&
        node === node.ownerDocument.documentElement &&
        node.ownerDocument.defaultView !== window;
    const stepUp = (node) => {
        if (node.parentElement) return node.parentElement;
        const r = node.getRootNode();
        if (r.host) return r.host;
        try { return r.defaultView?.frameElement ?? null; } catch { return null; }
    };
    const vh = window.innerHeight || document.documentElement.clientHeight;
    const vw = window.innerWidth || document.documentElement.clientWidth;
    const elRect = rectOf(el);
    const elStyle = styleOf(el);
    if (!(elRect.width > 0 && elRect.height > 0) || elStyle.visibility === 'hidden') return null;
    const chain = [];
    let pageCanMoveTarget = true;
    let cur = stepUp(el);
    let depth = 0;
    while (cur && depth < 60 && cur !== document.documentElement) {
        if (isFrameDocRoot(cur)) {
            const w = cur.ownerDocument.defaultView;
            if (cur.scrollHeight > w.innerHeight + 1) chain.push(cur);
            cur = stepUp(cur);
            depth++;
            continue;
        }
        const st = styleOf(cur);
        const scrollable = cur.scrollHeight > cur.clientHeight + 1 &&
            (st.overflowY === 'scroll' || st.overflowY === 'auto');
        if (cur.ownerDocument && cur === cur.ownerDocument.body) {
            const docRoot = cur.ownerDocument.documentElement;
            const docVh = cur.ownerDocument.defaultView?.innerHeight ?? vh;
            if (scrollable && !(docRoot.scrollHeight > docVh + 10)) chain.push(cur);
            if (cur === document.body) break;
            cur = stepUp(cur);
            depth++;
            continue;
        }
        if (scrollable) chain.push(cur);
        if (st.position === 'fixed') {
            if (cur.ownerDocument === document) {
                pageCanMoveTarget = false;
                break;
            }
            try { cur = cur.ownerDocument.defaultView?.frameElement ?? null; } catch { cur = null; }
            depth++;
            continue;
        }
        cur = stepUp(cur);
        depth++;
    }
    const hitAtPoint = (x, y) => {
        let hit = document.elementFromPoint(x, y);
        for (;;) {
            if (!hit || (hit.tagName !== 'IFRAME' && hit.tagName !== 'FRAME')) return hit;
            let inner = null;
            try { inner = hit.contentDocument; } catch { inner = null; }
            if (!inner) return hit;
            const fr = hit.getBoundingClientRect();
            x -= fr.x + hit.clientLeft;
            y -= fr.y + hit.clientTop;
            const next = inner.elementFromPoint(x, y);
            if (!next) return hit;
            hit = next;
        }
    };
    const containsDeep = (ancestor, node) => {
        let n = node;
        while (n) {
            if (n === ancestor) return true;
            n = stepUp(n);
        }
        return false;
    };
    const pinnedRectAt = (x, y) => {
        const hit = hitAtPoint(x, y);
        if (!hit || hit === el || containsDeep(el, hit) || containsDeep(hit, el)) return null;
        let n = hit;
        while (n && n !== document.body && n !== document.documentElement) {
            if (isFrameDocRoot(n) || (n.ownerDocument && n === n.ownerDocument.body)) {
                n = stepUp(n);
                continue;
            }
            const pos = styleOf(n).position;
            if (pos === 'fixed' || pos === 'sticky') {
                return containsDeep(n, el) ? null : rectOf(n);
            }
            n = stepUp(n);
        }
        return null;
    };
    let clearTop = arg.pad;
    let clearBottom = vh - arg.pad;
    const probeX = Math.min(Math.max(elRect.left + elRect.width / 2, 5), vw - 5);
    for (let i = 0; i < 4; i++) {
        const r = pinnedRectAt(probeX, clearTop + 1);
        if (r && r.bottom > clearTop) clearTop = Math.min(r.bottom + 2, vh);
        else break;
    }
    for (let i = 0; i < 4; i++) {
        const r = pinnedRectAt(probeX, clearBottom - 1);
        if (r && r.top < clearBottom) clearBottom = Math.max(r.top - 2, 0);
        else break;
    }
    if (clearBottom - clearTop < 40) {
        clearTop = arg.pad;
        clearBottom = vh - arg.pad;
    }
    const scrollerRectOf = (c) => {
        if (isFrameDocRoot(c)) {
            try {
                const fe = c.ownerDocument.defaultView?.frameElement;
                if (fe) return rectOf(fe);
            } catch {}
        }
        return rectOf(c);
    };
    let clipTop = clearTop;
    let clipBottom = clearBottom;
    let clipLeft = arg.edge;
    let clipRight = vw - arg.edge;
    for (const c of chain) {
        const cr = scrollerRectOf(c);
        clipTop = Math.max(clipTop, cr.top);
        clipBottom = Math.min(clipBottom, cr.bottom);
        clipLeft = Math.max(clipLeft, cr.left);
        clipRight = Math.min(clipRight, cr.right);
    }
    const regionLeft = Math.max(elRect.left, clipLeft);
    const regionTop = Math.max(elRect.top, clipTop);
    const regionRight = Math.min(elRect.right, clipRight);
    const regionBottom = Math.min(elRect.bottom, clipBottom);
    const region = regionRight - regionLeft >= 6 && regionBottom - regionTop >= 6
        ? { x: regionLeft, y: regionTop, width: regionRight - regionLeft, height: regionBottom - regionTop }
        : null;
    let occluded = false;
    if (region) {
        const cx = region.x + region.width / 2;
        const cy = region.y + region.height / 2;
        const samples = [
            [cx, cy],
            [cx - region.width * 0.3, cy],
            [cx + region.width * 0.3, cy],
            [cx, cy - region.height * 0.3],
            [cx, cy + region.height * 0.3],
        ];
        occluded = samples.some(([px, py]) => pinnedRectAt(Math.round(px), Math.round(py)) != null);
    }
    const bandHeight = clipBottom - clipTop;
    const fullyVisible = elRect.top >= clipTop && elRect.bottom <= clipBottom;
    const tallTarget = elRect.height > bandHeight - 4;
    const inView = region != null && !occluded && (fullyVisible || (tallTarget && region.height >= bandHeight * 0.6));
    let chosen = null;
    let chosenDelta = 0;
    let chosenIsPage = false;
    let hoverTop = clearTop;
    let hoverBottom = clearBottom;
    let hoverLeft = 5;
    let hoverRight = vw - 5;
    const steps = chain.length + (pageCanMoveTarget ? 1 : 0);
    for (let i = 0; i < steps; i++) {
        const isPageStep = i === chain.length;
        const node = isPageStep ? document.documentElement : chain[i];
        let bTop = clearTop;
        let bBottom = clearBottom;
        let bLeft = 5;
        let bRight = vw - 5;
        if (!isPageStep) {
            const r = scrollerRectOf(node);
            bTop = Math.max(bTop, r.top);
            bBottom = Math.min(bBottom, r.bottom);
            bLeft = Math.max(bLeft, r.left);
            bRight = Math.min(bRight, r.right);
            for (let j = i + 1; j < chain.length; j++) {
                const outer = scrollerRectOf(chain[j]);
                bTop = Math.max(bTop, outer.top);
                bBottom = Math.min(bBottom, outer.bottom);
                bLeft = Math.max(bLeft, outer.left);
                bRight = Math.min(bRight, outer.right);
            }
        }
        const bH = bBottom - bTop;
        if (bH < Math.min(elRect.height, 24) + 6) continue;
        const desiredTop = bTop + Math.max(0, bH - elRect.height) * arg.finishPosition;
        const frameWin = isFrameDocRoot(node) ? node.ownerDocument.defaultView : null;
        const current = isPageStep ? window.scrollY : frameWin ? frameWin.scrollY : node.scrollTop;
        const maxScroll = isPageStep
            ? Math.max(0, document.documentElement.scrollHeight - vh)
            : frameWin
                ? Math.max(0, node.scrollHeight - frameWin.innerHeight)
                : Math.max(0, node.scrollHeight - node.clientHeight);
        const target = Math.max(0, Math.min(current + (elRect.top - desiredTop), maxScroll));
        const delta = target - current;
        if (Math.abs(delta) >= 1) {
            chosen = node;
            chosenDelta = delta;
            chosenIsPage = isPageStep;
            hoverTop = bTop;
            hoverBottom = bBottom;
            hoverLeft = bLeft;
            hoverRight = bRight;
            break;
        }
    }
    const clampHX = (x) => Math.min(Math.max(x, Math.max(hoverLeft + 5, 5)), Math.min(hoverRight - 5, vw - 5));
    const clampHY = (y) => Math.min(Math.max(y, Math.max(hoverTop + 5, 5)), Math.min(hoverBottom - 5, vh - 5));
    const consumesWheel = (n) => {
        if (isFrameDocRoot(n)) {
            const w = n.ownerDocument.defaultView;
            return chosenDelta > 0 ? w.scrollY + w.innerHeight < n.scrollHeight - 1 : w.scrollY > 1;
        }
        const st = styleOf(n);
        if (st.overflowY !== 'auto' && st.overflowY !== 'scroll') return false;
        if (n.scrollHeight <= n.clientHeight + 1) return false;
        return chosenDelta > 0 ? n.scrollTop + n.clientHeight < n.scrollHeight - 1 : n.scrollTop > 1;
    };
    const wheelReaches = (x, y) => {
        const hit = hitAtPoint(x, y);
        if (!hit) return chosenIsPage;
        let n = hit;
        while (n && n !== document.documentElement && n !== document.body) {
            if (!chosenIsPage && n === chosen) return true;
            if (consumesWheel(n)) return false;
            n = stepUp(n);
        }
        return chosenIsPage;
    };
    let hoverX = clampHX(arg.fromX);
    let hoverY = clampHY(arg.fromY);
    if (chosen) {
        const roomLeft = arg.fromX - Math.max(hoverLeft + 5, 5);
        const roomRight = Math.min(hoverRight - 5, vw - 5) - arg.fromX;
        const lateral = roomRight > roomLeft ? [0, 60, -60, 120, -120, 200, -200] : [0, -60, 60, -120, 120, -200, 200];
        const baseY = hoverY;
        probe: for (const dy of [0, -40, 40, -90, 90]) {
            const y = clampHY(baseY + dy);
            for (const dx of lateral) {
                const x = clampHX(arg.fromX + dx);
                if (wheelReaches(x, y)) {
                    hoverX = x;
                    hoverY = y;
                    break probe;
                }
            }
        }
    }
    return {
        inView,
        deltaY: chosenDelta,
        hoverX,
        hoverY,
        isPage: chosenIsPage,
        region,
        box: { x: elRect.x, y: elRect.y, width: elRect.width, height: elRect.height },
        borders: [parseInt(elStyle.borderLeftWidth, 10), parseInt(elStyle.borderTopWidth, 10)],
    };
}"#;
