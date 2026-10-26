// The island of examples/port: it writes the props it was given, and counts how many times it was given any. It knows nothing of the page: it uses
// `cancho.props` and `cancho.onprops`, the names the frame is given. `step` and `note` come from the state of the page (docs/www-port/gaps.md section 5).
(function () {
    var out = document.createElement('p');
    out.id = 'out';
    document.body.appendChild(out);
    var seen = 0;
    function draw(p) {
        seen++;
        out.setAttribute('data-seen', String(seen));
        out.textContent = 'step ' + (typeof p.step === 'number' ? p.step : 'none') + ', note ' + (typeof p.note === 'string' ? p.note.length : 'none') + ' characters';
        // the first message the page sent (the call above the listener is the empty props before any): what the page said at first
        if (seen === 2) { out.setAttribute('data-first', out.textContent); }
    }
    draw(cancho.props);
    cancho.onprops(draw);
})();
