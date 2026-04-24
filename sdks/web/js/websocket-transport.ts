/**
 * Transport WebSocket para Gravital Sound en el browser.
 * El envío y recepción de bytes crudos se delega a una instancia de
 * WebSocket; el WASM hace el encode/decode del protocolo.
 */

export type TransportEvent =
  | { type: "open" }
  | { type: "message"; data: Uint8Array }
  | { type: "close"; code: number; reason: string }
  | { type: "error"; error: Error };

export type TransportListener = (ev: TransportEvent) => void;

export class WebSocketTransport {
  private ws: WebSocket | null = null;
  private listeners: Set<TransportListener> = new Set();
  private readonly url: string;

  constructor(url: string) {
    this.url = url;
  }

  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(this.url);
      ws.binaryType = "arraybuffer";
      ws.addEventListener("open", () => {
        this.emit({ type: "open" });
        resolve();
      });
      ws.addEventListener("message", (ev: MessageEvent) => {
        const data = ev.data as ArrayBuffer;
        this.emit({ type: "message", data: new Uint8Array(data) });
      });
      ws.addEventListener("close", (ev: CloseEvent) => {
        this.emit({ type: "close", code: ev.code, reason: ev.reason });
      });
      ws.addEventListener("error", (ev: Event) => {
        const err = new Error(`WebSocket error: ${ev.type}`);
        this.emit({ type: "error", error: err });
        reject(err);
      });
      this.ws = ws;
    });
  }

  send(bytes: Uint8Array): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error("transport not open");
    }
    this.ws.send(bytes);
  }

  close(code: number = 1000, reason: string = "normal closure"): void {
    this.ws?.close(code, reason);
  }

  addEventListener(listener: TransportListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emit(ev: TransportEvent): void {
    for (const l of this.listeners) {
      try {
        l(ev);
      } catch (e) {
        console.error("listener error", e);
      }
    }
  }
}
