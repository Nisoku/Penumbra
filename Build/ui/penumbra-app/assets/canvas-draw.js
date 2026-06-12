window.__penumbra_draw = function () {
    var s = window.__penumbra_state;
    if (!s || !s.camera) return;
    var canvasId = window.__penumbra_canvas_id;
    if (!canvasId) return;
    var c = document.getElementById(canvasId);
    if (!c) return;
    var ctx = c.getContext('2d');
    if (!ctx) return;
    var w = c.width, h = c.height;
    ctx.clearRect(0, 0, w, h);

    var spacing = 28;
    var inv = 1 / s.camera.zoom;
    var left = (-s.camera.x) * inv;
    var top = (-s.camera.y) * inv;
    var right = left + w * inv;
    var bottom = top + h * inv;

    ctx.fillStyle = 'rgba(99,148,220,0.18)';
    for (var gy = Math.floor(top / spacing) * spacing; gy <= bottom; gy += spacing) {
        for (var gx = Math.floor(left / spacing) * spacing; gx <= right; gx += spacing) {
            var sx = (gx + s.camera.x) * s.camera.zoom;
            var sy = (gy + s.camera.y) * s.camera.zoom;
            ctx.fillRect(sx - 0.5, sy - 0.5, 1, 1);
        }
    }

    var pos = {};
    for (var i = 0; i < s.nodes.length; i++) {
        var n = s.nodes[i];
        pos[n.id] = { x: n.position.x, y: n.position.y };
    }

    ctx.lineWidth = 1.5;
    for (var i = 0; i < s.edges.length; i++) {
        var e = s.edges[i];
        var src = pos[e.source];
        var tgt = pos[e.target];
        if (!src || !tgt) continue;
        var ax = (src.x + s.camera.x) * s.camera.zoom;
        var ay = (src.y + s.camera.y) * s.camera.zoom;
        var bx = (tgt.x + s.camera.x) * s.camera.zoom;
        var by = (tgt.y + s.camera.y) * s.camera.zoom;
        var cx = (ax + bx) / 2;
        ctx.beginPath();
        ctx.moveTo(ax, ay);
        ctx.bezierCurveTo(cx, ay, cx, by, bx, by);
        ctx.strokeStyle = e.opacity > 0.5 ? 'rgba(99,148,220,0.7)' : 'rgba(99,148,220,0.35)';
        ctx.stroke();
    }
};
