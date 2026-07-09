import { ref } from "vue";

// 卷面缩放/平移：滚轮缩放(夹 0.3–8)、拖拽平移、reset 复位。
// 判分页与标注页共用——标注认手写姓名/学号同样需要放大。
export function useZoomPan() {
  const zoom = ref(1);
  const panX = ref(0);
  const panY = ref(0);
  let dragging = false, lastX = 0, lastY = 0;

  function onWheel(e: WheelEvent) {
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    zoom.value = Math.min(8, Math.max(0.3, zoom.value * factor));
  }
  function onDown(e: MouseEvent) { dragging = true; lastX = e.clientX; lastY = e.clientY; }
  function onMove(e: MouseEvent) {
    if (!dragging) return;
    panX.value += e.clientX - lastX; panY.value += e.clientY - lastY;
    lastX = e.clientX; lastY = e.clientY;
  }
  function onUp() { dragging = false; }
  function reset() { zoom.value = 1; panX.value = 0; panY.value = 0; }

  return { zoom, panX, panY, onWheel, onDown, onMove, onUp, reset };
}
