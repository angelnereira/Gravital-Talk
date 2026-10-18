# Gravital Experience UI — Regla de Diseño de Interfaces Flutter

**Versión:** 1.0.0 · **Fecha:** 18 oct 2026 · **Autor:** @Angel Nereira
**Estado:** Regla estricta y obligatoria para toda la app Flutter (`app/`)

---

## 1. Propósito, alcance y principios

Toda la app Flutter de Gravital-Talk —actual y futura— usará un único sistema de diseño basado en el lenguaje visual **Gravital Experience UI (GEUI)**: una interpretación propia de Material 3 Expressive (M3E) adaptada a un protocolo de comunicación de audio Push-To-Talk en tiempo real. Este documento define sus tokens, componentes, patrones de UX y reglas de revisión. Es **de obligado cumplimiento**; ninguna pantalla, diálogo, notificación in-app ni flujo puede apartarse de lo aquí definido.

**Qué se replica:** el lenguaje de diseño abierto de Material (color tonal HCT, escala de formas, tipografía, movimiento por resortes, contención tonal, navegación adaptativa). Es una especificación pública y los componentes de Flutter que la implementan son de código abierto.

**Qué no se replica:** activos de marca de Google (logo, fuentes cuya licencia no lo permita), ni la UI del sistema operativo (barra de estado, ajustes rápidos, launcher). La app no controla la UI del sistema; solo se integra con ella (edge-to-edge, color dinámico en Android, retroceso predictivo).

### Principios rectores

1. **Una sola fuente de verdad.** Ningún widget usa colores, radios, espacios o duraciones literales; todo sale de tokens del tema compartido (`app/lib/ui/`).
2. **Material primero, extensión después.** Se usan los componentes Material 3 de Flutter y se extienden con `ThemeExtension`; solo se construye a mano lo que M3E define y Flutter aún no trae.
3. **Color con intención.** Color dinámico (del fondo de pantalla) en Android 12+; el esquema nunca toca los colores semánticos de estado de audio, conexión o error.
4. **Jerarquía por tono, no por sombra.** Las superficies se distinguen por nivel tonal (`surfaceContainer*`); la sombra se reserva para elementos que flotan sobre contenido en movimiento.
5. **Movimiento físico y con propósito.** Transiciones con resortes que explican el cambio de estado; nunca decoración. Se respeta «reducir movimiento» del sistema.
6. **Adaptativo desde el día uno.** El mismo código se acomoda a teléfono, plegable, tablet, escritorio y web por clases de tamaño de ventana, no por tipo de dispositivo.
7. **Accesible por defecto.** Contraste WCAG AA, objetivos táctiles de 48 dp, texto escalable al 200 % sin cortes, lectores de pantalla.
8. **Identidad igual, convenciones locales.** Color, forma, tipografía y componentes son idénticos en todas las plataformas; gestos, retroceso y diálogos críticos respetan la plataforma donde corre la app.

---

## 2. Estado del ecosistema (verificado al 12 oct 2026)

| Pieza | Estado a oct 2026 | Decisión para el sistema |
| --- | --- | --- |
| [M3 Expressive](https://blog.google/intl/en-in/products/hardware/material-3-expressive-updates-in-the-latest-pixel-drop/) | Llegó a Android 16+ con el Pixel Drop del 4 sep 2025 | Es el lenguaje que se replica |
| [Flutter estable](https://flutter.dev/blog/whats-new-in-flutter-3-47) | 3.47 (12 ago 2026), Dart 3.13; mínimos iOS 15, Android minSdk 24, target/compile SDK 36 | Versión mínima: Flutter 3.47 |
| [material\_ui](https://pub.dev/packages/material_ui/changelog) | 1.6.0 publicado el 6 oct 2026. La copia dentro del SDK se depreca en noviembre 2026 | Importar `package:material_ui/material_ui.dart` en todo el código |
| M3E oficial en `material_ui` | `StyleVariant` (M3 / M3 Expressive) desde 1.2.0; `IconButton` expresivo y niveles de contraste estándar/medio/alto desde 1.5.0 | Activar lo oficial a medida que llegue |
| [dynamic\_color](https://pub.dev/packages/dynamic_color/changelog) | 2.1.0 (20 ago 2026); desde 2.0 depende de `material_ui` | Usar la serie 2.x |
| [material\_3\_expressive](https://pub.dev/packages/material_3_expressive) | 1.1.6 (8 oct 2026), MIT, 44 widgets. Requiere Flutter ≥ 3.47 y `material_ui` ^1.6.0 | Solo para lo que falta, siempre envuelto en adaptadores |
| [Google Sans Flex](https://www.androidauthority.com/google-sans-flex-free-3617034/) | Abierta en Google Fonts el 18 nov 2025 bajo SIL OFL 1.1; cinco ejes en el fichero que ofrece (peso, ancho, tamaño óptico, inclinación, redondez). El eje de grado no viene en ese fichero y no se usa. | Fuente oficial del sistema |

**Regla de actualización:** cada trimestre, al salir una versión estable de Flutter, se revisa el changelog de `material_ui`. Cada componente M3E que pase a oficial reemplaza su adaptador comunitario sin cambiar la API pública del sistema.

---

## 3. Arquitectura del sistema de diseño

El sistema vive dentro de la app como una capa UI en `app/lib/ui/`. Las pantallas solo consumen esa capa; nunca definen colores, formas ni movimiento propios.

```text
app/lib/ui/
├── app.dart              # prepareApp() y GravitalThemes (los 4 temas, una vez)
├── tokens.dart           # NSpace, NShape, NMotion, NCurve, NDuration, WindowClass, NHitSize
├── semantic_colors.dart  # SemanticColors: los siete estados del dominio
├── theme.dart            # NTheme: ColorScheme tonal + ThemeData
├── patterns.dart         # UILoading, EmptyState, ErrorBanner, WarningBanner, ConfirmDialog
└── components/           # PttButton, StatusBadge, SectionCard, MetricTile, LevelMeter,
                           # EventLogView, ParticipantsCard, QrPanel, ReconnectingBanner…
```

**Regla de importación:** las pantallas importan `package:flutter/material.dart` y `app/lib/ui/`. Nada fuera de `components/` importa un paquete de terceros de UI.

### 3.1 Decisiones tomadas al implementar

Tres puntos donde la implementación se apartó del plan original, con el motivo:

1. **Sin color dinámico.** La sección 4.2 lo reservaba a apps personales. Gravital
   Talk ya tenía una paleta medida (teal de marca + ámbar de transmisión) y los
   contrastes del estado «transmitiendo» y del modo demo dependían de ella. Hace
   falta `dynamic_color` y con él volver a medir esos estados en cada Android
   distinto. La semilla de marca es fija en todas las plataformas.
2. **Sin adaptadores de `material_3_expressive`.** Flutter 3.47 cubre de
   sobra lo que la app usa; el paquete se añade sólo cuando falte un
   componente concreto, siempre envuelto en `components/`.
3. **Sin Google Sans Flex empaquetada.** Requiere incrustar un TTF variable en
   el APK. La fuente es la del sistema y los pesos salen de la escala M3, que es
   donde el cambio se nota. Revisado cuando el peso tipográfico sea el problema
   que se vea.

---

## 4. Color

La app declara un solo color semilla de marca Gravital; de él se genera todo el esquema claro, oscuro y de alto contraste en el espacio HCT. El color dinámico del fondo de pantalla solo se activa en Android 12+, y nunca toca los colores semánticos de audio, conexión, estado o error.

### 4.1 Modelo

- El esquema sale de seis paletas tonales (primaria, secundaria, terciaria, neutral, neutral variante, error), cada una con tonos de 0 (negro) a 100 (blanco).
- Los widgets nunca usan un tono: usan **roles** (`primary`, `onSurface`, `surfaceContainerHigh`…). El rol garantiza el contraste en claro, oscuro y alto contraste.
- Generación: `ColorScheme.fromSeed(seedColor:, brightness:, dynamicSchemeVariant:, contrastLevel:)`. Variante por defecto `tonalSpot`.

### 4.2 Política de fuente de color

| Contexto | Fuente del color | Variante | Motivo |
| --- | --- | --- | --- |
| App Gravital Talk (global) | Dinámico en Android 12+; semilla de marca como respaldo | `tonalSpot` | App personal de uso diario; debe sentirse parte del teléfono |
| iOS y escritorio/web | Siempre semilla de marca | `tonalSpot` | No existe color de fondo de pantalla |

En iOS y web no existe color de fondo de pantalla: el sistema usa siempre la semilla de marca.

### 4.3 Uso de roles

| Rol | Uso obligatorio |
| --- | --- |
| `primary` / `onPrimary` | Una sola acción principal por pantalla (botón filled, FAB) |
| `primaryContainer` / `onPrimaryContainer` | Estado seleccionado, FAB, chips activos, indicador de navegación, peer activo |
| `secondaryContainer` / `onSecondaryContainer` | Botones tonales, filtros, acciones secundarias |
| `tertiary` / `tertiaryContainer` | Acento de contraste: insignias no críticas, destacar un dato |
| `error` / `errorContainer` | Solo errores y acciones destructivas |
| `surface` → `surfaceContainerLowest…Highest` | Fondos y jerarquía de superficies (sección 6) |
| `onSurface` / `onSurfaceVariant` | Texto e iconos principales / secundarios |
| `outline` / `outlineVariant` | Bordes de campos y botones outlined / separadores y bordes decorativos |
| `inverseSurface` / `onInverseSurface` | Snackbars y tooltips |
| `primaryFixed`, `secondaryFixed`… | Elementos que deben conservar el mismo tono en claro y oscuro (gráficos, ilustraciones) |

### 4.4 Colores semánticos extendidos

Material no define los estados de una llamada PTT ni la calidad de audio. El sistema los agrega como `ThemeExtension` (`SemanticColors`), cada uno con cuatro roles: color, sobre-color, contenedor y sobre-contenedor.

- **Roles:** `success` (conectado/activo), `warning` (latencia alta), `info` (neutral), `pttActive` (hablante actual), `pttFloor` (floor otorgado), `calling` (llamando), `connectionPoor` (conexión degradada).
- **Generación:** cada uno parte de un tono fijo y se armoniza con `color.harmonizeWith(scheme.primary)` del paquete `dynamic_color`. Se generan con la misma escala tonal para que el contraste sea correcto en claro y oscuro.
- **Regla de estado:** el color nunca es el único indicador. Un peer hablando lleva waveform + nombre destacado; un floor otorgado lleva icono + etiqueta; una conexión degradada lleva texto descriptivo.
- **Gráficos:** paleta categórica de hasta 8 colores derivada de los roles `*Fixed` y de tonos armonizados; nunca colores literales en código de gráficos.

### 4.5 Contraste y accesibilidad de color

- Texto normal ≥ 4.5:1, texto grande y componentes de interfaz ≥ 3:1 (WCAG 2.2 AA).
- Tres niveles: `contrastLevel` 0 (estándar), 0.5 (medio) y 1.0 (alto). El usuario puede elegirlo en Ajustes de la app.
- Si `MediaQuery.highContrastOf(context)` es verdadero, se usa el nivel 1.0 automáticamente.
- El modo oscuro se genera desde la misma semilla; nunca se invierten colores a mano.

---

## 5. Tipografía

Toda la app usa **Google Sans Flex**, empaquetada como fuente variable dentro de `app/assets/fonts/`, con la escala tipográfica de Material 3 (15 estilos) más sus 15 variantes enfatizadas.

### 5.1 Fuente y carga

- **Archivo:** el TTF variable se incluye en `app/assets/fonts/` y se declara en `pubspec.yaml`. No se descarga en tiempo de ejecución.
- **Licencia:** el texto OFL se registra en `LicenseRegistry`.
- **Ejes:** `wght` 1–1000, `wdth` 25–151, `opsz` 1–144, `slnt` −10–0, `ROND` 0–100. El fichero que se empaqueta no lleva `GRAD`: el que ofrece Google Fonts (noviembre 2025) tiene cinco ejes, no seis. Se controlan con `TextStyle.fontVariations`. Regla: `ROND` 0 en datos y formularios; los textos corrientes usan 8.
- **Respaldo:** `fontFamilyFallback` con Roboto Flex y la fuente del sistema para escrituras que Google Sans Flex no cubra.

### 5.2 Escala base (valores de los tokens oficiales de Material 3)

| Estilo | Tamaño / interlineado (sp) | Peso base | Peso enfatizado | Uso típico en Gravital Talk |
| --- | --- | --- | --- | --- |
| displayLarge | 57 / 64 | 400 | 500 | Número de sala, temporizador de llamada |
| displayMedium | 45 / 52 | 400 | 500 | Hero de pantalla principal |
| displaySmall | 36 / 44 | 400 | 500 | Totales destacados |
| headlineLarge | 32 / 40 | 400 | 500 | Título de pantalla grande |
| headlineMedium | 28 / 36 | 400 | 500 | Título de sección en tablet |
| headlineSmall | 24 / 32 | 400 | 500 | Título de diálogo |
| titleLarge | 22 / 28 | 400 | 500 | Barra superior |
| titleMedium | 16 / 24 | 500 | 700 | Título de tarjeta, encabezado de lista |
| titleSmall | 14 / 20 | 500 | 700 | Subtítulos densos |
| bodyLarge | 16 / 24 | 400 | 500 | Texto principal |
| bodyMedium | 14 / 20 | 400 | 500 | Texto secundario |
| bodySmall | 12 / 16 | 400 | 500 | Notas al pie |
| labelLarge | 14 / 20 | 500 | 700 | Botones |
| labelMedium | 12 / 16 | 500 | 700 | Chips, pestañas |
| labelSmall | 11 / 16 | 500 | 700 | Insignias, metadatos |

Las variantes enmatizadas no existen en `TextTheme`, así que el sistema las expone como `ThemeExtension` (`EmphasizedTextTheme`). Se usan para el elemento seleccionado, la cifra que importa o la acción principal, nunca para párrafos completos.

### 5.3 Reglas

1. **Cifras tabulares** en duraciones, contadores y RTT: `fontFeatures: [FontFeature.tabularFigures()]`.
2. **Formato de fechas** siempre con `intl` y el `Locale` activo.
3. **Escalado de texto:** las pantallas deben funcionar con el texto al 200 %. Sin alturas fijas en contenedores de texto.
4. **Ancho de lectura:** los bloques de texto largos se limitan a unos 70 caracteres por línea (contenedor de máximo 720 dp en pantallas anchas).
5. **Máximo tres estilos por pantalla** además de los de los componentes.

---

## 6. Forma y superficies

Las esquinas salen de una escala de diez tokens y las superficies se distinguen por tono, no por sombra.

### 6.1 Escala de esquinas (tokens oficiales M3 Expressive)

| Token | Radio (dp) | Componentes |
| --- | --- | --- |
| none | 0 | Mapas a pantalla completa |
| extraSmall | 4 | Campos de texto, tooltips |
| small | 8 | Chips, insignias de texto |
| medium | 12 | Tarjetas, miniaturas, tiles de sala |
| large | 16 | FAB estándar, hojas laterales, contenedores de lista |
| largeIncreased | 20 | Tarjetas destacadas, FAB mediano |
| extraLarge | 28 | Diálogos, hojas inferiores (solo arriba), overlay de llamada |
| extraLargeIncreased | 32 | Paneles hero, toolbar flotante si no es píldora |
| extraExtraLarge | 48 | Contenedores de ilustración |
| full | 50 % | Botones, barra de búsqueda, indicador de navegación, avatares, botón PTT |

### 6.2 Jerarquía tonal

| Superficie | Rol de color | Dónde |
| --- | --- | --- |
| Fondo | `surface` | Lienzo de cada pantalla |
| Tarjeta elevada, hoja inferior | `surfaceContainerLow` | Contenido agrupado sobre el fondo |
| Barra de navegación, rail, menús | `surfaceContainer` | Navegación persistente |
| Diálogo, barra de búsqueda, toolbar flotante | `surfaceContainerHigh` | Lo que está «más cerca» del usuario |
| Campo de texto filled, tarjeta filled | `surfaceContainerHighest` | Áreas de entrada y énfasis máximo |
| Áreas de retroceso, fondos de lista en tablet | `surfaceContainerLowest` | Lo que está «más lejos» |

### 6.3 Sombra y desenfoque

- **Elevación por defecto: 0.** Solo llevan sombra el FAB, la toolbar flotante, los menús desplegables y los elementos que se arrastran.
- **Desenfoque (opcional).** Se permite únicamente en la toolbar flotante y en el fondo de hojas modales, con `BackdropFilter` y una superficie al 80 % de opacidad. Siempre dentro de un `ClipRRect` del tamaño exacto del elemento.
- **Desactivar** el desenfoque en dispositivos de gama baja, en web y cuando el usuario active alto contraste.

---

## 7. Movimiento y háptica

Todo cambio de posición, tamaño o forma se anima con resorte; todo cambio de color u opacidad usa un resorte de «efectos» sin rebote. Hay dos esquemas: **Standard** para interacciones repetitivas y **Expressive** para momentos protagonistas.

### 7.1 Tokens de resorte (valores de `ExpressiveMotionTokens` y `StandardMotionTokens`)

| Esquema | Token | Amortiguamiento (ratio) | Rigidez | Uso |
| --- | --- | --- | --- | --- |
| Expressive | fastSpatial | 0.6 | 800 | Presión de botón PTT, cambio de forma de un control |
| Expressive | defaultSpatial | 0.8 | 380 | Expandir tarjeta, abrir menú FAB, hoja inferior |
| Expressive | slowSpatial | 0.8 | 200 | Transiciones de pantalla completa, confirmación de llamada |
| Standard | fastSpatial | 0.9 | 1400 | Chips, switches, tabs |
| Standard | defaultSpatial | 0.9 | 700 | Listas, navegación, paneles |
| Standard | slowSpatial | 0.9 | 300 | Cambios grandes de layout |
| Ambos | fastEffects | 1.0 | 3800 | Estados de presión y foco |
| Ambos | defaultEffects | 1.0 | 1600 | Fundidos, cambio de color |
| Ambos | slowEffects | 1.0 | 800 | Aparición de contenido cargado |

En Flutter cada token se construye con `SpringDescription.withDampingRatio(mass: 1, stiffness: k, ratio: ζ)`. Un ratio menor que 1 rebota; 1.0 no rebota.

### 7.2 Qué esquema usa cada parte

- **Expressive** en momentos únicos: conexión establecida, inicio de llamada, respuesta a llamada, onboarding, transición a pantalla de llamada, toggle PTT.
- **Standard** en interacciones repetitivas: navegación entre salas, scroll de historial, ajustes.

### 7.3 Animaciones con duración

Para transiciones que no son físicas (rutas, fundidos cruzados) se usan los tokens de duración y curva de M3:

- Duraciones: short 50–200 ms, medium 250–400 ms, long 450–600 ms, extra long 700–1000 ms.
- Curvas: emphasized `Cubic(0.2, 0, 0, 1)`, emphasized decelerate `Cubic(0.05, 0.7, 0.1, 1)`, emphasized accelerate `Cubic(0.3, 0, 0.8, 0.15)`.
- Transición de ruta: en Android `PredictiveBackPageTransitionsBuilder`; en iOS `CupertinoPageTransitionsBuilder`.

### 7.4 Movimiento reducido

Si `MediaQuery.disableAnimationsOf(context)` es verdadero, todos los resortes espaciales se reemplazan por un fundido de efectos o un cambio instantáneo. En web, Flutter 3.44+ respeta `prefers-reduced-motion`.

### 7.5 Háptica

| Evento | API | Ejemplo |
| --- | --- | --- |
| Selección discreta | `HapticFeedback.selectionClick()` | Switch, segmento, filtro de contacto |
| Acción confirmada | `HapticFeedback.lightImpact()` | Pulsar FAB, agregar contacto |
| Resultado importante | `HapticFeedback.mediumImpact()` | Llamada conectada, código QR válido, floor otorgado |
| Rechazo o error | `HapticFeedback.heavyImpact()` | Llamada rechazada, conexión fallida |

Una vibración por gesto como máximo, nunca durante el desplazamiento, y sincronizada con el inicio del resorte.

---

## 8. Espaciado, layout y adaptabilidad

### 8.1 Espaciado

- Retícula base de **4 dp**. Tokens: 4, 8, 12, 16, 24, 32, 48, 64.
- Márgenes laterales: 16 dp en compact, 24 dp desde medium.
- Padding interno de tarjetas: 16 dp; de diálogos y hojas: 24 dp.
- Objetivo táctil mínimo de **48 × 48 dp**; el botón PTT y controles de llamada: 56 dp.

### 8.2 Clases de tamaño de ventana

| Clase | Ancho (dp) | Navegación | Paneles |
| --- | --- | --- | --- |
| Compact | < 600 | Barra de navegación inferior (3–5 destinos) | 1 |
| Medium | 600–839 | Navigation rail | 1 (2 si el contenido lo permite) |
| Expanded | 840–1199 | Rail, expandible | 2 (lista + detalle) |
| Large | 1200–1599 | Rail expandido o drawer permanente | 2–3 |
| Extra-large | ≥ 1600 | Drawer permanente | 3 |

### 8.3 Layouts canónicos

| Layout | Cuándo | Ejemplos |
| --- | --- | --- |
| Lista + detalle | Colección donde se abre un elemento | Historial de salas → sala |
| Panel de apoyo | Tarea principal con contexto al lado | Sala activa + lista de peers |
| Feed | Contenido navegable en tarjetas | Descubrir salas públicas |

### 8.4 Borde a borde y áreas seguras

- Las apps con `targetSdk` 36 ya no pueden desactivar el modo borde a borde en Android 16+.
- Los **fondos** llegan hasta el borde; el **contenido interactivo** respeta las áreas seguras: `MediaQuery.paddingOf` para barras, `viewInsetsOf` para el teclado.
- No envolver la pantalla entera en `SafeArea`: se aplica solo al contenido.
- Desde Flutter 3.44, usar `MediaQueryData.displayCornerRadii` para que nada se recorte en esquinas muy redondeadas.

### 8.5 Entrada no táctil

En escritorio y web: estados de hover, anillo de foco visible, atajos de teclado (por ejemplo, Espacio para PTT, Ctrl+K buscar) y menú contextual con clic derecho.

---

## 9. Iconografía y componentes

### 9.1 Iconos

- **Familia única:** Material Symbols **Rounded**, vía el paquete `material_symbols_icons`.
- **Ejes:** `fill` 0 por defecto y 1 en el estado seleccionado; `weight` 400; `opticalSize` igual al tamaño del icono.
- **Tamaños:** 20, 24 (por defecto), 40 y 48 dp.
- Un icono sin texto siempre lleva `semanticLabel` o `tooltip`.

### 9.2 Catálogo

| Grupo | Componente | Fuente | Regla clave |
| --- | --- | --- | --- |
| Acciones | Botón filled / tonal / outlined / text | Oficial | Un solo filled por pantalla |
| Acciones | Icon button | Oficial (M3E) | Toggle cambia de redondo a cuadrado |
| Acciones | Button group | Adaptador | 2–5 opciones (método de respuesta, filtro) |
| Acciones | FAB y extended FAB | Oficial | Uno por pantalla; se oculta al desplazar |
| Comunicación | Snackbar, tooltip, badge | Oficial | Snackbar para deshacer; nunca para errores críticos |
| Comunicación | Progress lineal y circular | Oficial | Solo en procesos con progreso medible |
| Contención | Card, lista, divider | Oficial | Listas agrupadas en contenedores redondeados |
| Contención | Diálogo, bottom sheet | Oficial | Diálogo solo para confirmar decisiones |
| Navegación | Navigation bar, rail, drawer, tabs | Oficial | Según la clase de tamaño |
| Navegación | App bar (small, medium/large) | Oficial | Título grande que colapsa al desplazar |
| Selección | Checkbox, radio, switch, chips, slider | Oficial | Estado seleccionado siempre visible |
| Entrada | Text field (filled por defecto) | Oficial | Etiqueta siempre visible; error con texto |

### 9.3 Componentes de dominio (Gravital Talk)

Se construyen una vez sobre los tokens y se reutilizan en toda la app:

- **Llamada y PTT:** `PttButton` (botón principal con morph y resorte), `CallControls` (toggle redondo), `InCallOverlay` (pantalla completa de llamada activa), `AudioWaveform` (visualizador de actividad de audio).
- **Salas y peers:** `RoomTile` (sala con estado), `PeerAvatar` (con indicador de speaking/listening), `RoomList`, `PeerList`, `StatusChip` (conectado, conectando, error, muteado).
- **Contactos:** `ContactTile`, `ContactAvatar`, `CallButton`.
- **Estados de pantalla:** `EmptyState`, `ErrorState`, `OfflineBanner`, `SkeletonList`, `ConnectionLostBanner`.
- **Descubrimiento:** `QrScannerView`, `QrDisplayView`, `PairingCode`.

---

## 10. Navegación y patrones de UX

### 10.1 Modelo de navegación

| Nivel | Patrón | Ejemplo |
| --- | --- | --- |
| Destinos principales | Navigation bar / rail / drawer | Inicio, Contactos, Historial, Ajustes |
| Jerárquico | Push de pantalla con app bar y retroceso | Lista de salas → sala |
| Tarea crítica | Flujo modal a pantalla completa | Iniciar llamada, escribir código QR, onboarding |
| Contextual | Hoja inferior o side sheet | Filtros, detalle rápido, opciones de sala |

- **Router:** `go_router` con `StatefulShellRoute` para conservar el estado de cada destino.
- **Toolbar flotante vs. barra de navegación:** la barra lleva a destinos; la toolbar ejecuta acciones de la pantalla actual.

### 10.2 Retroceso predictivo

- En Android 16+ con `targetSdk` 36, las animaciones de retroceso predictivo están activas por defecto.
- Se usa `PopScope(canPop: …, onPopInvokedWithResult: …)`. `canPop` se calcula antes del gesto: solo es `false` cuando hay cambios sin guardar o una llamada en curso.
- **Nunca** interceptar el retroceso de forma permanente.

### 10.3 Formularios

1. Una columna; etiqueta siempre visible; campos agrupados por tema.
2. Teclado correcto por campo (`TextInputType.number` para códigos, `phone` para números de contacto).
3. Validación al salir del campo. El mensaje dice cómo corregir.
4. Acción principal fija abajo y por encima del teclado; deshabilitada solo si falta un dato obligatorio.

### 10.4 Estados de pantalla

| Estado | Regla |
| --- | --- |
| Carga < 300 ms | No mostrar nada (evita parpadeo) |
| Carga 300 ms – 1 s | `SkeletonList` con la forma del contenido final |
| Carga 1 – 5 s | Esqueleto + indicador de carga |
| Carga > 5 s | Progreso determinado con texto |
| Vacío | Ilustración pequeña + título + una acción |
| Error | Qué pasó + cómo resolverlo + botón Reintentar |
| Sin conexión | `OfflineBanner` persistente; acciones se encolan |

### 10.5 Confirmaciones y acciones destructivas

- **Reversible** (salir de sala, cancelar): se ejecuta de inmediato con snackbar «Deshacer».
- **Irreversible** (eliminar contacto, bloquear): diálogo con verbo explícito en el botón, nunca «Aceptar».
- **Llamada** (iniciar, terminar): la terminación usa botón grande y claro con etiqueta explícita.

### 10.6 Voz y microcopy

- Español neutro de Panamá; verbos en infinitivo en botones («Llamar», «Guardar», «Compartir»).
- Trato de **tú** en toda la app.
- Mensajes cortos: títulos de hasta 6 palabras, descripciones de hasta 2 líneas.

---

## 11. Estrategia multiplataforma

| Plataforma | Color | Navegación y gestos | Notas |
| --- | --- | --- | --- |
| Android | Dinámico donde aplique | Retroceso predictivo con `PredictiveBackPageTransitionsBuilder` | Referencia principal |
| iOS / iPadOS | Siempre semilla de marca | Deslizar desde el borde con `CupertinoPageTransitionsBuilder` | Mínimo iOS 15 |
| Web | Semilla de marca | Historial del navegador con `go_router`; atajos de teclado | Sin desenfoque |
| Escritorio | Semilla de marca | Teclado, hover, clic derecho | Impeller por defecto desde Flutter 3.47 |

### 11.1 Reglas de adaptación

1. Se adapta el **comportamiento**, no la apariencia: `PageTransitionsTheme` por plataforma, `ScrollPhysics` por plataforma.
2. Los diálogos de permisos los muestra el sistema operativo; la app solo explica antes por qué los pide.
3. La decisión se toma con `Theme.of(context).platform`, nunca con `Platform.isIOS`.

---

## 12. Accesibilidad, internacionalización y rendimiento

### 12.1 Accesibilidad

- **Lectores de pantalla:** todo icono interactivo con etiqueta; filas de lista con `MergeSemantics`; cambios importante (llamada conectada, nuevo peer) se anuncian.
- **Escala de texto:** probado al 200 %; nada se corta ni se superpone.
- **Contraste:** tres niveles. Desde Flutter 3.47 se detectan en Android el alto contraste y la inversión de colores del sistema.
- **Movimiento reducido:** sección 7.4.
- **Teclado y foco:** orden lógico, anillo de foco visible, todos los flujos completables sin táctil.
- **Color:** nunca es el único portador de significado (signo, icono o texto siempre acompañan).
- **Pruebas:** en cada pantalla, `meetsGuideline(androidTapTargetGuideline)`, `iOSTapTargetGuideline`, `labeledTapTargetGuideline` y `textContrastGuideline` en `flutter_test`.

### 12.2 Internacionalización

- Textos en archivos ARB con `gen-l10n`; `es` por defecto, `en` como segundo idioma.
- Con `material_ui`, las traducciones de Material vienen en el paquete: `localizationsDelegates: GlobalMaterialLocalizations.delegates`.
- Paddings y alineaciones con versiones direccionales (`EdgeInsetsDirectional`, `AlignmentDirectional`).
- Fechas y números con `intl` y el locale activo.

### 12.3 Rendimiento

- **Motor:** Impeller en Android, iOS y escritorio.
- **Aislar animaciones continuas** (waveform, indicador de PTT) en `RepaintBoundary`.
- **Desenfoque:** máximo un `BackdropFilter` visible por pantalla.
- **Listas:** siempre `ListView.builder` o slivers; constructores `const` en widgets estáticos.
- **Presupuesto de cuadro:** 16,6 ms a 60 Hz y 8,3 ms a 120 Hz; se mide en modo profile con DevTools.

---

## 13. Gobernanza y cumplimiento

### 13.1 Reglas estrictas (obligatorias)

1. **Prohibido** usar colores, radios, espaciaciones o duraciones literales en widgets. Todo sale de `NSpace`, `NShape`, `NMotion` o roles de color.
2. **Prohibido** importar `material_3_expressive` fuera de `app/lib/ui/adapters/`.
3. **Prohibido** usar `Theme.of(context).colorScheme.primary` directamente en un widget sin pasar por un componente del sistema.
4. **Obligatorio** usar los componentes de dominio (PttButton, CallControls, StatusChip, etc.) en lugar de construir UI equivalente a mano.
5. **Obligatorio** respetar `WindowClass.of(context)` para decisiones de layout; nunca hardcoded breakpoints.
6. **Obligatorio** implementar los cuatro estados de pantalla (carga, vacío, error, sin conexión) en toda pantalla con datos remotos.
7. **Obligatorio** usar `semanticLabel` en todo icono interactivo sin texto.
8. **Obligatorio** respetar `MediaQuery.disableAnimationsOf(context)` en toda animación.

### 13.2 Checklist de revisión de cada pantalla

- [ ] Sin colores, radios, espacios ni duraciones literales; todo sale de tokens o roles
- [ ] Un solo botón filled (acción principal) por pantalla
- [ ] Funciona en compact y expanded, en vertical y horizontal
- [ ] Texto al 200 % sin cortes ni superposiciones
- [ ] Revisada en claro, oscuro y alto contraste
- [ ] Fondos borde a borde; contenido interactivo dentro de áreas seguras
- [ ] Retroceso predictivo intacto; `PopScope` solo con cambios sin guardar
- [ ] Estados de carga, vacío, error y sin conexión implementados
- [ ] Iconos interactivos con etiqueta; color nunca es el único indicador
- [ ] Movimiento reducido respetado; una sola vibración por gesto
- [ ] Golden tests y pruebas de accesibilidad en verde

### 13.3 Orden de adopción (migración)

1. Tokens y tema (`NTheme`, colores semánticos, tipografía, resortes).
2. Componentes oficiales de `material_ui` tematizados.
3. Adaptadores M3E para lo que falta.
4. Componentes de dominio (PTT, llamada, salas, contactos).
5. Migrar pantallas existentes de mayor uso primero.
6. Las nuevas pantallas nacen sobre el sistema; nunca al margen.

---

## 14. Correcciones al documento base

El documento de investigación previo acierta en la dirección general (HCT, contención tonal, resortes, borde a borde), pero tiene errores que romperían la implementación:

| Documento base | Corrección |
| --- | --- |
| Contenedores con «squircles» de 24 a 32 dp | M3 usa escornes circulares con una escala de 10 tokens (4 a 48 dp y full) |
| Se eliminan por completo las sombras | La elevación por defecto es 0, pero FAB, menús y toolbar flotante sí llevan sombra |
| `material_color_utilities` y `fromImageProvider` dan «la misma fidelidad que Monet» | La versión Dart implementa la especificación de color 2021; Android 16+ genera con la de 2025 |
| `dynamic_color: ^1.8.1` | `dynamic_color` va en 2.1.0 y depende de `material_ui` |
| `import 'package:flutter/material.dart'` | Desde Flutter 3.47 se usa `package:material_ui/material_ui.dart` |
| `CardTheme(...)` dentro de `ThemeData` | `ThemeData.cardTheme` recibe `CardThemeData` |
| `colors.shadow.withOpacity(0.05)` | `withOpacity` está obsoleto; usar `withValues(alpha: 0.05)` |
| `statusBarIconBrightness: Brightness.dark` fijo | El color de los iconos lo decide el tema o la app bar |

---

## 15. Implementación de referencia en Dart

Fragmentos de código listos para usar como punto de partida del sistema en `app/lib/ui/`.

### 15.1 Tokens

```dart
// app/lib/ui/tokens.dart
import 'package:flutter/physics.dart';
import 'package:material_ui/material_ui.dart';

/// Espaciado: retícula base de 4 dp.
abstract final class NSpace {
  static const double xs = 4, s = 8, m = 12, l = 16, xl = 24, xxl = 32, xxxl = 48, huge = 64;
}

/// Escala de esquinas (tokens oficiales M3 Expressive).
abstract final class NShape {
  static const double extraSmall = 4, small = 8, medium = 12, large = 16,
      largeIncreased = 20, extraLarge = 28, extraLargeIncreased = 32,
      extraExtraLarge = 48;

  static RoundedRectangleBorder rounded(double r) =>
      RoundedRectangleBorder(borderRadius: BorderRadius.circular(r));
}

/// Valores de ExpressiveMotionTokens / StandardMotionTokens.
/// SpringDescription espera el coeficiente de amortiguamiento, por eso se
/// construye desde el ratio con withDampingRatio.
abstract final class NMotion {
  static SpringDescription _s(double stiffness, double ratio) =>
      SpringDescription.withDampingRatio(mass: 1, stiffness: stiffness, ratio: ratio);

  static final expressiveFastSpatial = _s(800, 0.6);
  static final expressiveDefaultSpatial = _s(380, 0.8);
  static final expressiveSlowSpatial = _s(200, 0.8);

  static final standardFastSpatial = _s(1400, 0.9);
  static final standardDefaultSpatial = _s(700, 0.9);
  static final standardSlowSpatial = _s(300, 0.9);

  static final fastEffects = _s(3800, 1.0);
  static final defaultEffects = _s(1600, 1.0);
  static final slowEffects = _s(800, 1.0);
}

enum WindowClass {
  compact, medium, expanded, large, extraLarge;

  static WindowClass of(BuildContext context) {
    final w = MediaQuery.sizeOf(context).width;
    if (w < 600) return compact;
    if (w < 840) return medium;
    if (w < 1200) return expanded;
    if (w < 1600) return large;
    return extraLarge;
  }
}
```

### 15.2 Tema

```dart
// app/lib/ui/theme.dart
import 'package:material_ui/material_ui.dart';
import 'tokens.dart';
import 'semantic_colors.dart';

abstract final class NTheme {
  static const _font = 'packages/gravital_talk_app/GoogleSansFlex';

  static TextStyle? _axes(TextStyle? s, {bool emphasized = false}) {
    if (s == null) return null;
    final base = s.fontWeight ?? FontWeight.w400;
    final w = emphasized ? (base.value >= 500 ? FontWeight.w700 : FontWeight.w500) : base;
    return s.copyWith(
      fontWeight: w,
      fontVariations: [
        FontVariation.weight(w.value.toDouble()),
        if (s.fontSize != null) FontVariation.opticalSize(s.fontSize!),
      ],
    );
  }

  static TextTheme _map(TextTheme t, TextStyle? Function(TextStyle?) f) => TextTheme(
        displayLarge: f(t.displayLarge), displayMedium: f(t.displayMedium), displaySmall: f(t.displaySmall),
        headlineLarge: f(t.headlineLarge), headlineMedium: f(t.headlineMedium), headlineSmall: f(t.headlineSmall),
        titleLarge: f(t.titleLarge), titleMedium: f(t.titleMedium), titleSmall: f(t.titleSmall),
        bodyLarge: f(t.bodyLarge), bodyMedium: f(t.bodyMedium), bodySmall: f(t.bodySmall),
        labelLarge: f(t.labelLarge), labelMedium: f(t.labelMedium), labelSmall: f(t.labelSmall),
      );

  static ThemeData build(ColorScheme scheme) {
    final base = ThemeData(colorScheme: scheme);
    final text = _map(
      base.textTheme.apply(fontFamily: _font, bodyColor: scheme.onSurface, displayColor: scheme.onSurface),
      _axes,
    );
    final emphasized = _map(text, (s) => _axes(s, emphasized: true));

    return base.copyWith(
      textTheme: text,
      scaffoldBackgroundColor: scheme.surface,
      cardTheme: CardThemeData(
        elevation: 0,
        margin: EdgeInsets.zero,
        color: scheme.surfaceContainerLow,
        shape: NShape.rounded(NShape.medium),
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: scheme.surfaceContainerHigh,
        shape: NShape.rounded(NShape.extraLarge),
      ),
      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: scheme.surfaceContainerLow,
        showDragHandle: true,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.vertical(top: Radius.circular(NShape.extraLarge)),
        ),
      ),
      navigationBarTheme: NavigationBarThemeData(
        backgroundColor: scheme.surfaceContainer,
        indicatorColor: scheme.secondaryContainer,
      ),
      pageTransitionsTheme: const PageTransitionsTheme(builders: {
        TargetPlatform.android: PredictiveBackPageTransitionsBuilder(),
        TargetPlatform.iOS: CupertinoPageTransitionsBuilder(),
        TargetPlatform.macOS: CupertinoPageTransitionsBuilder(),
      }),
      extensions: [
        SemanticColors.from(scheme),
        EmphasizedTextTheme(emphasized),
      ],
    );
  }
}

@immutable
class EmphasizedTextTheme extends ThemeExtension<EmphasizedTextTheme> {
  const EmphasizedTextTheme(this.text);
  final TextTheme text;

  @override
  EmphasizedTextTheme copyWith({TextTheme? text}) => EmphasizedTextTheme(text ?? this.text);

  @override
  EmphasizedTextTheme lerp(EmphasizedTextTheme? other, double t) =>
      other == null ? this : EmphasizedTextTheme(TextTheme.lerp(text, other.text, t));
}
```

### 15.3 Colores semánticos (dominio Gravital Talk)

```dart
// app/lib/ui/semantic_colors.dart
import 'package:dynamic_color/dynamic_color.dart';
import 'package:material_ui/material_ui.dart';

@immutable
class SemanticTone {
  const SemanticTone(this.color, this.onColor, this.container, this.onContainer);
  final Color color, onColor, container, onContainer;

  factory SemanticTone.from(Color seed, ColorScheme scheme) {
    final s = ColorScheme.fromSeed(
      seedColor: seed.harmonizeWith(scheme.primary),
      brightness: scheme.brightness,
      dynamicSchemeVariant: DynamicSchemeVariant.fidelity,
    );
    return SemanticTone(s.primary, s.onPrimary, s.primaryContainer, s.onPrimaryContainer);
  }

  static SemanticTone lerp(SemanticTone a, SemanticTone b, double t) => SemanticTone(
        Color.lerp(a.color, b.color, t)!,
        Color.lerp(a.onColor, b.onColor, t)!,
        Color.lerp(a.container, b.container, t)!,
        Color.lerp(a.onContainer, b.onContainer, t)!,
      );
}

@immutable
class SemanticColors extends ThemeExtension<SemanticColors> {
  const SemanticColors({
    required this.success, required this.warning, required this.info,
    required this.pttActive, required this.pttFloor, required this.calling,
    required this.connectionPoor,
  });

  final SemanticTone success, warning, info, pttActive, pttFloor, calling, connectionPoor;

  factory SemanticColors.from(ColorScheme s) => SemanticColors(
        success: SemanticTone.from(const Color(0xFF1E8E3E), s),
        warning: SemanticTone.from(const Color(0xFFF9AB00), s),
        info: SemanticTone.from(const Color(0xFF1A73E8), s),
        pttActive: SemanticTone.from(const Color(0xFFE8710A), s),
        pttFloor: SemanticTone.from(const Color(0xFF0B57D0), s),
        calling: SemanticTone.from(const Color(0xFF9C27B0), s),
        connectionPoor: SemanticTone.from(const Color(0xFFC5221F), s),
      );

  @override
  SemanticColors copyWith() => this;

  @override
  SemanticColors lerp(SemanticColors? other, double t) {
    if (other == null) return this;
    return SemanticColors(
      success: SemanticTone.lerp(success, other.success, t),
      warning: SemanticTone.lerp(warning, other.warning, t),
      info: SemanticTone.lerp(info, other.info, t),
      pttActive: SemanticTone.lerp(pttActive, other.pttActive, t),
      pttFloor: SemanticTone.lerp(pttFloor, other.pttFloor, t),
      calling: SemanticTone.lerp(calling, other.calling, t),
      connectionPoor: SemanticTone.lerp(connectionPoor, other.connectionPoor, t),
    );
  }
}
```

### 15.4 Componente de dominio: PTT Button

```dart
// app/lib/ui/components/ptt_button.dart
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import '../tokens.dart';

/// Botón Push-To-Talk principal. Cambia de forma con resorte y dispara
/// háptica al activarse. Color nunca es el único indicador: también cambia
/// de forma, tamaño y etiqueta.
class PttButton extends StatefulWidget {
  const PttButton({
    super.key,
    required this.onPress,
    required this.onRelease,
    this.size = 72,
    this.semanticsLabel = 'Mantener para hablar',
  });

  final VoidCallback onPress;
  final VoidCallback onRelease;
  final double size;
  final String semanticsLabel;

  @override
  State<PttButton> createState() => _PttButtonState();
}

class _PttButtonState extends State<PttButton>
    with SingleTickerProviderStateMixin {
  late final _c = AnimationController.unbounded(vsync: this, value: 1);
  bool _pressed = false;

  void _press() {
    _pressed = true;
    HapticFeedback.lightImpact();
    widget.onPress();
    _c.animateWith(SpringSimulation(
      NMotion.expressiveFastSpatial, _c.value, 0.90, 0));
  }

  void _release() {
    if (!_pressed) return;
    _pressed = false;
    widget.onRelease();
    _c.animateWith(SpringSimulation(
      NMotion.expressiveFastSpatial, _c.value, 1.0, 0));
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Semantics(
      label: widget.semanticsLabel,
      button: true,
      child: GestureDetector(
        onTapDown: (_) => _press(),
        onTapUp: (_) => _release(),
        onTapCancel: _release,
        child: AnimatedBuilder(
          animation: _c,
          builder: (context, _) {
            return Transform.scale(
              scale: _c.value.clamp(0.85, 1.0),
              child: Container(
                width: widget.size,
                height: widget.size,
                decoration: BoxDecoration(
                  color: _pressed ? scheme.primaryContainer : scheme.primary,
                  shape: BoxShape.circle,
                  boxShadow: _pressed
                      ? [BoxShadow(
                          color: scheme.primary.withValues(alpha: 0.3),
                          blurRadius: 24,
                          spreadRadius: 4,
                        )]
                      : null,
                ),
                child: Icon(
                  _pressed ? Icons.mic : Icons.mic_none,
                  size: widget.size * 0.42,
                  color: _pressed ? scheme.onPrimaryContainer : scheme.onPrimary,
                ),
              ),
            );
          },
        ),
      ),
    );
  }
}
```

### 15.5 Arranque de la app

```dart
// app/lib/ui/app.dart
import 'package:dynamic_color/dynamic_color.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'theme.dart';

/// Prepara binding, edge-to-edge y overlay transparente.
void prepareApp() {
  WidgetsFlutterBinding.ensureInitialized();
  SystemChrome.setEnabledSystemUIMode(SystemUiMode.edgeToEdge);
  SystemChrome.setSystemUIOverlayStyle(const SystemUiOverlayStyle(
    systemNavigationBarColor: Colors.transparent,
    systemNavigationBarContrastEnforced: false,
  ));
}

/// Construye el tema desde la semilla de marca o el color dinámico.
ThemeData buildTheme(ColorScheme light, ColorScheme dark) =>
    NTheme.build(MaterialApp.themeMode == ThemeMode.dark ? dark : light);
```

### 15.6 Resorte con rebote real

Un `AnimationController` normal recorta los valores entre 0 y 1, así que el rebote de los resortes espaciales se pierde. Para animaciones espaciales se usa un controlador sin límites:

```dart
late final _c = AnimationController.unbounded(vsync: this, value: 1);

void _press()   => _c.animateWith(SpringSimulation(NMotion.expressiveFastSpatial, _c.value, 0.94, 0));
void _release() => _c.animateWith(SpringSimulation(NMotion.expressiveFastSpatial, _c.value, 1.0, 0));

// build: ScaleTransition(scale: _c, child: …) dentro de un RepaintBoundary
```

---

## 16. Fuentes

Páginas consultadas para este documento (revisadas el 12 oct 2026):

**Flutter y paquetes**

- [What's new in Flutter 3.47](https://flutter.dev/blog/whats-new-in-flutter-3-47) — `material_ui` / `cupertino_ui` 1.0, Impeller en escritorio, mínimos iOS 15 y SDK Android 36.
- [material\_ui — changelog](https://pub.dev/packages/material_ui/changelog) — `StyleVariant`, IconButton M3E, niveles de contraste.
- [dynamic\_color — changelog](https://pub.dev/packages/dynamic_color/changelog)
- [material\_3\_expressive](https://pub.dev/packages/material_3_expressive)

**Tokens oficiales de Material (código fuente de Jetpack Compose)**

- [ExpressiveMotionTokens.kt](https://raw.githubusercontent.com/androidx/androidx/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/ExpressiveMotionTokens.kt)
- [StandardMotionTokens.kt](https://raw.githubusercontent.com/androidx/androidx/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/StandardMotionTokens.kt)
- [ShapeTokens.kt](https://raw.githubusercontent.com/androidx/androidx/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/ShapeTokens.kt)
- [TypeScaleTokens.kt](https://raw.githubusercontent.com/androidx/androidx/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/TypeScaleTokens.kt)

**Tipografía**

- [Google Sans Flex — METADATA.pb (repositorio google/fonts)](https://raw.githubusercontent.com/google/fonts/main/ofl/googlesansflex/METADATA.pb) — ejes y rangos oficiales.
- [Google's versatile Sans Flex font is now free to the public](https://www.androidauthority.com/google-sans-flex-free-3617034/) — Android Authority, 18 nov 2025.
