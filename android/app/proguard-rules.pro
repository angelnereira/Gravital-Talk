# Reglas ProGuard para Gravital Talk.

# Mantener la clase JNI para que el linker pueda encontrar los métodos nativos.
-keep class com.gravitaltalk.GravitalTalkJni { *; }

# Mantener enums de estado usados en la UI.
-keepclassmembers enum com.gravitaltalk.** { *; }
