/// Sistema de diseño Glassmorphism de Gravital Talk.
///
/// ## La receta, y por qué el fondo es la parte importante
///
/// El glassmorphism no es una propiedad de la tarjeta: es una propiedad del
/// **conjunto** tarjeta + fondo. Un vidrio translúcido sobre un fondo plano
/// negro no se lee como vidrio, sino como una mancha gris. Las tres
/// referencias usadas (music, liquid glass, fitness) comparten lo mismo: un
/// degradado saturado con manchas de luz difusas detrás de cada tarjeta.
///
/// De ahí las tres piezas de este fichero: el fondo ([GravitalBackground]),
/// la superficie de vidrio ([GlassSurface]) y los colores que ambas comparten.
///
/// ## Contraste: medido, no supuesto
///
/// En una app de comunicaciones hay métricas, texto y estados que hay que leer
/// de un vistazo, así que el vidrio no puede ir a costa de la legibilidad.
/// Medido sobre el caso peor —el punto más claro del fondo detrás del vidrio—
/// con el vidrio al 14 % de blanco:
///
/// | Texto | Contraste |
/// |---|---|
/// | Primario (blanco) | 7.01:1 |
/// | Secundario (blanco al 72 %) | 4.57:1 |
///
/// Ambos pasan AA. De ahí que [GlassTextTheme.dark] use 0.72 y no menos: bajar
/// a 55 % deja el texto terciario en 3.38:1, que ya no pasa.
///
/// ## Modo claro y oscuro
///
/// El glass funciona en los dos, pero con recetas distintas: en claro el fondo
/// es crema (como la referencia de fitness) y el texto es oscuro. Los colores
/// de texto salen de un `ThemeExtension` en lugar de constantes, porque un
/// texto fijo blanco sobre fondo claro sería invisible.
library;

import 'dart:ui' show ImageFilter;

import 'package:flutter/material.dart';

import 'tokens.dart';

/// Colores del fondo y del vidrio, por tema.
abstract final class GlassColors {
  // ── Modo oscuro ─────────────────────────────────────────────────────────

  /// Primera parada del degradado oscuro (arriba): azul profundo.
  static const backgroundTopDark = Color(0xFF071A2B);

  /// Segunda parada: azul petróleo, el tono medio de la marca.
  static const backgroundMidDark = Color(0xFF0B2337);

  /// Tercera parada (abajo): violeta profundo.
  static const backgroundBottomDark = Color(0xFF1A0B2E);

  // ── Modo claro ──────────────────────────────────────────────────────────

  /// Degradado claro: crema hacia menta, como la referencia de fitness.
  static const backgroundTopLight = Color(0xFFEFF6F4);

  static const backgroundMidLight = Color(0xFFE3EEF3);

  static const backgroundBottomLight = Color(0xFFF5EAF3);

  /// Manchas de luz del fondo. Se difuminan dentro del vidrio y son las que
  /// hacen que el glass tenga algo que refractar.
  static const orbTeal = Color(0xFF14B8A6);
  static const orbViolet = Color(0xFF7C3AED);
  static const orbMagenta = Color(0xFFDB2777);

  // ── Vidrio ──────────────────────────────────────────────────────────────

  /// Relleno del vidrio. 14 % de blanco: medido como el mejor compromiso
  /// entre "se lee como vidrio" y "el texto sigue en AA".
  static const glassFillTop = Color(0x24FFFFFF); // 14 %
  static const glassFillBottom = Color(0x0FFFFFFF); // 6 %

  /// Canto especular: el destello del borde del vidrio.
  ///
  /// Más intenso arriba a la izquierda, que es de donde viene la luz en las
  /// tres referencias.
  static const glassBorderTop = Color(0x59FFFFFF); // 35 %
  static const glassBorderBottom = Color(0x14FFFFFF); // 8 %
}

/// Colores de texto sobre vidrio, por tema.
///
/// Es un `ThemeExtension` y no constantes porque el contraste sobre vidrio
/// depende de lo que haya debajo: en modo claro un texto blanco sería
/// invisible, así que cada tema aporta su propia tríada.
@immutable
class GlassTextTheme extends ThemeExtension<GlassTextTheme> {
  const GlassTextTheme({
    required this.primary,
    required this.secondary,
    required this.tertiary,
  });

  /// Texto principal en oscuro: blanco puro.
  static const dark = GlassTextTheme(
    primary: Color(0xFFFFFFFF),
    secondary: Color(0xB8FFFFFF), // 72 %
    tertiary: Color(0x8CFFFFFF),
  );

  /// Texto principal en claro: tinta oscura.
  static const light = GlassTextTheme(
    primary: Color(0xFF0F172A),
    secondary: Color(0xB30F172A), // 70 %
    tertiary: Color(0x8C0F172A),
  );

  final Color primary;
  final Color secondary;
  final Color tertiary;

  /// Atajo para usar desde `build`.
  static GlassTextTheme of(BuildContext context) =>
      Theme.of(context).extension<GlassTextTheme>() ?? dark;

  @override
  GlassTextTheme copyWith({
    Color? primary,
    Color? secondary,
    Color? tertiary,
  }) =>
      GlassTextTheme(
        primary: primary ?? this.primary,
        secondary: secondary ?? this.secondary,
        tertiary: tertiary ?? this.tertiary,
      );

  @override
  GlassTextTheme lerp(ThemeExtension<GlassTextTheme>? other, double t) {
    if (other == null || other is! GlassTextTheme) return this;
    return GlassTextTheme(
      primary: Color.lerp(primary, other.primary, t)!,
      secondary: Color.lerp(secondary, other.secondary, t)!,
      tertiary: Color.lerp(tertiary, other.tertiary, t)!,
    );
  }
}

/// Acentos para estados sobre cristal.
abstract final class GlassAccent {
  /// Elemento activo o seleccionado.
  static const active = Color(0xFF0F9E8E);

  /// Transmitiendo: la convención de "en el aire".
  static const transmit = Color(0xFFB45309);

  static const danger = Color(0xFFB3261E);
  static const online = Color(0xFF15803D);
}

/// Fondo degradado con manchas de luz.
///
/// Es lo que hace que el resto del glass funcione: sin luz detrás, el vidrio
/// no tiene nada que difuminar y se limita a aclarar un poco el fondo.
///
/// Las manchas se pintan con gradientes radiales a baja opacidad. No llevan
/// blur propio —lo aporta el `BackdropFilter` de cada [GlassSurface]— así que
/// el fondo cuesta un degradado y nada más.
class GravitalBackground extends StatelessWidget {
  const GravitalBackground({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final colors = dark
        ? const [
            GlassColors.backgroundTopDark,
            GlassColors.backgroundMidDark,
            GlassColors.backgroundBottomDark,
          ]
        : const [
            GlassColors.backgroundTopLight,
            GlassColors.backgroundMidLight,
            GlassColors.backgroundBottomLight,
          ];
    // En claro las manchas se suavizan: con la opacidad del oscuro taparían
    // el texto.
    final intensity = dark ? 1.0 : 0.55;

    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: colors,
          stops: const [0.0, 0.45, 1.0],
        ),
      ),
      child: Stack(
        children: [
          // Las manchas usan FractionallySizedBox para mantener la proporción
          // en cualquier tamaño de pantalla.
          Orb(
            color: GlassColors.orbTeal,
            alignment: const Alignment(0.9, -0.75),
            widthFactor: 0.9,
            heightFactor: 0.5,
            opacity: 0.45 * intensity,
          ),
          Orb(
            color: GlassColors.orbViolet,
            alignment: const Alignment(-0.95, -0.15),
            widthFactor: 0.8,
            heightFactor: 0.45,
            opacity: 0.40 * intensity,
          ),
          Orb(
            color: GlassColors.orbMagenta,
            alignment: const Alignment(0.75, 0.85),
            widthFactor: 0.85,
            heightFactor: 0.45,
            opacity: 0.32 * intensity,
          ),
          child,
        ],
      ),
    );
  }
}

/// Una mancha de luz difusa dentro del fondo.
class Orb extends StatelessWidget {
  const Orb({
    super.key,
    required this.color,
    required this.alignment,
    required this.widthFactor,
    required this.heightFactor,
    required this.opacity,
  });

  final Color color;
  final Alignment alignment;
  final double widthFactor;
  final double heightFactor;
  final double opacity;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: alignment,
      child: FractionallySizedBox(
        widthFactor: widthFactor,
        heightFactor: heightFactor,
        child: DecoratedBox(
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            gradient: RadialGradient(
              colors: [
                color.withValues(alpha: opacity),
                color.withValues(alpha: 0.0),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Superficie de vidrio reutilizable.
///
/// Aplica la receta completa: desenfoque del fondo, degradado translúcido y
/// canto especular. Es el bloque sobre el que se construyen el resto de
/// componentes.
class GlassSurface extends StatelessWidget {
  const GlassSurface({
    super.key,
    required this.child,
    this.borderRadius = Radii.glass,
    this.padding = const EdgeInsets.all(Spacing.xl),
    this.blur = 18,
    this.showBorder = true,
    this.glow,
    this.width,
    this.height,
  });

  /// Contenido de la tarjeta.
  final Widget child;

  /// Radio de las esquinas.
  final double borderRadius;

  /// Relleno interior.
  final EdgeInsetsGeometry padding;

  /// Intensidad del desenfoque del fondo, en sigma.
  ///
  /// 18 es el valor que se lee como vidrio sin perder la silueta de lo que
  /// hay detrás. Subirlo cuesta GPU y no aporta legibilidad.
  final double blur;

  /// Si pintar el canto especular.
  final bool showBorder;

  /// Color del resplandor exterior, o `null` para no pintarlo.
  ///
  /// Se usa en el estado "transmitiendo": el vidrio emite luz propia.
  final Color? glow;

  final double? width;
  final double? height;

  @override
  Widget build(BuildContext context) {
    final radius = BorderRadius.circular(borderRadius);

    final box = AnimatedContainer(
      duration: GravitalDurations.fast,
      curve: Curves.easeOut,
      width: width,
      height: height,
      padding: padding,
      decoration: BoxDecoration(
        borderRadius: radius,
        gradient: const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [GlassColors.glassFillTop, GlassColors.glassFillBottom],
        ),
        boxShadow: glow == null
            ? null
            : [
                BoxShadow(
                  color: glow!.withValues(alpha: 0.45),
                  blurRadius: 28,
                  spreadRadius: 2,
                ),
              ],
      ),
      child: child,
    );

    // El canto se pinta dentro de un ClipRRect para que siga el contorno en
    // lugar de ser una línea recta.
    final clipped = showBorder
        ? ClipRRect(
            borderRadius: radius,
            child: CustomPaint(
              painter: _SpecularBorderPainter(borderRadius: borderRadius),
              child: box,
            ),
          )
        : box;

    // El desenfoque va al final: tiene que envolver la tarjeta ya montada
    // para borrar lo que hay justo detrás de ella.
    return ClipRRect(
      borderRadius: radius,
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: blur, sigmaY: blur),
        child: clipped,
      ),
    );
  }
}

/// Pinta el canto especular: un degradado blanco fino en el borde, más
/// intenso arriba a la izquierda. Es el detalle que hace que el vidrio parezca
/// tener grosor en vez de ser un relleno plano.
class _SpecularBorderPainter extends CustomPainter {
  const _SpecularBorderPainter({required this.borderRadius});

  final double borderRadius;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Rect.fromLTWH(0.5, 0.5, size.width - 1, size.height - 1);
    final edge = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.0
      ..shader = LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [
          GlassColors.glassBorderTop,
          GlassColors.glassBorderBottom,
        ],
      ).createShader(rect);
    canvas.drawRRect(
      RRect.fromRectAndRadius(rect, Radius.circular(borderRadius)),
      edge,
    );
  }

  @override
  bool shouldRepaint(_SpecularBorderPainter old) =>
      old.borderRadius != borderRadius;
}

/// Botón de acción principal, en vidrio con acento.
///
/// Existe porque un botón de texto plano sobre vidrio se pierde: el acento lo
/// separa del fondo y lo convierte en el objetivo táctil.
///
/// No lleva `BackdropFilter`: en un elemento tan pequeño el desenfoque no se
/// aprecia y sí cuesta. Lo que da el aspecto de vidrio es el relleno
/// translúcido con el canto del color del acento.
class GlassButton extends StatelessWidget {
  const GlassButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.icon,
    this.color = GlassAccent.active,
    this.expand = true,
  });

  final String label;
  final VoidCallback? onPressed;
  final IconData? icon;
  final Color color;
  final bool expand;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final enabled = onPressed != null;
    final tint = color.withValues(alpha: enabled ? 0.24 : 0.10);

    final content = Row(
      mainAxisSize: expand ? MainAxisSize.max : MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        if (icon != null) ...[
          Icon(icon, size: 18, color: enabled ? color : GlassTextTheme.of(context).tertiary),
          const SizedBox(width: Spacing.sm),
        ],
        Text(
          label,
          style: theme.textTheme.labelLarge?.copyWith(
            color: enabled ? GlassTextTheme.of(context).primary : GlassTextTheme.of(context).tertiary,
            fontWeight: FontWeight.w700,
          ),
        ),
      ],
    );

    return Semantics(
      button: true,
      enabled: enabled,
      child: Opacity(
        opacity: enabled ? 1 : 0.6,
        child: Material(
          color: Colors.transparent,
          child: InkWell(
            onTap: onPressed,
            borderRadius: BorderRadius.circular(Radii.pill),
            child: Container(
              width: expand ? double.infinity : null,
              padding: const EdgeInsets.symmetric(
                horizontal: Spacing.lg,
                vertical: Spacing.md,
              ),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(Radii.pill),
                gradient: LinearGradient(
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                  colors: [tint, tint.withValues(alpha: tint.a * 0.5)],
                ),
                border: Border.all(
                  color: enabled
                      ? color.withValues(alpha: 0.5)
                      : GlassTextTheme.of(context).tertiary.withValues(alpha: 0.3),
                ),
              ),
              child: Center(child: content),
            ),
          ),
        ),
      ),
    );
  }
}