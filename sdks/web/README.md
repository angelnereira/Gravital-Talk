# @gravital/sound-web

SDK de Gravital Sound para navegadores. Combina un módulo WASM (encode/decode
del protocolo) con un transport WebSocket en JavaScript.

> El MVP depende de un relay WebSocket externo (ver
> `examples/relay_server.rs` en la raíz del repo para un relay UDP que puede
> extenderse a WebSocket).

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
