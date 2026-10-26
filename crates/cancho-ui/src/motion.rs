//! The script that watches a page for what its elements say about being seen (docs/motion.md): `Reveal` shows a block when it has been seen,
//! `CountUp` counts a number up when it has been seen, and `visible:` / `hidden:` tell the program. Inline in `index.html`, after the page's own HTML, so
//! it runs before `app.js` is fetched and works on the elements the page was written with. Everything it does to a page is in the CSS and the
//! script here; the rules that keep it safe are measured by `scripts/check-motion.js`.

/// What a program uses of it.
#[derive(Default, Clone, Copy)]
pub struct MotionUse {
    pub reveal: bool,
    pub countup: bool,
    pub visible: bool,
}

impl MotionUse {
    pub fn any(&self) -> bool {
        self.reveal || self.countup || self.visible
    }
}

/// What the program uses, by reading its views.
pub fn uses(ir: &cancho_ui_ir::ir::IrFile) -> MotionUse {
    fn walk(n: &cancho_ui_ir::ir::IrViewNode, u: &mut MotionUse) {
        use cancho_ui_ir::ir::{IrElementKind, IrViewNode};
        match n {
            IrViewNode::Element {
                kind,
                bindings,
                children,
                ..
            } => {
                match kind {
                    IrElementKind::Reveal => u.reveal = true,
                    IrElementKind::CountUp => u.countup = true,
                    _ => {}
                }
                if bindings
                    .iter()
                    .any(|b| b.key == "visible" || b.key == "hidden")
                {
                    u.visible = true;
                }
                children.iter().for_each(|c| walk(c, u));
            }
            IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
                children.iter().for_each(|c| walk(c, u))
            }
            IrViewNode::Text { .. } => {}
        }
    }
    let mut u = MotionUse::default();
    for c in &ir.components {
        walk(&c.view.body, &mut u);
    }
    u
}

/// The script. Three parts, each only for what the program uses.
///
/// * **seen(e, f)**: an element has been seen when it intersects the window and either a fraction `f` of it, or a fraction `f` of the window's
///   height, is inside. The second half is for an element taller than the window: a fraction of *it* can never be inside, and one that waits for that
///   is never shown.
/// * **Reveal** is armed (`cui-armed`) only for the elements that are in the page's HTML now and only when the browser has `IntersectionObserver`
///   and the visitor has not asked for reduced motion; the CSS hides an element only while it is armed and not yet `cui-in`. An element the script
///   does not know (one built later) is never hidden.
/// * **CountUp** keeps its text, which is its final number, from the first byte to the last: the frames are drawn over it by CSS (`content` with an empty
///   alternative text, so a screen reader does not read them), and only where the browser supports that.
/// * **Visible** tells the program by the name of the event written on the element, once when it is seen and once when it is left; what is
///   told before `app.js` has run is kept and told when it has.
pub fn script(u: MotionUse) -> String {
    let mut s = String::from(
        "<script>(function(){var d=document,w=window,reduce=w.matchMedia&&w.matchMedia('(prefers-reduced-motion: reduce)').matches,io=typeof IntersectionObserver==='function',T=[];for(var i=0;i<=100;i++){T.push(i/100)}function seen(e,f){var vh=(e.rootBounds&&e.rootBounds.height)||w.innerHeight;return e.isIntersecting&&(e.intersectionRatio>=f||e.intersectionRect.height>=f*vh)}function all(q){return [].slice.call(d.querySelectorAll(q))}",
    );
    if u.reveal {
        s.push_str("try{if(io&&!reduce){var rv=all('.cui-reveal'),ro=new IntersectionObserver(function(es){es.forEach(function(e){if(seen(e,.25)){e.target.classList.add('cui-in');ro.unobserve(e.target)}})},{threshold:T});rv.forEach(function(e){ro.observe(e);e.classList.add('cui-armed')})}}catch(x){all('.cui-armed').forEach(function(e){e.classList.remove('cui-armed')})}");
    }
    if u.countup {
        s.push_str("try{if(io&&!reduce&&w.CSS&&CSS.supports&&CSS.supports('content','\"a\" / \"\"')){var co=new IntersectionObserver(function(es){es.forEach(function(e){if(seen(e,.4)){co.unobserve(e.target);up(e.target)}})},{threshold:T});all('.cui-countup[data-to]').forEach(function(e){co.observe(e)});function up(el){var to=+el.getAttribute('data-to'),t0=performance.now();el.style.setProperty('--cui-cu',getComputedStyle(el).color);el.setAttribute('data-frame','0');(function tick(){var k=Math.min(1,(performance.now()-t0)/1300);el.setAttribute('data-frame',String(Math.round(to*(1-Math.pow(1-k,3)))));if(k<1){requestAnimationFrame(tick)}else{el.removeAttribute('data-frame');el.style.removeProperty('--cui-cu')}})()}}}catch(x){}");
    }
    if u.visible {
        s.push_str("try{var vs=all('[data-cui-visible],[data-cui-hidden]');function tell(n){if(w.__cui_vis_live){w.__cui_vis_live(n)}else{(w.__cui_vis_q=w.__cui_vis_q||[]).push(n)}}if(!io){vs.forEach(function(e){var n=e.getAttribute('data-cui-visible');if(n){tell(n)}})}else{var st=new WeakMap(),vo=new IntersectionObserver(function(es){es.forEach(function(e){var on=seen(e,.5),k=e.target;if(!st.has(k)){st.set(k,on);if(on){var a=k.getAttribute('data-cui-visible');if(a){tell(a)}}return}if(st.get(k)===on){return}st.set(k,on);var n=k.getAttribute(on?'data-cui-visible':'data-cui-hidden');if(n){tell(n)}})},{threshold:T});vs.forEach(function(e){vo.observe(e)})}}catch(x){}");
    }
    s.push_str("})()</script>\n");
    s
}

/// The rules of the baseline stylesheet for motion.
pub const CSS: &str = "/* motion (docs/motion.md): an element is hidden only while the page's script has it armed and has not yet seen it, and never when the visitor asks for less motion */
.cui-reveal.cui-armed { transition: opacity 0.7s ease, transform 0.7s ease; }
.cui-reveal.cui-armed:not(.cui-in) { opacity: 0; transform: translateY(18px); }
.cui-countup { font-variant-numeric: tabular-nums; }
.cui-countup[data-frame] { position: relative; color: transparent; }
.cui-countup[data-frame]::after { content: attr(data-frame) / \"\"; position: absolute; left: 0; top: 0; color: var(--cui-cu); }
@media (prefers-reduced-motion: reduce), print {
    .cui-reveal.cui-armed { transition: none; }
    .cui-reveal.cui-armed:not(.cui-in) { opacity: 1; transform: none; }
    .cui-countup[data-frame] { color: inherit; }
    .cui-countup[data-frame]::after { content: none; }
}
";
