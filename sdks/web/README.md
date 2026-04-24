# @gravital/sound-web

SDK de Gravital Sound para navegadores. Combina un módulo WASM (encode/decode
del protocolo) con un transport WebSocket en JavaScript.

> **Requiere un relay WebSocket externo.** El MVP sólo entrega relay UDP. El
> relay WebSocket (`tokio-tungstenite` + proxy a UDP) está planeado para
> Track C (producción y seguridad). Hasta entonces, el demo
> `examples/browser-demo/` marca la funcionalidad como WIP.

## Build

```bash
cd sdks/web
npm install
npm run build   # wasm-pack build --target web --out-dir pkg --release
```

## Uso

```ts
import { GravitalSoundSession } from "@gravital/sound-web";

const session = await GravitalSoundSession.connect({
  url: "wss://relay.example.com/ws",
  sampleRate: 48000,
  channels: 1,
});

session.onAudio((frame) => {
  // frame.payload es un Uint8Array con PCM16 little-endian.
});

await session.sendAudio(new Uint8Array(1920));
```

## Demo en navegador

`examples/browser-demo/index.html` — conéctalo a un relay WebSocket y envía
una ráfaga de 1 segundo de senoidal 440 Hz.
