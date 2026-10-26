// The hero scene of the page: the drawing is the page's own scene.js on its rocks.js (the two files before this one, byte for byte). This file only makes the <svg>
// they draw into and moves the layers with the pointer, as the page's script did (the page feeds `pointer` as a position from 0 to 1 over the window).
(function () {
    var svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('class', 'scene');
    svg.setAttribute('viewBox', '0 0 640 600');
    svg.setAttribute('preserveAspectRatio', 'xMidYMax meet');
    document.body.appendChild(svg);
    window.Scene.draw(svg);
    cancho.onprops(function (p) {
        if (p.pointer && typeof p.pointer.x === 'number') {
            svg.style.setProperty('--px', ((p.pointer.x - 0.5) * -2).toFixed(3));
            svg.style.setProperty('--py', ((p.pointer.y - 0.5) * -2).toFixed(3));
        }
    });
})();
