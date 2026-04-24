/**
 * Gravital Sound — SDK Web.
 *
 * API de alto nivel basada en Promises y EventTarget. Combina el WASM para
 * framing del protocolo con un transporte WebSocket administrado en JS.
 *
 * Ejemplo:
 *
 *   import { GravitalSoundSession } from "@gravital/sound-web";
 *
 *   const session = await GravitalSoundSession.connect({
 *     url: "wss://relay.example.com/ws",
 *     sampleRate: 48000,
 *     channels: 1,
 *   });
 *   session.onAudio((frame) => playPcm(frame));
 *   await session.sendAudio(new Uint8Array(1920));
 */

import init, {
  buildHandshakeConfirm,
  buildHandshakeInit,
  decodePacket,
  encodePacket,
  MsgType,
  protocolVersion,
  version,
} from "../pkg/gravital_sound_web";
import { WebSocketTransport } from "./websocket-transport";

export interface SessionOptions {
  url: string;
  sampleRate?: number;
  channels?: number;
  frameDurationMs?: number;
  maxBitrate?: number;
  capabilityFlags?: number;
}

export interface AudioFrame {
  sequence: number;
  timestamp: number;
  payload: Uint8Array;
}

type AudioListener = (frame: AudioFrame) => void;

export class GravitalSoundSession {
  private readonly transport: WebSocketTransport;
  private sessionId: number = 0;
  private sequence: number = 0;
  private listeners: Set<AudioListener> = new Set();
  private resolveHandshake: ((sid: number) => void) | null = null;
  private rejectHandshake: ((err: Error) => void) | null = null;

  private constructor(transport: WebSocketTransport) {
    this.transport = transport;
  }

  static async connect(opts: SessionOptions): Promise<GravitalSoundSession> {
    await init();
    const transport = new WebSocketTransport(opts.url);
    const session = new GravitalSoundSession(transport);
    transport.addEventListener((ev) => session.onTransportEvent(ev));
    await transport.connect();
    await session.performHandshake(opts);
    return session;
  }

  private async performHandshake(opts: SessionOptions): Promise<void> {
    const nonce = Math.floor(Math.random() * 0xffffffff) >>> 0;
    const init = buildHandshakeInit(
      opts.sampleRate ?? 48000,
      opts.channels ?? 1,
      opts.frameDurationMs ?? 20,
      opts.maxBitrate ?? 64000,
      opts.capabilityFlags ?? 0,
      nonce,
    );
    const handshakePromise = new Promise<number>((resolve, reject) => {
      this.resolveHandshake = resolve;
      this.rejectHandshake = reject;
      setTimeout(() => reject(new Error("handshake timeout")), 10000);
    });
    this.transport.send(init);
    const sid = await handshakePromise;
    this.sessionId = sid;
    const confirm = buildHandshakeConfirm(sid, this.sequence++);
    this.transport.send(confirm);
  }

  private onTransportEvent(ev: {
    type: string;
    data?: Uint8Array;
    code?: number;
    reason?: string;
    error?: Error;
  }): void {
    if (ev.type === "message" && ev.data) {
      try {
        const pkt = decodePacket(ev.data) as AudioFrame & { msg_type: number };
        if (pkt.msg_type === MsgType.HANDSHAKE_ACCEPT) {
          // Session id viene en el payload de ACCEPT; el WASM devuelve
          // el raw payload y JS extrae los últimos 4 bytes (session_id BE).
          const payload = pkt.payload;
          const sid =
            (payload[payload.length - 4] << 24) |
            (payload[payload.length - 3] << 16) |
            (payload[payload.length - 2] << 8) |
            payload[payload.length - 1];
          this.resolveHandshake?.(sid >>> 0);
        } else if (pkt.msg_type === MsgType.AUDIO_FRAME) {
          for (const l of this.listeners) {
            l({
              sequence: pkt.sequence,
              timestamp: pkt.timestamp,
              payload: pkt.payload,
            });
          }
        }
      } catch (e) {
        console.warn("bad packet", e);
      }
    } else if (ev.type === "close") {
      this.rejectHandshake?.(new Error(`closed: ${ev.reason}`));
    } else if (ev.type === "error" && ev.error) {
      this.rejectHandshake?.(ev.error);
    }
  }

  onAudio(listener: AudioListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  async sendAudio(payload: Uint8Array): Promise<void> {
    const pkt = encodePacket(
      MsgType.AUDIO_FRAME,
      this.sessionId,
      this.sequence++,
      performance.now() * 1000,
      payload,
    );
    this.transport.send(pkt);
  }

  close(): void {
    this.transport.close();
  }

  get id(): number {
    return this.sessionId;
  }
}

export { version, protocolVersion };
