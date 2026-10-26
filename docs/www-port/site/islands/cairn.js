// One cairn of the dividers: three stones, from the page's rocks.js (the file before this one), with the seeds the page's script used ('cairn' + n + i).
(function () {
    var R = window.Rocks, el = R.el;
    function draw(n) {
        var svg = el('svg', { viewBox: '0 0 84 58' }, document.body);
        [[42, 58, 32, 14, 3], [44, 38, 22, 11, 4], [41, 21, 13, 8, 5]].forEach(function (s, i) {
            var g = el('g', { class: 'st' }, svg);
            R.rock(g, { seed: 'cairn' + n + i, x: s[0], baseY: s[1], rx: s[2], ry: s[3], n: 9, cracks: 0, lichen: 0, jitter: 0.12 });
        });
    }
    // the props arrive after this file has run: the stones are drawn once, when the seed is known
    var drawn = false;
    cancho.onprops(function (p) {
        if (drawn || typeof p.seed !== 'number') { return; }
        drawn = true;
        draw(p.seed);
    });
})();
