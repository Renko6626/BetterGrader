import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export function usePdf() {
  // PDF 字节 → 每页 PNG 字节数组
  async function renderToPngs(bytes: Uint8Array, scale = 1.6): Promise<Uint8Array[]> {
    const pdf = await pdfjs.getDocument({ data: bytes }).promise;
    const out: Uint8Array[] = [];
    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const viewport = page.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      const ctx = canvas.getContext("2d")!;
      await page.render({ canvasContext: ctx, viewport }).promise;
      const blob: Blob = await new Promise((r) => canvas.toBlob((b) => r(b!), "image/png"));
      out.push(new Uint8Array(await blob.arrayBuffer()));
      canvas.width = 0; canvas.height = 0; // 释放
    }
    return out;
  }
  return { renderToPngs };
}
