// Treemap "squarified" (Bruls et al.): bloques lo más cuadrados posible.
// items: [{ value, ... }] ordenados de mayor a menor. Devuelve cada item con x,y,w,h en %.
export function squarify(items, W = 100, H = 100) {
  const total = items.reduce((a, i) => a + i.value, 0);
  if (!total) return [];
  const scale = (W * H) / total;
  const nodes = items.map((i) => ({ ...i, area: i.value * scale }));
  const out = [];
  let x = 0, y = 0, w = W, h = H;

  const worst = (row, side) => {
    const sum = row.reduce((a, n) => a + n.area, 0);
    const max = Math.max(...row.map((n) => n.area));
    const min = Math.min(...row.map((n) => n.area));
    return Math.max((side * side * max) / (sum * sum), (sum * sum) / (side * side * min));
  };

  const place = (row) => {
    const sum = row.reduce((a, n) => a + n.area, 0);
    if (w >= h) {
      const rw = sum / h;
      let cy = y;
      for (const n of row) {
        const nh = n.area / rw;
        out.push({ ...n, x, y: cy, w: rw, h: nh });
        cy += nh;
      }
      x += rw;
      w -= rw;
    } else {
      const rh = sum / w;
      let cx = x;
      for (const n of row) {
        const nw = n.area / rh;
        out.push({ ...n, x: cx, y, w: nw, h: rh });
        cx += nw;
      }
      y += rh;
      h -= rh;
    }
  };

  let row = [];
  for (const n of nodes) {
    const side = Math.min(w, h);
    if (!row.length || worst([...row, n], side) <= worst(row, side)) {
      row.push(n);
    } else {
      place(row);
      row = [n];
    }
  }
  if (row.length) place(row);
  return out;
}
