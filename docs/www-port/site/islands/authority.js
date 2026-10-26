// The canvas of the authority animation (the page's assets/site.js, `authority()`): the world splits into five capabilities, which are handed on or released.
// The page keeps everything that is text (the tabs, the code, the captions, the verdict, the buttons); this file draws where the five rocks are for the scenario and
// the step it is told (`scenario`, `step`, props that the page reads from its state). Positions and sets are the page's own numbers.
(function () {
    var R = window.Rocks, el = R.el;
    var W = [400, 52];
    var row = { io: [130, 150], fs: [265, 150], ffi: [400, 150], heap: [535, 150], args: [670, 150] };
    var ground = { fs: [70, 282], ffi: [130, 282], heap: [190, 282], args: [250, 282], io: [310, 282] };
    var inBox = [700, 262];
    var names = ['io', 'fs', 'ffi', 'heap', 'args'];
    function put(o) { var r = {}; names.forEach(function (n) { r[n] = o[n] || ground[n]; }); return r; }
    var scenarios = [
        { greet: '[io_write]', lab: 'greet(io)', steps: [
            { at: {}, vis: [] },
            { at: row, vis: names },
            { at: { io: row.io, fs: ground.fs, ffi: ground.ffi, heap: ground.heap, args: ground.args }, vis: names },
            { at: { io: inBox, fs: ground.fs, ffi: ground.ffi, heap: ground.heap, args: ground.args }, vis: names, zone: 'on' },
            { at: ground, vis: names }] },
        { greet: '[io_write]', lab: 'greet(io)', steps: [
            { at: {}, vis: [] },
            { at: row, vis: names },
            { at: { io: row.io, fs: ground.fs, ffi: ground.ffi, heap: row.heap, args: ground.args }, vis: names, bad: ['heap'] },
            { at: { io: inBox, fs: ground.fs, ffi: ground.ffi, heap: row.heap, args: ground.args }, vis: names, bad: ['heap'], zone: 'on' },
            { at: { io: ground.io, fs: ground.fs, ffi: ground.ffi, heap: row.heap, args: ground.args }, vis: names, bad: ['heap'] }] },
        { greet: '[]', lab: 'greet(io) -> []', steps: [
            { at: {}, vis: [] },
            { at: { io: row.io, fs: ground.fs, ffi: ground.ffi, heap: ground.heap, args: ground.args }, vis: names },
            { at: { io: inBox, fs: ground.fs, ffi: ground.ffi, heap: ground.heap, args: ground.args }, vis: names, zone: 'bad' },
            { at: { io: inBox, fs: ground.fs, ffi: ground.ffi, heap: ground.heap, args: ground.args }, vis: names, zone: 'bad' }] }
    ];
    var svg = el('svg', { viewBox: '0 0 800 340', 'aria-hidden': 'true' }, document.body);
    var host = el('g', {}, svg);
    el('rect', { x: 28, y: 226, width: 330, height: 92, rx: 14, class: 'zone' }, host);
    el('text', { x: 44, y: 244, class: 'zlab' }, host).textContent = 'release(\u2026)';
    var box = el('rect', { x: 520, y: 200, width: 252, height: 118, rx: 14, class: 'zone' }, host);
    var boxRow = el('text', { x: 536, y: 226, class: 'zrow' }, host);
    var boxLab = el('text', { x: 536, y: 304, class: 'zlab' }, host);
    var worldLab = el('text', { x: 400, y: 100, class: 'zlab', 'text-anchor': 'middle' }, host);
    worldLab.textContent = 'World';
    names.forEach(function (n) {
        el('ellipse', { cx: row[n][0], cy: row[n][1] + 4, rx: 34, ry: 22, class: 'zone slot' }, host);
        el('text', { x: row[n][0], y: row[n][1] + 46, class: 'zlab slot', 'text-anchor': 'middle' }, host).textContent = n;
    });
    var caps = {};
    names.forEach(function (n) {
        var g = el('g', { class: 'cap-rock', style: '--x:' + W[0] + 'px;--y:' + W[1] + 'px;opacity:0' }, host);
        R.rock(g, { seed: 'cap-' + n, x: 0, baseY: 18, rx: n === 'io' ? 30 : 26, ry: 20, n: 10, cracks: 1, lichen: 1 });
        el('text', { x: 0, y: 40, class: 'lab' }, g).textContent = n;
        caps[n] = g;
    });
    var world = el('g', { class: 'cap-rock', style: '--x:' + W[0] + 'px;--y:' + W[1] + 'px' }, host);
    R.rock(world, { seed: 'world', x: 0, baseY: 34, rx: 56, ry: 40, n: 13, cracks: 3, lichen: 4 });
    function render(p) {
        var sc = scenarios[p.scenario] || scenarios[0];
        var st = sc.steps[Math.max(0, Math.min(sc.steps.length - 1, p.step | 0))];
        names.forEach(function (n) {
            var shown = st.vis.indexOf(n) >= 0, at = shown ? (st.at[n] || row[n]) : W;
            var small = shown && (at === ground[n] || at === inBox);
            caps[n].style.setProperty('--x', at[0] + 'px');
            caps[n].style.setProperty('--y', at[1] + 'px');
            caps[n].style.opacity = shown ? 1 : 0;
            caps[n].setAttribute('class', 'cap-rock' + (st.bad && st.bad.indexOf(n) >= 0 ? ' bad' : ''));
            caps[n].style.transform = 'translate(' + at[0] + 'px,' + at[1] + 'px) scale(' + (shown ? (small ? 0.72 : 1) : 0.3) + ')';
        });
        var first = (p.step | 0) === 0;
        worldLab.style.opacity = first ? 1 : 0;
        world.style.opacity = first ? 1 : 0;
        box.setAttribute('class', 'zone' + (st.zone ? ' ' + st.zone : ''));
        boxRow.textContent = sc.greet;
        boxLab.textContent = sc.lab;
    }
    render({ scenario: 0, step: 0 });
    cancho.onprops(render);
})();
