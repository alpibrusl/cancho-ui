// Three stones drawn into an <svg>. `seed` sets their sizes, the pointer (x and y from 0 to 1, fed by the page at most 20 times a second) makes
// them lean, and a click on one says `Picked` with its number. This file knows nothing of the page: it uses `cancho.props`, `cancho.onprops` and
// `cancho.emit`, the three names the frame is given.
(function () {
    var NS = 'http://www.w3.org/2000/svg';
    var svg = document.createElementNS(NS, 'svg');
    svg.setAttribute('viewBox', '0 0 320 200');
    svg.setAttribute('width', '100%');
    document.body.appendChild(svg);
    var stones = [];
    for (var i = 0; i < 3; i++) {
        (function (n) {
            var g = document.createElementNS(NS, 'ellipse');
            g.setAttribute('class', 'stone');
            g.setAttribute('tabindex', '-1');
            g.addEventListener('click', function () { cancho.emit('Picked', n); });
            svg.appendChild(g);
            stones.push(g);
        })(i + 1);
    }
    function draw(p) {
        var seed = typeof p.seed === 'number' ? p.seed : 0;
        var lean = p.pointer ? (p.pointer.x - 0.5) * 24 : 0;
        for (var i = 0; i < stones.length; i++) {
            var w = 34 + ((seed * 7 + i * 13) % 22);
            stones[i].setAttribute('cx', String(70 + i * 90 + lean * (i + 1) / 3));
            stones[i].setAttribute('cy', String(150 - (i % 2) * 14));
            stones[i].setAttribute('rx', String(w));
            stones[i].setAttribute('ry', String(w * 0.7));
        }
    }
    draw(cancho.props);
    cancho.onprops(draw);
})();
