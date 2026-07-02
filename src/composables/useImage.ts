import { ref, onUnmounted } from "vue";
import { readImage } from "../api";

// filename → blob URL；自动 revoke 上一张，卸载时清理
export function useImage() {
  const url = ref<string | null>(null);
  let current: string | null = null;
  async function show(filename: string | null) {
    if (current) { URL.revokeObjectURL(current); current = null; }
    url.value = null;
    if (!filename) return;
    const bytes = await readImage(filename);           // number[]
    const blob = new Blob([new Uint8Array(bytes)]);
    current = URL.createObjectURL(blob);
    url.value = current;
  }
  onUnmounted(() => { if (current) URL.revokeObjectURL(current); });
  return { url, show };
}
