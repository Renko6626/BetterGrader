import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export function usePdf() {
  // PDF 字节 → 每页 JPEG 字节数组。
  // 用 JPEG 而非 PNG：扫描页的 PNG 单页可达数 MB，经 IPC 传 number[] 极慢；
  // JPEG(q≈0.82) 通常小 5–10 倍，判卷清晰度足够，导入从 ~10s/份 降到 1–2s/份。
  async function renderToJpegs(bytes: Uint8Array, scale = 1.6, quality = 0.82): Promise<Uint8Array[]> {
    const pdf = await pdfjs.getDocument({ data: bytes }).promise;
    const out: Uint8Array[] = [];
    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const viewport = page.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      const ctx = canvas.getContext("2d")!;
      // JPEG 无透明通道，先铺白底，避免扫描件透明区域变黑
      ctx.fillStyle = "#fff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      await page.render({ canvasContext: ctx, viewport }).promise;
      const blob: Blob = await new Promise((r) => canvas.toBlob((b) => r(b!), "image/jpeg", quality));
      out.push(new Uint8Array(await blob.arrayBuffer()));
      canvas.width = 0; canvas.height = 0; // 释放
    }
    return out;
  }
  return { renderToJpegs };
}
