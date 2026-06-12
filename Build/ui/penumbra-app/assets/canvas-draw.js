// Canvas coordinate system:
//   screen_x = world_x * zoom + cam.x
//   world_x  = (screen_x - cam.x) / zoom
//
// The HTML card container uses `translate(cam.x, cam.y) scale(zoom)` which
// produces the same formula, so canvas edges now stay anchored to cards.

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
  var cam = s.camera;
  var zoom = cam.zoom;

  // Pull themed colors from CSS variables so the canvas matches the active theme.
  var cs = getComputedStyle(document.documentElement);
  var COL_GRID  = (cs.getPropertyValue('--grid-dot') || 'rgba(99,148,220,0.16)').trim();
  var COL_EDGE  = (cs.getPropertyValue('--edge') || 'rgba(99,148,220,0.55)').trim();
  var COL_EDGEH = (cs.getPropertyValue('--edge-strong') || 'rgba(140,194,255,0.9)').trim();
  var COL_PIN   = (cs.getPropertyValue('--pin') || 'rgba(255,185,100,0.9)').trim();
  var COL_ACC   = (cs.getPropertyValue('--accent-bright') || 'rgba(140,194,255,0.95)').trim();
  var COL_TEXT  = (cs.getPropertyValue('--text') || 'rgba(220,232,255,0.85)').trim();

  // World ↔ screen helpers
  function wx(worldX) { return worldX * zoom + cam.x; }
  function wy(worldY) { return worldY * zoom + cam.y; }
  function worldLeft()   { return -cam.x / zoom; }
  function worldTop()    { return -cam.y / zoom; }
  function worldRight()  { return (w - cam.x) / zoom; }
  function worldBottom() { return (h - cam.y) / zoom; }

  ctx.clearRect(0, 0, w, h);

  // Dot grid
  // Adaptive spacing: keep ~28 screen-px between dots regardless of zoom.
  var BASE_SPACING = 28;
  var worldSpacing = BASE_SPACING / zoom;
  // Snap worldSpacing to a nice round number so the grid doesn't jitter.
  var magnitude = Math.pow(10, Math.floor(Math.log10(worldSpacing)));
  var normalized = worldSpacing / magnitude;
  if (normalized < 1.5) worldSpacing = magnitude;
  else if (normalized < 3.5) worldSpacing = 2 * magnitude;
  else if (normalized < 7.5) worldSpacing = 5 * magnitude;
  else worldSpacing = 10 * magnitude;

  var gridFade = Math.min(1, Math.max(0, (zoom - 0.05) / 0.25));
  if (gridFade > 0) {
    ctx.save();
    ctx.globalAlpha = gridFade;
    ctx.fillStyle = COL_GRID;
    var wl = worldLeft(), wt = worldTop(), wr = worldRight(), wb = worldBottom();
    var startX = Math.floor(wl / worldSpacing) * worldSpacing;
    var startY = Math.floor(wt / worldSpacing) * worldSpacing;
    for (var gy = startY; gy <= wb; gy += worldSpacing) {
      for (var gx = startX; gx <= wr; gx += worldSpacing) {
        ctx.fillRect(wx(gx) - 0.5, wy(gy) - 0.5, 1, 1);
      }
    }
    ctx.restore();
  }

  // Build position lookup
  var pos = {};
  var selectedId = s.selected_node;
  var hoveredId  = s.hovered_node;

  for (var i = 0; i < s.nodes.length; i++) {
    var n = s.nodes[i];
    pos[n.id] = { x: n.position.x, y: n.position.y };
  }

  // Edges
  // Card half-extents (world units at zoom=1)
  var CARD_W = 90;  // half of typical width ~180px (min 150, max 210)
  var CARD_H = 42;  // half height: title(18) + preview(16) + tags(16) + padding(20) + border(2) ≈ 72px / 2

  function clampToBox(fx, fy, tx, ty) {
    // Trim the line segment so it starts/ends at the card border rather than
    // the center.  Returns adjusted [ax, ay, bx, by].
    var dx = tx - fx, dy = ty - fy;
    var len = Math.sqrt(dx*dx + dy*dy);
    if (len < 1) return [fx, fy, tx, ty];
    var ux = dx/len, uy = dy/len;

    // Approximate card as rect; trim by whichever boundary is hit first.
    function trimSrc(ox, oy, udx, udy) {
      var t = Infinity;
      if (Math.abs(udx) > 1e-6) {
        t = Math.min(t, CARD_W / Math.abs(udx));
      }
      if (Math.abs(udy) > 1e-6) {
        t = Math.min(t, CARD_H / Math.abs(udy));
      }
      return t;
    }

    var ts = trimSrc(fx, fy,  ux,  uy);
    var te = trimSrc(tx, ty, -ux, -uy);
    ts = Math.min(ts, len * 0.4);
    te = Math.min(te, len * 0.4);

    return [fx + ux*ts, fy + uy*ts, tx - ux*te, ty - uy*te];
  }

  for (var i = 0; i < s.edges.length; i++) {
    var e = s.edges[i];
    var src = pos[e.source];
    var tgt = pos[e.target];
    if (!src || !tgt) continue;

    var explicit = e.opacity > 0.5;
    var isHighlit = (e.source === selectedId || e.target === selectedId ||
                     e.source === hoveredId  || e.target === hoveredId);

    var trimmed = clampToBox(src.x, src.y, tgt.x, tgt.y);
    var ax = wx(trimmed[0]), ay = wy(trimmed[1]);
    var bx = wx(trimmed[2]), by = wy(trimmed[3]);

    // Bezier control point: gentle S-curve.
    var midX = (ax + bx) / 2;
    var midY = (ay + by) / 2;

    ctx.beginPath();
    ctx.moveTo(ax, ay);
    ctx.bezierCurveTo(midX, ay, midX, by, bx, by);

    ctx.save();
    if (explicit) {
      ctx.lineWidth = isHighlit ? 2.0 : 1.5;
      ctx.strokeStyle = isHighlit ? COL_EDGEH : COL_EDGE;
      ctx.setLineDash([]);
    } else {
      ctx.lineWidth = isHighlit ? 1.5 : 1.0;
      ctx.strokeStyle = COL_EDGE;
      ctx.globalAlpha = isHighlit ? 0.7 : 0.38;
      ctx.setLineDash([4, 5]);
    }
    ctx.stroke();
    ctx.restore();
    ctx.setLineDash([]);

    // Arrowhead on explicit links (at the target end).
    if (explicit && zoom > 0.35) {
      var dx = bx - ax, dy = by - ay;
      var len = Math.sqrt(dx*dx + dy*dy);
      if (len > 20) {
        var ux = dx/len, uy = dy/len;
        var arrowLen = Math.min(10, len * 0.15) * Math.min(zoom, 1.5);
        var arrowW = arrowLen * 0.45;
        ctx.beginPath();
        ctx.moveTo(bx, by);
        ctx.lineTo(bx - ux*arrowLen - uy*arrowW, by - uy*arrowLen + ux*arrowW);
        ctx.lineTo(bx - ux*arrowLen + uy*arrowW, by - uy*arrowLen - ux*arrowW);
        ctx.closePath();
        ctx.fillStyle = isHighlit ? COL_EDGEH : COL_EDGE;
        ctx.fill();
      }
    }
  }

  // Selection / hover rings drawn on canvas so feedback shows behind cards.
  function drawRing(worldX, worldY, color, radius, alpha) {
    var sx = wx(worldX), sy = wy(worldY);
    var r = radius * Math.max(zoom, 0.3);
    ctx.save();
    ctx.globalAlpha = alpha;
    ctx.beginPath();
    ctx.arc(sx, sy, r, 0, Math.PI * 2);
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.stroke();
    ctx.globalAlpha = alpha * 0.2;
    ctx.beginPath();
    ctx.arc(sx, sy, r + 4, 0, Math.PI * 2);
    ctx.lineWidth = 8;
    ctx.stroke();
    ctx.restore();
  }

  if (zoom > 0.2) {
    for (var i = 0; i < s.nodes.length; i++) {
      var n = s.nodes[i];
      var p = pos[n.id];
      if (!p) continue;
      if (n.id === selectedId) {
        drawRing(p.x, p.y, COL_ACC, 80, 0.85);
      } else if (n.id === hoveredId) {
        drawRing(p.x, p.y, COL_EDGE, 80, 0.6);
      }
    }
  }

  // LOD dots (zoomed out)
  // Below zoom 0.45 the HTML cards become too small to read; draw dot labels instead.
  if (zoom < 0.45) {
    var dotR = Math.max(3, 6 * zoom / 0.45);
    for (var i = 0; i < s.nodes.length; i++) {
      var n = s.nodes[i];
      var p = pos[n.id];
      if (!p) continue;
      var sx = wx(p.x), sy = wy(p.y);

      // Skip if outside viewport (with margin).
      if (sx < -40 || sx > w + 40 || sy < -40 || sy > h + 40) continue;

      var isSelected = n.id === selectedId;
      var isHovered  = n.id === hoveredId;

      // Outer glow for selected/hovered.
      if (isSelected || isHovered) {
        ctx.save();
        ctx.globalAlpha = isSelected ? 0.28 : 0.18;
        ctx.beginPath();
        ctx.arc(sx, sy, dotR + 5, 0, Math.PI * 2);
        ctx.fillStyle = isSelected ? COL_ACC : COL_EDGE;
        ctx.fill();
        ctx.restore();
      }

      // Dot fill.
      ctx.beginPath();
      ctx.arc(sx, sy, dotR, 0, Math.PI * 2);
      ctx.fillStyle = n.pinned ? COL_PIN : (isSelected ? COL_ACC : COL_EDGE);
      ctx.fill();

      // Label (only when there's enough room).
      if (zoom > 0.25 && n.title) {
        var fontSize = Math.round(9 + zoom * 4);
        ctx.font = '600 ' + fontSize + 'px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif';
        ctx.save();
        ctx.globalAlpha = 0.85;
        ctx.fillStyle = COL_TEXT;
        ctx.textAlign = 'center';
        ctx.fillText(n.title.length > 22 ? n.title.slice(0, 21) + '…' : n.title, sx, sy + dotR + fontSize + 1);
        ctx.restore();
      }
    }
  }
};
