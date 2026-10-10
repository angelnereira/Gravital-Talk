# Outputs — Builds distribuibles de Gravital Talk

Este directorio contiene los binarios y APKs construidos localmente con
`./scripts/build-apk.sh` o por CI. **Los binarios no se versionan en git**:
viven en disco durante el desarrollo y se publican como ficheros descargables
en el release [Builds](https://github.com/angelnereira/Gravital-Talk/releases/tag/builds).

## Descargar para probar en el móvil

El release `builds` mantiene dos ficheros con nombres fijos, así que el enlace
para bajar desde el móvil no cambia nunca:

| Fichero | Para qué |
|---|---|
| `gravital-talk-latest.apk` | El último build. Este es el que hay que instalar. |
| `gravital-talk-previous.apk` | El anterior, para volver atrás si algo va mal. |

```
https://github.com/angelnereira/Gravital-Talk/releases/download/builds/gravital-talk-latest.apk
```

Va sin firmar: Android pide permiso para instalar desde fuentes desconocidas.
Se firma al publicar, no durante el desarrollo.

El workflow (`build-outputs.yml`) actualiza ese release solo en cada build:
antes de subir el APK nuevo, el actual pasa a `previous.apk` y los ficheros
viejos se borran. En el release nunca hay más de dos.

## Estructura

```
outputs/
├── android/
│   ├── debug/          APKs de debug (firmados con key de debug)
│   └── release/        APKs de release (unsigned — firmar antes de instalar)
├── linux/
│   ├── x86_64/         Binario CLI `gs` para Linux 64-bit
│   └── aarch64/        Binario CLI `gs` para Linux ARM64
├── macos/
│   ├── x86_64/         Binario CLI `gs` para Intel Mac
│   └── aarch64/        Binario CLI `gs` para Apple Silicon
└── windows/
    └── x86_64/         Ejecutable CLI `gs.exe` para Windows 64-bit
```

## Nomenclatura de versiones

```
gravital-talk-v<semver>-<short_sha>[-debug|-release-unsigned].apk
gs-v<semver>-<short_sha>-<os>-<arch>[.exe]
```

Ejemplo:
```
android/debug/gravital-talk-v0.1.0-alpha.1-abc1234-debug.apk
linux/x86_64/gs-v0.1.0-alpha.1-abc1234-linux-x86_64
```

## Cómo instalar el APK

```bash
# Instalar en dispositivo conectado por USB (modo debug activado)
adb install -r outputs/android/debug/gravital-talk-<version>-debug.apk

# Lanzar la app directamente
adb install -r outputs/android/debug/gravital-talk-<version>-debug.apk && \
adb shell am start -n com.gravitaltalk/.PairingActivity
```

## Cómo usar el CLI `gs`

```bash
# Linux — dar permisos y ejecutar
chmod +x outputs/linux/x86_64/gs-<version>-linux-x86_64
./outputs/linux/x86_64/gs-<version>-linux-x86_64 ptt --relay relay.host:9000

# Levantár relay local
./outputs/linux/x86_64/gs-<version>-linux-x86_64 relay --bind 0.0.0.0 --udp-port 9000
```

## Archivos LATEST.txt

Cada subdirectorio de plataforma contiene un `LATEST.txt` con el nombre
del último archivo construido, para facilitar scripting:

```bash
LATEST=$(cat outputs/android/debug/LATEST.txt)
adb install -r "outputs/android/debug/$LATEST"
```
